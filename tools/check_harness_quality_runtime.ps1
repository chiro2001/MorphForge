#requires -Version 7.0
param([Parameter(Mandatory)][string]$CoreExe,[Parameter(Mandatory)][string]$DataDir,[Parameter(Mandatory)][string]$ReportPath)
$ErrorActionPreference='Stop'
$process=$null
$client=$null
$checks=[Collections.Generic.List[object]]::new()
$report=[ordered]@{success=$false;scope='Windows release core and HTTP; real recipe worker, Unicode modules/patch, capture broker PNG fixture, persistent timeline and preflight budget. No paid API/OSC; physical rendering tested separately in Chromium.';checks=$checks;error=$null}
function Check([string]$Name,[bool]$Passed){$checks.Add(@{name=$Name;passed=$Passed});if(-not $Passed){throw $Name}}
try {
 New-Item -ItemType Directory -Path $DataDir -Force | Out-Null
 $empty=Join-Path $DataDir 'no-provider.env'
 [IO.File]::WriteAllText($empty,'# isolated native fixture',[Text.UTF8Encoding]::new($false))
 $info=[Diagnostics.ProcessStartInfo]::new($CoreExe);$info.UseShellExecute=$false;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
 foreach($argument in @('serve','--data',$DataDir,'--port','0','--no-capture','--env-file',$empty)){$info.ArgumentList.Add($argument)}
 $process=[Diagnostics.Process]::Start($info);$stderr=$process.StandardError.ReadToEndAsync();$ready=$process.StandardOutput.ReadLineAsync()
 $deadline=[DateTimeOffset]::UtcNow.AddSeconds(60)
 while(-not $ready.IsCompleted -and [DateTimeOffset]::UtcNow -lt $deadline){Start-Sleep -Milliseconds 200}
 if(-not $ready.IsCompleted){throw 'Core startup timed out'}
 $line=$ready.GetAwaiter().GetResult();$match=[regex]::Match($line,'http://127\.0\.0\.1:\d+');if(-not $match.Success){throw "No ready URL: $line"};$base=$match.Value
 function Api([string]$Path,$Body=$null){if($null -eq $Body){return Invoke-RestMethod -Uri ($base+'/api/'+$Path) -NoProxy};return Invoke-RestMethod -Uri ($base+'/api/'+$Path) -Method Post -ContentType 'application/json; charset=utf-8' -Body ($Body|ConvertTo-Json -Depth 50 -Compress) -NoProxy}
 function Tool([string]$Name,$Arguments){return Api 'harness/tool' @{name=$Name;arguments=$Arguments}}
 $state=Api 'state';Check 'fresh 1M/384K defaults and disabled OSC' ($state.revision -eq 0 -and -not $state.connection.enabled -and $state.harness.settings.contextWindowTokens -eq 1048576 -and $state.harness.settings.maxOutputTokens -eq 393216)
 $null=Api 'provider/config' @{baseUrl='http://127.0.0.1:9';apiType='responses';requiresApiKey=$false;clearKey=$true}
 $helper=Tool 'recipe_sources' @{action='write';path='鲸鱼/parts/body.js';baseRevision=0;content='module.exports=(mf,p)=>mf.ellipsoid([0,8,0],[p.radius,5,3],{style:3});'}
 $null=Tool 'recipe_sources' @{action='write';path='鲸鱼/main.js';baseRevision=0;content="const body=require('./parts/body');module.exports=(mf,p)=>mf.replaceGroup('body',()=>body(mf,p));"}
 $built=Tool 'recipe_sources' @{action='run';path='鲸鱼/main.js';baseRevision=0;mode='replace';params=@{radius=6}}
 Check 'real CommonJS worker executes Unicode relative modules' ($built.revision -eq 1 -and $built.stats.boxCount -gt 0)
 $dry=Tool 'recipe_sources' @{action='run';path='鲸鱼/main.js';baseRevision=1;params=@{radius=7};dryRun=$true;capture=@{views=@('hero')}}
 Check 'dry run skips capture and preserves revision' ($dry.dryRun -and $dry.captureSkipped -eq 'dryRun' -and (Api 'state').revision -eq 1)
 $patch=Tool 'recipe_sources' @{action='patch';path='鲸鱼/parts/body.js';baseRevision=1;expectedHash=$helper.file.hash;edits=@(@{find='p.radius';replace='p.radius+1'})}
 Check 'exact patch returns a new per-file hash' ($patch.file.hash -ne $helper.file.hash)
 $handler=[Net.Http.HttpClientHandler]::new();$handler.UseProxy=$false;$client=[Net.Http.HttpClient]::new($handler);$client.Timeout=[TimeSpan]::FromSeconds(60)
 $runArguments=@{action='run';path='鲸鱼/main.js';baseRevision=1;mode='apply';params=@{radius=6};capture=@{views=@('hero','front');shots=@(@{id='body-detail';projection='orthographic';fit=@{group='body'};yaw=0;pitch=0})}}
 $payload=@{name='recipe_sources';arguments=$runArguments}|ConvertTo-Json -Depth 30 -Compress
 $content=[Net.Http.StringContent]::new($payload,[Text.Encoding]::UTF8,'application/json')
 $pending=$client.PostAsync($base+'/api/harness/tool',$content)
 $job=$null;$deadline=[DateTimeOffset]::UtcNow.AddSeconds(20)
 while($null -eq $job -and [DateTimeOffset]::UtcNow -lt $deadline){$job=Api 'capture/jobs';if($null -eq $job){Start-Sleep -Milliseconds 100}}
 Check 'committed source run exposes one mixed capture job' ($null -ne $job -and $job.views.Count -eq 3 -and $job.revision -eq 2)
 $png='data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a+wAAAABJRU5ErkJggg=='
 $images=@();foreach($view in $job.views){$images+=@{view=$view;data=$png;camera=@{scope='PNG protocol fixture; not physical render'}}}
 $null=Api 'capture/result' @{id=$job.id;revision=$job.revision;images=$images}
 $response=$pending.GetAwaiter().GetResult();$text=$response.Content.ReadAsStringAsync().GetAwaiter().GetResult();if(-not $response.IsSuccessStatusCode){throw $text};$combined=$text|ConvertFrom-Json
 Check 'draw plus capture returns actual committed revision and artifacts' ($combined.revision -eq 2 -and $combined.images.Count -eq 3)
 $slice=Tool 'inspect_region' @{group='body';slice=@{axis='y';index=8};limit=128}
 Check 'slice is one voxel thick' ($slice.sample.Count -gt 0 -and @($slice.sample|Where-Object {$_.min[1] -ne 8 -or $_.size[1] -ne 1}).Count -eq 0)
 $before=Api 'state';$historic=Api 'scene?revision=1';$after=Api 'state'
 Check 'reading old revision does not restore or mutate current draft' ($historic.scene.revision -eq 1 -and $after.revision -eq $before.revision)
 $timeline=Api 'timeline'
 Check 'timeline includes durable source and geometry records' (@($timeline.events|Where-Object {$_.kind -eq 'source.changed'}).Count -gt 0 -and @($timeline.events|Where-Object {$_.kind -eq 'capture.completed'}).Count -eq 1)
 $first=$timeline.events[0];$detail=Api ('timeline/'+$first.seq)
 Check 'timeline detail resolves by durable event sequence' ($detail.seq -eq $first.seq)
 $source=Tool 'recipe_sources' @{action='read';path='鲸鱼/parts/body.js'}
 Check 'module source survives worker runs' ($source.content.Contains('p.radius+1'))
 $null=Api 'harness/config' @{cacheGuard='strict';maxRunTokens=1000;maxSteps=1;maxOutputTokens=512}
 $null=Api 'harness/message' @{prompt='只测试预算，不应触达模型生成请求。'}
 $deadline=[DateTimeOffset]::UtcNow.AddSeconds(20)
 do {$h=Api 'harness/state';if($h.run.status -in @('failed','paused','completed','stopped')){break};Start-Sleep -Milliseconds 100}while([DateTimeOffset]::UtcNow -lt $deadline)
 Check 'input reserve rejects an unaffordable generation before sending' ($h.run.status -eq 'failed' -and $h.run.error.StartsWith('RUN_BUDGET'))
 $timeline=Api 'timeline'
 Check 'no generation request recorded and OSC stays off' (@($timeline.events|Where-Object {$_.kind -eq 'provider.request'}).Count -eq 0 -and -not (Api 'state').connection.enabled)
 $report.success=$true
} catch {$report.error=$_.Exception.Message}
finally {
 if($client){$client.Dispose()}
 if($process){if(-not $process.HasExited){$process.Kill($true);$process.WaitForExit()};$process.Dispose()}
 $report.finishedAt=[DateTimeOffset]::UtcNow.ToString('o')
 New-Item -ItemType Directory -Path (Split-Path -Parent $ReportPath) -Force | Out-Null
 [IO.File]::WriteAllText($ReportPath,($report|ConvertTo-Json -Depth 50),[Text.UTF8Encoding]::new($false))
}
if(-not $report.success){throw $report.error}
Write-Host "Windows harness enhancement checks passed: $($checks.Count)"
