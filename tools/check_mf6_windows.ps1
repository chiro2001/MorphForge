#requires -Version 7.0
param([Parameter(Mandatory)][string]$PackageDir,[Parameter(Mandatory)][string]$ValidationRoot,[Parameter(Mandatory)][string]$CatalogPath,[string]$ExpectedVersion='0.12.0')
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$checks=[Collections.Generic.List[object]]::new();$core=$null;$udp=$null;$base=$null
$report=[ordered]@{success=$false;scope='Packaged Windows core and recipe worker, real loopback UDP, Minecraft import. Isolated data, no paid model or VRChat traffic.';checks=$checks;error=$null}
function Check([string]$Name,[bool]$Value){$checks.Add(@{name=$Name;passed=$Value});if(-not $Value){throw $Name}}
function Api([string]$Path,$Body=$null){if($null-eq $Body){return Invoke-RestMethod ($base+'/api/'+$Path) -NoProxy};Invoke-RestMethod ($base+'/api/'+$Path) -Method Post -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes(($Body|ConvertTo-Json -Depth 100 -Compress))) -NoProxy -TimeoutSec 30}
function Wait-Api([scriptblock]$Condition){$watch=[Diagnostics.Stopwatch]::StartNew();while($watch.Elapsed.TotalSeconds-lt 15){$s=Api 'state';if(&$Condition $s){return $s};Start-Sleep -Milliseconds 100};throw 'API condition timed out'}
try{
 New-Item -ItemType Directory -Path $ValidationRoot -Force|Out-Null
 $data=Join-Path $ValidationRoot 'Data';Check 'QA uses a fresh workspace' (-not(Test-Path $data));$listener=[Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback,0);$listener.Start();$port=$listener.LocalEndpoint.Port;$listener.Stop()
 $core=Start-Process -FilePath (Join-Path $PackageDir 'morphforge.exe') -ArgumentList @('serve','--data',('"'+$data+'"'),'--port',$port,'--no-capture','--env-file',('"'+(Join-Path $ValidationRoot 'no-key.env')+'"')) -PassThru -RedirectStandardOutput (Join-Path $ValidationRoot 'stdout.log') -RedirectStandardError (Join-Path $ValidationRoot 'stderr.log')
 $base='http://127.0.0.1:'+$port;$s=$null;for($i=0;$i-lt 100;$i++){try{$s=Api 'state';break}catch{if($core.HasExited){throw 'QA core exited'};Start-Sleep -Milliseconds 100}}
 Check 'packaged core version and OSC disabled' ($null-ne $s -and $s.software.version-eq $ExpectedVersion -and -not $s.connection.enabled)
 $tools=Api 'harness/tools';Check 'mesh and Minecraft tools are embedded' ($tools.version-eq 'morphforge-tools/4.0' -and @($tools.tools|Where-Object {$_.name-in @('mesh_tools','minecraft_import')}).Count-eq 2)
 $null=Api 'mesh' @{action='wedge';primitive='voxel';styleId=1;baseRevision=$s.revision;min=@(0,0,0);size=@(4,4,4);direction='+x'}
 $s=Api 'state';$null=Api 'harness/tool' @{name='run_recipe';arguments=@{mode='apply';baseRevision=$s.revision;code='mf.mesh([[8,0,0],[12,0,0],[12,4,0],[8,4,0]],[[0,1,2],[0,2,3]],{colors:["#ff8800","#0088ff"],group:"qa-mesh"});'}}
 $scene=Api 'scene';Check 'packaged recipe subprocess creates indexed RGB mesh' ($scene.scene.triangles.Count-eq 2 -and $scene.scene.boxes[0].shape-eq 1)
 $zip=Join-Path $ValidationRoot 'builtin-catalog.zip';Invoke-WebRequest ($base+'/api/vrc/catalog?builtinMinecraft=true') -OutFile $zip -NoProxy
 Add-Type -AssemblyName System.IO.Compression.FileSystem;$archive=[IO.Compression.ZipFile]::OpenRead($zip);try{$reader=[IO.StreamReader]::new($archive.GetEntry('catalog.json').Open());try{$catalog=$reader.ReadToEnd()|ConvertFrom-Json}finally{$reader.Dispose()}}finally{$archive.Dispose()}
 Check 'expanded original builtin catalog exported' ($catalog.materials.Count-ge 500)
 $null=Invoke-RestMethod ($base+'/api/vrc/profile/import') -Method Post -Form @{file=(Get-Item -LiteralPath $CatalogPath)} -NoProxy
 $udp=[Net.Sockets.UdpClient]::new(0);$udp.Client.ReceiveTimeout=1000;$oscPort=([Net.IPEndPoint]$udp.Client.LocalEndPoint).Port
 $null=Api 'connect' @{profile='mf6';enabled=$true;port=$oscPort;interval=.25;capacity=16384;loopReplay=$true;auto=$false}
 $null=Api 'publish' @{}
 $found=$false;$watch=[Diagnostics.Stopwatch]::StartNew();while($watch.Elapsed.TotalSeconds-lt 15){try{$peer=[Net.IPEndPoint]::new([Net.IPAddress]::Any,0);$bytes=$udp.Receive([ref]$peer);$text=[Text.Encoding]::ASCII.GetString($bytes);if($text.Contains('/avatar/parameters/MF6_D28') -and $text.Contains('/avatar/parameters/MF6_Ctl')){$found=$true;break}}catch [Net.Sockets.SocketException]{}}
 Check 'real UDP contains the unified 29-byte payload and control' $found
 $s=Wait-Api {param($value) $value.connection.replayCycles-ge 1};$before=$s.connection.replayCycles
 $null=Api 'edit' @{op='clear'};$null=Api 'publish' @{}
 $s=Wait-Api {param($value) $value.connection.replayCycles-ge ($before+2)}
 Check 'empty target retains and replays both deletion types' ($s.connection.repairDeletions[0]-gt 0 -and $s.connection.repairDeletions[1]-gt 0 -and $s.connection.ack-eq $false)
 $null=Api 'connect' @{enabled=$false}
 $sourcePath=Join-Path $ValidationRoot 'building.nbt';[IO.File]::WriteAllBytes($sourcePath,[Convert]::FromBase64String('CgAACQAEc2l6ZQMAAAADAAAABAAAAAIAAAAECQAHcGFsZXR0ZQoAAAADCAAETmFtZQARbWluZWNyYWZ0Om9ha19sb2cACAAETmFtZQAWbWluZWNyYWZ0OnN0b25lX2JyaWNrcwAIAAROYW1lABJtaW5lY3JhZnQ6cmVkX3dvb2wACQAGYmxvY2tzCgAAABgJAANwb3MDAAAAAwAAAAAAAAAAAAAAAAMABXN0YXRlAAAAAAAJAANwb3MDAAAAAwAAAAAAAAABAAAAAAMABXN0YXRlAAAAAQAJAANwb3MDAAAAAwAAAAAAAAAAAAAAAQMABXN0YXRlAAAAAQAJAANwb3MDAAAAAwAAAAAAAAABAAAAAQMABXN0YXRlAAAAAgAJAANwb3MDAAAAAwAAAAAAAAAAAAAAAgMABXN0YXRlAAAAAgAJAANwb3MDAAAAAwAAAAAAAAABAAAAAgMABXN0YXRlAAAAAAAJAANwb3MDAAAAAwAAAAAAAAAAAAAAAwMABXN0YXRlAAAAAAAJAANwb3MDAAAAAwAAAAAAAAABAAAAAwMABXN0YXRlAAAAAQAJAANwb3MDAAAAAwAAAAEAAAAAAAAAAAMABXN0YXRlAAAAAQAJAANwb3MDAAAAAwAAAAEAAAABAAAAAAMABXN0YXRlAAAAAgAJAANwb3MDAAAAAwAAAAEAAAAAAAAAAwMABXN0YXRlAAAAAQAJAANwb3MDAAAAAwAAAAEAAAABAAAAAwMABXN0YXRlAAAAAgAJAANwb3MDAAAAAwAAAAIAAAAAAAAAAAMABXN0YXRlAAAAAgAJAANwb3MDAAAAAwAAAAIAAAABAAAAAAMABXN0YXRlAAAAAAAJAANwb3MDAAAAAwAAAAIAAAAAAAAAAwMABXN0YXRlAAAAAgAJAANwb3MDAAAAAwAAAAIAAAABAAAAAwMABXN0YXRlAAAAAAAJAANwb3MDAAAAAwAAAAMAAAAAAAAAAAMABXN0YXRlAAAAAAAJAANwb3MDAAAAAwAAAAMAAAABAAAAAAMABXN0YXRlAAAAAQAJAANwb3MDAAAAAwAAAAMAAAAAAAAAAQMABXN0YXRlAAAAAQAJAANwb3MDAAAAAwAAAAMAAAABAAAAAQMABXN0YXRlAAAAAgAJAANwb3MDAAAAAwAAAAMAAAAAAAAAAgMABXN0YXRlAAAAAgAJAANwb3MDAAAAAwAAAAMAAAABAAAAAgMABXN0YXRlAAAAAAAJAANwb3MDAAAAAwAAAAMAAAAAAAAAAwMABXN0YXRlAAAAAAAJAANwb3MDAAAAAwAAAAMAAAABAAAAAwMABXN0YXRlAAAAAQAA'))
 $source=Invoke-RestMethod ($base+'/api/minecraft/upload') -Method Post -Form @{file=(Get-Item -LiteralPath $sourcePath)} -NoProxy
 $s=Api 'state';$preview=Api 'minecraft' @{action='preview';sourceId=$source.source.id;baseRevision=$s.revision}
 Check 'Minecraft preview leaves the Windows draft unchanged' ((Api 'state').revision-eq $s.revision -and $preview.canApply)
 $null=Api 'minecraft' @{action='apply';previewId=$preview.previewId;baseRevision=$preview.baseRevision;requestId='windows-building-1'}
 Check 'Minecraft compiled geometry and source survive apply' ((Api 'scene').scene.boxes.Count-gt 0 -and (Api 'minecraft').sources.Count-eq 1)
 $null=Api 'undo' @{};Check 'Minecraft apply is undoable on Windows' ((Api 'scene').scene.boxes.Count-eq 0)
 $report.finalState=Api 'state';$report.success=$true
}catch{$report.error=$_.Exception.Message}
finally{if($udp){$udp.Dispose()};if($core-and -not $core.HasExited){Stop-Process -Id $core.Id -Force};$report.finishedAt=[DateTimeOffset]::UtcNow.ToString('o');New-Item -ItemType Directory -Path $ValidationRoot -Force|Out-Null;[IO.File]::WriteAllText((Join-Path $ValidationRoot 'validation.json'),($report|ConvertTo-Json -Depth 100),[Text.UTF8Encoding]::new($false))}
if(-not $report.success){throw $report.error};Write-Output "MF6 Windows acceptance passed: $($checks.Count) checks"
