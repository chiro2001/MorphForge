#requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Installer,
    [Parameter(Mandatory)][string]$ExpectedVersion,
    [Parameter(Mandatory)][string]$ExpectedCoreHash,
    [Parameter(Mandatory)][string]$ExpectedShellHash,
    [string]$ReportPath='',
    [string]$BackupRoot='',
    [switch]$CloseRunning,
    [switch]$VerifyOnly
)
$ErrorActionPreference='Stop'
$ProgressPreference='SilentlyContinue'
Set-StrictMode -Version Latest
$root=Join-Path $env:LOCALAPPDATA 'MorphForge Desktop'
$data=Join-Path $env:LOCALAPPDATA 'MorphForge'
$backup=if($BackupRoot){[IO.Path]::GetFullPath($BackupRoot)}else{Join-Path $env:USERPROFILE ('tmp\morphforge\main-backup-'+[DateTimeOffset]::Now.ToString('yyyyMMdd-HHmmss')+'-'+[guid]::NewGuid().ToString('N').Substring(0,8))}
$checks=[Collections.Generic.List[object]]::new()
$report=[ordered]@{success=$false;checks=$checks;installRoot=$root;dataDir=$data;backupRoot=$backup;error=$null}
function Check([string]$Name,[bool]$Pass){$checks.Add(@{name=$Name;passed=$Pass});if(-not $Pass){throw $Name}}
function Running {
    @(Get-CimInstance Win32_Process|Where-Object {$_.Name -in @('morphforge.exe','morphforge-desktop.exe') -and $_.ExecutablePath -and $_.ExecutablePath.StartsWith($root+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)})
}
function DataFingerprint {
    $manifest=[ordered]@{};$bytes=0L
    if(Test-Path -LiteralPath $data){
        foreach($f in Get-ChildItem -LiteralPath $data -File -Recurse|Sort-Object FullName){$manifest[[IO.Path]::GetRelativePath($data,$f.FullName)]=(Get-FileHash -LiteralPath $f.FullName -Algorithm SHA256).Hash.ToLowerInvariant();$bytes+=$f.Length}
    }
    $encoded=[Text.Encoding]::UTF8.GetBytes(($manifest|ConvertTo-Json -Compress))
    @{sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($encoded)).ToLowerInvariant();files=$manifest.Count;bytes=$bytes}
}
try {
    Check 'backup is outside the installation and data' (-not @($root,$data).Where({$backup -eq $_ -or $backup.StartsWith($_+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)}).Count)
    Check 'installer exists' (Test-Path -LiteralPath $Installer -PathType Leaf)
    $report.installerSha256=(Get-FileHash -LiteralPath $Installer -Algorithm SHA256).Hash.ToLowerInvariant()
    if($VerifyOnly){
        $previous=Get-Content -LiteralPath $ReportPath -Raw|ConvertFrom-Json
        Check 'previous installer completed and backup is available' ($previous.installerExitCode -eq 0 -and (Test-Path -LiteralPath (Join-Path $previous.backupRoot 'Data')) -and $previous.installerSha256 -eq $report.installerSha256)
        $report.backupRoot=$previous.backupRoot
        $report.installerExitCode=$previous.installerExitCode
        $before=$previous.dataBefore
        $report.dataBefore=$before
    } else {
    if(@(Running).Count -gt 0 -and $CloseRunning){
        foreach($p in (Running|Where-Object {$_.Name -eq 'morphforge-desktop.exe'})){
            # Graceful native close only; never force-kill a model/recipe job.
            $proc=Get-Process -Id $p.ProcessId -ErrorAction SilentlyContinue
            if($proc){$proc.CloseMainWindow()|Out-Null}
        }
        $deadline=[DateTimeOffset]::UtcNow.AddSeconds(20)
        while(@(Running).Count -gt 0 -and [DateTimeOffset]::UtcNow -lt $deadline){Start-Sleep -Milliseconds 200}
    }
    Check 'main installation is closed before backup and replacement' (@(Running).Count -eq 0)
    New-Item -ItemType Directory -Path $backup -Force|Out-Null
    $before=DataFingerprint
    if(Test-Path -LiteralPath $root){Copy-Item -LiteralPath $root -Destination (Join-Path $backup 'Installation') -Recurse}
    if(Test-Path -LiteralPath $data){Copy-Item -LiteralPath $data -Destination (Join-Path $backup 'Data') -Recurse}
    $report.dataBefore=$before
    Check 'main installation remains closed after backup' (@(Running).Count -eq 0)
    $install=Start-Process -FilePath ([IO.Path]::GetFullPath($Installer)) -ArgumentList '/S' -Wait -PassThru
    $report.installerExitCode=$install.ExitCode
    Check 'official installer exits successfully' ($install.ExitCode -eq 0)
    }
    $version=(& (Join-Path $root 'morphforge.exe') --version|Out-String).Trim()
    Check 'installed core reports expected version' ($LASTEXITCODE -eq 0 -and $version -eq ('morphforge '+$ExpectedVersion))
    $coreHash=(Get-FileHash -LiteralPath (Join-Path $root 'morphforge.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
    $shellHash=(Get-FileHash -LiteralPath (Join-Path $root 'morphforge-desktop.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
    Check 'installed core matches verified release payload' ($coreHash -eq $ExpectedCoreHash.ToLowerInvariant())
    Check 'installed shell matches verified release payload' ($shellHash -eq $ExpectedShellHash.ToLowerInvariant())
    $after=DataFingerprint;$report.dataAfter=$after
    Check 'original data files are byte-for-byte unchanged by installation' ($before.sha256 -eq $after.sha256 -and $before.files -eq $after.files)
    $registration=Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\MorphForge Desktop'
    Check 'Windows installed-app registration reports expected version' ($registration.DisplayVersion -eq $ExpectedVersion)
    $shell=New-Object -ComObject WScript.Shell
    foreach($dir in @([Environment]::GetFolderPath('Desktop'),(Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'))){
        $path=Join-Path $dir 'MorphForge Desktop.lnk'
        Check ('main entry exists: '+$path) (Test-Path -LiteralPath $path)
        $link=$shell.CreateShortcut($path)
        Check ('main entry targets installed shell: '+$path) ($link.TargetPath -eq (Join-Path $root 'morphforge-desktop.exe'))
    }
    $report.version=$ExpectedVersion;$report.coreSha256=$coreHash;$report.shellSha256=$shellHash;$report.success=$true
} catch {$report.error=$_.Exception.Message}
finally {
    $report.finishedAt=[DateTimeOffset]::UtcNow.ToString('o')
    if(-not $ReportPath){$ReportPath=Join-Path $backup 'install-report.json'}
    New-Item -ItemType Directory -Path (Split-Path -Parent $ReportPath) -Force|Out-Null
    [IO.File]::WriteAllText($ReportPath,($report|ConvertTo-Json -Depth 10),[Text.Encoding]::UTF8)
}
if(-not $report.success){throw $report.error}
Write-Output "Main installation updated to $ExpectedVersion; original data preserved."
