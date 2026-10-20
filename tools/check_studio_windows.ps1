#requires -Version 7.0
param([Parameter(Mandatory)][string]$PackageDir,[Parameter(Mandatory)][string]$ValidationRoot,[Parameter(Mandatory)][string]$ArtworkPath)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$checks=[Collections.Generic.List[object]]::new();$core=$null;$base=$null
$report=[ordered]@{success=$false;scope='Packaged Windows core: real model exports and recipe subprocess. Isolated data; no paid model or OSC.';checks=$checks;error=$null}
function Check([string]$Name,[bool]$Value){$checks.Add(@{name=$Name;passed=$Value});if(-not $Value){throw $Name}}
function Api([string]$Path,$Body=$null){if($null-eq $Body){return Invoke-RestMethod ($base+'/api/'+$Path) -NoProxy};Invoke-RestMethod ($base+'/api/'+$Path) -Method Post -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes(($Body|ConvertTo-Json -Depth 100 -Compress))) -NoProxy -TimeoutSec 60}
try {
 New-Item -ItemType Directory -Path $ValidationRoot -Force|Out-Null
 $data=Join-Path $ValidationRoot 'Data';Check 'validation data is fresh' (-not(Test-Path $data))
 $listener=[Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback,0);$listener.Start();$port=$listener.LocalEndpoint.Port;$listener.Stop()
 $core=Start-Process -FilePath (Join-Path $PackageDir 'morphforge.exe') -ArgumentList @('serve','--data',('"'+$data+'"'),'--port',$port,'--no-capture','--env-file',('"'+(Join-Path $ValidationRoot 'no-key.env')+'"')) -PassThru -RedirectStandardOutput (Join-Path $ValidationRoot 'stdout.log') -RedirectStandardError (Join-Path $ValidationRoot 'stderr.log')
 $base='http://127.0.0.1:'+$port;$s=$null
 for($i=0;$i-lt 100;$i++){try{$s=Api 'state';break}catch{if($core.HasExited){throw 'QA core exited'};Start-Sleep -Milliseconds 100}}
 Check 'actual packaged version is 0.14.0 and OSC is off' ($null-ne $s -and $s.software.version-eq '0.14.0' -and -not $s.connection.enabled)
 $html=(Invoke-WebRequest $base -NoProxy).Content
 Check 'packaged UI embeds observation and desktop menus' ($html.Contains('id="desktopMenu"') -and $html.Contains('id="activityFeed"') -and $html.Contains('id="modelExportDialog"'))
 $null=Api 'import' (Get-Content -LiteralPath $ArtworkPath -Raw | ConvertFrom-Json -Depth 100)
 $state=Api 'state';$scene=Api 'scene';$faces=$scene.scene.triangles.Count
 Check 'real kingfisher triangle artwork imports on Windows' ($faces-eq 1134 -and $scene.scene.boxes.Count-eq 0)
 foreach($format in @('glb','obj','ply','stl')){
  $path=Join-Path $ValidationRoot ('kingfisher.'+$format)
  $r=Invoke-WebRequest ($base+'/api/export/model?format='+$format) -OutFile $path -PassThru -NoProxy
  Check ($format+' is a successful downloadable file') ($r.StatusCode-eq 200 -and (Get-Item $path).Length-gt 100)
 }
 $glb=[IO.File]::ReadAllBytes((Join-Path $ValidationRoot 'kingfisher.glb'));Check 'GLB header and total length are correct' ([Text.Encoding]::ASCII.GetString($glb,0,4)-eq 'glTF' -and [BitConverter]::ToUInt32($glb,8)-eq $glb.Length)
 $doc=[Text.Encoding]::UTF8.GetString($glb,20,[BitConverter]::ToInt32($glb,12))|ConvertFrom-Json -Depth 100
 $vertices=0;foreach($mesh in $doc.meshes){foreach($primitive in $mesh.primitives){$vertices+=$doc.accessors[$primitive.attributes.POSITION].count}}
 Check 'GLB preserves all real triangle faces' ($vertices-eq $faces*3)
 $zip=[IO.Compression.ZipFile]::OpenRead((Join-Path $ValidationRoot 'kingfisher.obj'));try{Check 'OBJ bundle includes material sidecar' ($null-ne $zip.GetEntry('model.obj') -and $null-ne $zip.GetEntry('model.mtl'))}finally{$zip.Dispose()}
 $stl=[IO.File]::ReadAllBytes((Join-Path $ValidationRoot 'kingfisher.stl'));Check 'STL face count and binary length match' ([BitConverter]::ToUInt32($stl,80)-eq $faces -and $stl.Length-eq 84+50*$faces)
 $ply=[IO.File]::ReadAllBytes((Join-Path $ValidationRoot 'kingfisher.ply'));$header=[Text.Encoding]::ASCII.GetString($ply,0,[Math]::Min(500,$ply.Length));Check 'PLY preserves RGB vertex/face schema' ($header.Contains('element face '+$faces) -and $header.Contains('property uchar red'))
 Check 'exports leave project revision unchanged' ((Api 'state').revision-eq $state.revision)
 $null=Api 'harness/tool' @{name='run_recipe';arguments=@{mode='apply';baseRevision=$state.revision;code='mf.box([50,0,50],[2,2,2],{style:1,group:"qa-worker"});'}}
 Check 'packaged recipe worker still runs from its immutable executable' ((Api 'scene').scene.boxes.Count-eq 1)
 Check 'viewing and exporting start neither model nor OSC' (-not (Api 'state').connection.enabled -and $null-eq (Api 'harness/state').run)
 $report.version=$s.software.version;$report.faces=$faces;$report.success=$true
} catch {$report.error=$_.Exception.Message}
finally {
 if($core-and -not $core.HasExited){Stop-Process -Id $core.Id -Force}
 $report.finishedAt=[DateTimeOffset]::UtcNow.ToString('o');New-Item -ItemType Directory -Path $ValidationRoot -Force|Out-Null
 [IO.File]::WriteAllText((Join-Path $ValidationRoot 'validation.json'),($report|ConvertTo-Json -Depth 100),[Text.UTF8Encoding]::new($false))
}
if(-not $report.success){throw $report.error};Write-Output "Studio Windows acceptance passed: $($checks.Count) checks"
