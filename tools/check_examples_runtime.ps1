#requires -Version 7.0
param([Parameter(Mandatory)][string]$CoreExe,[Parameter(Mandatory)][string]$RepoRoot,[Parameter(Mandatory)][string]$DataDir,[Parameter(Mandatory)][string]$ReportPath)
$ErrorActionPreference='Stop'
$process=$null
$report=[ordered]@{success=$false;scope='Windows release binary; six embedded artworks, image bytes and editable fork; no model requests or OSC';examples=@();error=$null}
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
 if($state.connection.enabled -or $state.revision -ne 0){throw 'Requires isolated empty project with OSC disabled'}
 if($state.harness.settings.contextWindowTokens -ne 1048576 -or $state.harness.settings.maxOutputTokens -ne 393216){throw 'Large-window defaults mismatch'}
 $manifest=Get-Content -LiteralPath (Join-Path $RepoRoot 'examples/manifest.json') -Raw | ConvertFrom-Json
 $catalog=Invoke-RestMethod -Uri ($base+'/api/artworks') -NoProxy
 if($catalog.examples.Count -ne 6 -or $catalog.artworks.Count -ne 0){throw 'Offline library contract mismatch'}
 foreach($entry in $manifest.examples){
  $example=Invoke-RestMethod -Uri ($base+'/api/artworks/'+$entry.id) -NoProxy
  if($example.project.scene.boxes.Count -ne $entry.artwork.boxCount -or $example.project.sourceCredits.Count -lt 1){throw "Invalid example $($entry.id)"}
  $expected=Get-Content -LiteralPath (Join-Path $RepoRoot ('examples/'+$entry.projectFile)) -Raw | ConvertFrom-Json
  if(($example.project.scene|ConvertTo-Json -Depth 100 -Compress) -cne ($expected.project.scene|ConvertTo-Json -Depth 100 -Compress)){throw 'Embedded geometry differs from frozen source'}
  $images=@()
  for($i=0;$i -lt 3;$i++){
   $artifact=$example.artwork.views[$i]
   $response=Invoke-WebRequest -Uri ($base+'/artifacts/'+$artifact) -NoProxy
   $actual=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($response.RawContentStream.ToArray())).ToLowerInvariant()
   if($actual -ne $entry.images[$i].sha256){throw 'Embedded artwork image mismatch'}
   $images+=@{view=$entry.images[$i].view;sha256=$actual}
  }
  $report.examples+=@{id=$entry.id;boxCount=$example.project.scene.boxes.Count;images=$images;sourceCredits=$example.project.sourceCredits}
 }
 $state=Invoke-RestMethod -Uri ($base+'/api/state') -NoProxy
 if($state.revision -ne 0){throw 'Browsing modified current project'}
 $body=@{action='fork';id='builtin:whale-maid';baseRevision=0}|ConvertTo-Json
 $fork=Invoke-RestMethod -Uri ($base+'/api/artworks') -Method Post -ContentType 'application/json' -Body $body -NoProxy
 $state=Invoke-RestMethod -Uri ($base+'/api/state') -NoProxy
 if($state.stats.boxCount -ne 223 -or $state.revision -ne 1 -or $state.connection.enabled){throw 'Editable fork contract mismatch'}
 $original=Invoke-RestMethod -Uri ($base+'/api/artworks/builtin:whale-maid') -NoProxy
 if($original.project.scene.boxes.Count -ne 223){throw 'Builtin original changed'}
 $report.success=$true
} catch {$report.error=$_.Exception.Message}
finally {
 if($process){if(-not $process.HasExited){$process.Kill($true);$process.WaitForExit()};$process.Dispose()}
 $report.finishedAt=[DateTimeOffset]::UtcNow.ToString('o')
 New-Item -ItemType Directory -Path (Split-Path -Parent $ReportPath) -Force | Out-Null
 [IO.File]::WriteAllText($ReportPath,($report|ConvertTo-Json -Depth 50),[Text.UTF8Encoding]::new($false))
}
if(-not $report.success){throw $report.error}
Write-Host "Release artwork checks passed: $($report.examples.Count) examples and 18 images"
