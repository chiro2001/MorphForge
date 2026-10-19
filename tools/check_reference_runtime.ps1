#requires -Version 7.0
param([Parameter(Mandatory)][string]$CoreExe,[Parameter(Mandatory)][string]$RepoRoot,[Parameter(Mandatory)][string]$DataDir,[Parameter(Mandatory)][string]$ReportPath)
$ErrorActionPreference='Stop'
$process=$null
$report=[ordered]@{success=$false;scope='Windows release binary; built-in image bytes and credits; no model requests';images=@();error=$null}
try {
 $info=[Diagnostics.ProcessStartInfo]::new($CoreExe);$info.UseShellExecute=$false;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
 foreach($argument in @('serve','--data',$DataDir,'--port','0','--no-capture')){$info.ArgumentList.Add($argument)}
 $process=[Diagnostics.Process]::Start($info);$stderr=$process.StandardError.ReadToEndAsync();$ready=$process.StandardOutput.ReadLineAsync()
 $deadline=[DateTimeOffset]::UtcNow.AddSeconds(60)
 while(-not $ready.IsCompleted -and [DateTimeOffset]::UtcNow -lt $deadline){Start-Sleep -Milliseconds 200}
 if(-not $ready.IsCompleted){throw 'Core startup timed out'}
 $line=$ready.GetAwaiter().GetResult();$match=[regex]::Match($line,'http://127\.0\.0\.1:\d+')
 if(-not $match.Success){throw "Missing ready URL: $line"}
 $base=$match.Value
 $state=Invoke-RestMethod -Uri ($base+'/api/state') -NoProxy
 if($state.connection.enabled){throw 'OSC must be disabled'}
 $catalog=Invoke-RestMethod -Uri ($base+'/api/references') -NoProxy
 if($catalog.images.Count -lt 8 -or $catalog.maxSelected -ne 6){throw 'Built-in catalog contract mismatch'}
 foreach($image in $catalog.images){
  if($image.origin -ne 'builtin'){continue}
  if(-not $image.author -or -not $image.license){throw 'Missing image credits'}
  $hashes=[ordered]@{}
  foreach($kind in @('image','thumbnail')){
   $url=if($kind -eq 'image'){$image.url}else{$image.previewUrl}
   $file=if($kind -eq 'image'){$image.artifact}else{$image.thumbnail}
   $response=Invoke-WebRequest -Uri ($base+$url) -NoProxy
   $actual=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($response.RawContentStream.ToArray())).ToLowerInvariant()
   $expected=(Get-FileHash -LiteralPath (Join-Path (Join-Path $RepoRoot 'references') $file) -Algorithm SHA256).Hash.ToLowerInvariant()
   if($actual -ne $expected){throw "Bundled reference bytes mismatch: $($image.id) $kind"}
   $hashes[$kind]=$actual
  }
  $report.images+=@{id=$image.id;hashes=$hashes;author=$image.author;license=$image.license}
 }
 $report.success=$true
} catch {$report.error=$_.Exception.Message}
finally {
 if($process){if(-not $process.HasExited){$process.Kill($true);$process.WaitForExit()};$process.Dispose()}
 $report.finishedAt=[DateTimeOffset]::UtcNow.ToString('o')
 New-Item -ItemType Directory -Path (Split-Path -Parent $ReportPath) -Force | Out-Null
 [IO.File]::WriteAllText($ReportPath,($report|ConvertTo-Json -Depth 30),[Text.UTF8Encoding]::new($false))
}
if(-not $report.success){throw $report.error}
Write-Host "Release reference image checks passed: $($report.images.Count) images and their thumbnails"
