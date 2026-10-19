#requires -Version 7.0
param([Parameter(Mandatory)][string]$BaseUrl,[Parameter(Mandatory)][string]$ReportPath)
$ErrorActionPreference='Stop'
$checks=[Collections.Generic.List[object]]::new()
$report=[ordered]@{success=$false;scope='Windows packaged desktop + real screenshot worker; isolated QA data only. No paid API, real VRChat or OSC.';checks=$checks;error=$null}
function Check([string]$Name,[bool]$Value){$checks.Add(@{name=$Name;passed=$Value});if(-not $Value){throw $Name}}
function Api([string]$Path,$Body=$null){if($null -eq $Body){return Invoke-RestMethod ($BaseUrl+'/api/'+$Path) -NoProxy};Invoke-RestMethod ($BaseUrl+'/api/'+$Path) -Method Post -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes(($Body|ConvertTo-Json -Depth 100 -Compress))) -NoProxy -TimeoutSec 60}
function Tool([string]$Name,$Arguments){Api 'harness/tool' @{name=$Name;arguments=$Arguments}}
try{
 $s=Api 'state';Check 'packaged core reports 0.7.0 and OSC off' ($s.software.version -eq '0.7.0' -and -not $s.connection.enabled)
 if($s.revision -ne 0){$null=Api 'workspaces' @{action='new_project';name='0.7 isolated QA';baseProjectId=$s.project.id;baseSessionId=$s.harness.context.id}}
 $null=Api 'provider/config' @{baseUrl='http://127.0.0.1:9';apiType='responses';requiresApiKey=$false;clearKey=$true}
 $null=Api 'edit' @{op='box';min=@(-12,-4,3);size=@(8,6,4);styleId=1;group='body'}
 $s=Api 'state';Check 'negative-Y target violation is visible' ($s.targetCompatibility.outOfBoundsCount -eq 1)
 $dry=Tool 'transform_scene' @{action='center_on_ground';baseRevision=$s.revision;dryRun=$true}
 Check 'center preview leaves original revision unchanged' ((Api 'state').revision -eq $s.revision -and $dry.offset[1] -eq 4)
 $moved=Tool 'transform_scene' @{action='center_on_ground';baseRevision=$s.revision}
 Check 'whole-scene centering fits target' ($moved.after.fits)
 $theme=Tool 'builtin_materials' @{action='apply';id='pixel-workshop';baseRevision=(Api 'state').revision}
 Check 'nineteen original pixel materials are offline' ($theme.materialsAdded -eq 19)
 $scene=Api 'scene';$grass=@($scene.materials|Where-Object {$_.uid -eq 'mf-pixel:grass'})[0];$water=@($scene.materials|Where-Object {$_.uid -eq 'mf-pixel:water'})[0]
 Check 'grass contains different top and bottom textures' ($grass.textures[2] -ne $grass.textures[3])
 $null=Api 'edit' @{operations=@(@{op='box';min=@(6,0,0);size=@(2,2,2);styleId=$water.id;group='water-a'},@{op='box';min=@(8,0,0);size=@(2,2,2);styleId=$water.id;group='water-b'},@{op='box';min=@(-3,0,-5);size=@(5,1,5);styleId=$grass.id;group='grass'})}
 $scene=Api 'scene';Check 'native backend finds adjacent transparent interfaces' ($scene.surfaces.hiddenFaces -eq 2)
 $settings=Api 'view-settings' @{quality='high';captureQuality='high';voxelGrid=$false;surfaceCulling=$true};Check 'quality options persist outside model context' ($settings.quality -eq 'high' -and -not $settings.voxelGrid)
 $capture=Tool 'capture_views' @{revision=$scene.scene.revision;mode='vrc';quality='high';width=640;height=480;views=@('hero');shots=@(@{id='grass-detail';fit=@{group='grass'};projection='orthographic';yaw=15;pitch=35})}
 Check 'real screenshot worker returns custom and preset cameras' ($capture.images.Count -eq 2)
 Check 'real Windows render uses 2x sampling and virtual light' (@($capture.images|Where-Object {$_.rendering.supersampling -eq 2 -and $_.rendering.mode -eq 'vrc' -and -not $_.rendering.voxelGrid}).Count -eq 2)
 $out=Split-Path -Parent $ReportPath;New-Item -ItemType Directory -Path $out -Force|Out-Null
 foreach($image in $capture.images){Invoke-WebRequest ($BaseUrl+$image.url) -OutFile (Join-Path $out ($image.view+'.png')) -NoProxy}
 $report.capture=$capture
 $catalogPath=Join-Path $out 'catalog.zip';Invoke-WebRequest ($BaseUrl+'/api/vrc/catalog') -OutFile $catalogPath -NoProxy
 $import=Invoke-RestMethod ($BaseUrl+'/api/vrc/profile/import') -Method Post -Form @{file=(Get-Item -LiteralPath $catalogPath)} -NoProxy
 Check 'target catalog export/import agrees with used materials' ($import.matched -and $import.surfaceMasks -and -not $import.remoteVerified)
 $null=Api 'publish' @{}
 Check 'preparing publication does not turn on OSC' (-not (Api 'state').connection.enabled)
 $tools=Api 'harness/tools';Check 'final localized tool schema is embedded' ($tools.version -eq 'morphforge-tools/3.6' -and @($tools.tools|Where-Object {$_.name -eq 'transform_scene'}).Count -eq 1)
 $report.success=$true
}catch{$report.error=$_.Exception.Message}
finally{$report.finishedAt=[DateTimeOffset]::UtcNow.ToString('o');New-Item -ItemType Directory -Path (Split-Path -Parent $ReportPath) -Force|Out-Null;[IO.File]::WriteAllText($ReportPath,($report|ConvertTo-Json -Depth 100),[Text.UTF8Encoding]::new($false))}
if(-not $report.success){throw $report.error}
Write-Host "Windows acceptance passed: $($checks.Count) checks."
