#requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$RepoRoot,
    [Parameter(Mandatory)][string]$RuntimeDir,
    [Parameter(Mandatory)][string]$DataDir,
    [Parameter(Mandatory)][string]$ReportPath
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$checks = [Collections.Generic.List[object]]::new()
$tasks = [Collections.Generic.List[string]]::new()
$desktopProcess = $null
$prefix = 'MorphForge-candidate-' + [guid]::NewGuid().ToString('N')
$report = [ordered]@{ success=$false; scope='Isolated unpackaged Tauri runtime; no installer or user-install changes'; checks=@(); error=$null }
function Check([string]$Name,[bool]$Passed) {
    $checks.Add(@{name=$Name;passed=$Passed})
    if (-not $Passed) { throw "CHECK_FAILED: $Name" }
}
function Api([string]$Path,$Body=$null) {
    if ($null -eq $Body) { return Invoke-RestMethod -Uri ($script:baseUrl + $Path) -NoProxy }
    Invoke-RestMethod -Uri ($script:baseUrl + $Path) -Method Post -ContentType 'application/json' -Body ([Text.Encoding]::UTF8.GetBytes(($Body | ConvertTo-Json -Depth 50 -Compress))) -NoProxy
}
function Task([string]$Suffix,[string]$Script,[string[]]$Arguments) {
    $name = "$prefix-$Suffix"
    $argsText = '-NoLogo -NoProfile -NonInteractive -File "' + $Script + '"'
    foreach ($argument in $Arguments) { $argsText += ' "' + $argument.Replace('"','\"') + '"' }
    $action = New-ScheduledTaskAction -Execute 'pwsh.exe' -Argument $argsText
    $principal = New-ScheduledTaskPrincipal -UserId ([Security.Principal.WindowsIdentity]::GetCurrent().Name) -LogonType Interactive -RunLevel Limited
    Register-ScheduledTask -TaskName $name -Action $action -Principal $principal -Force | Out-Null
    $tasks.Add($name)
    Start-ScheduledTask -TaskName $name
}
try {
    foreach ($dir in @($RuntimeDir,$DataDir)) {
        Check "isolated directory is new: $dir" (-not (Test-Path -LiteralPath $dir))
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
    $release = Join-Path $RepoRoot 'desktop\target\x86_64-pc-windows-msvc\release'
    foreach ($name in @('morphforge-desktop.exe','morphforge.exe')) {
        Copy-Item -LiteralPath (Join-Path $release $name) -Destination (Join-Path $RuntimeDir $name)
    }
    $exe = Join-Path $RuntimeDir 'morphforge-desktop.exe'
    Task 'launch' (Join-Path $RepoRoot 'tools\desktop_v2_launch.ps1') @($exe,$DataDir)
    $log = Join-Path $DataDir 'localappdata\MorphForge\desktop-launch.log'
    $deadline = [DateTimeOffset]::UtcNow.AddSeconds(60)
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        $desktopProcess = Get-Process morphforge-desktop -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe } | Select-Object -First 1
        if (Test-Path -LiteralPath $log) {
            $match = [regex]::Match((Get-Content -LiteralPath $log -Raw),'MorphForge Rust ready: (http://127\.0\.0\.1:\d+)')
            if ($match.Success) { $script:baseUrl=$match.Groups[1].Value; break }
        }
        Start-Sleep -Milliseconds 300
    }
    Check 'isolated Tauri process and loopback core started' ($null -ne $desktopProcess -and $null -ne $script:baseUrl)
    $state = Api '/api/state'
    Check 'candidate version 0.3.0 and OSC disabled' ($state.software.version -eq '0.3.0' -and -not $state.connection.enabled)
    $config = Api '/api/harness/config' @{preset='detailed'}
    Check 'detailed preset applies complete budgets and Goal' ($config.settings.maxSteps -eq 128 -and $config.settings.goalMode -and $config.settings.goalMinReviewRounds -eq 3)
    $connection = Api '/api/provider/config' @{baseUrl='http://127.0.0.1:17777';apiType='chat';apiKey='local-candidate-fixture';sendReasoning=$false}
    Check 'connection config saves without returning key' ($connection.apiType -eq 'chat' -and $connection.keyConfigured -and -not (($connection|ConvertTo-Json) -match 'local-candidate-fixture'))
    Check 'private config stored only in isolated data directory' (Test-Path -LiteralPath (Join-Path $DataDir 'provider-connection.json'))
    $html = (Invoke-WebRequest -Uri ($script:baseUrl+'/studio.html') -NoProxy).Content
    Check 'packaged UI includes preset, Goal and provider settings' ($html.Contains('id="preset"') -and $html.Contains('id="goalMode"') -and $html.Contains('id="providerKey"'))
    $tools = Api '/api/harness/tools'
    Check 'runtime exposes final prompt and tools versions' ($tools.version -eq 'morphforge-tools/3.0' -and $tools.promptVersion -eq 'morphforge-system/3.1')
    $outDir = Split-Path -Parent $ReportPath
    New-Item -ItemType Directory -Path $outDir -Force | Out-Null
    $png = Join-Path $outDir 'desktop-candidate.png'
    $info = Join-Path $outDir 'desktop-candidate-window.json'
    Task 'capture' (Join-Path $RepoRoot 'tools\desktop_v2_capture.ps1') @([string]$desktopProcess.Id,$png,$info)
    $deadline = [DateTimeOffset]::UtcNow.AddSeconds(25)
    while ([DateTimeOffset]::UtcNow -lt $deadline -and -not (Test-Path -LiteralPath $info)) { Start-Sleep -Milliseconds 300 }
    Check 'real native window captured' ((Test-Path -LiteralPath $png) -and (Test-Path -LiteralPath $info))
    $window = Get-Content -LiteralPath $info -Raw | ConvertFrom-Json
    Check 'window belongs to isolated process and has usable size' ($window.processId -eq $desktopProcess.Id -and $window.title -eq 'MorphForge' -and $window.width -ge 600 -and $window.height -ge 400)
    $closeInfo = Join-Path $outDir 'desktop-candidate-close.json'
    Task 'close' (Join-Path $RepoRoot 'tools\desktop_v2_close.ps1') @([string]$desktopProcess.Id,[string]$window.windowHandle,$closeInfo)
    $deadline = [DateTimeOffset]::UtcNow.AddSeconds(45)
    while ([DateTimeOffset]::UtcNow -lt $deadline -and (Get-Process -Id $desktopProcess.Id -ErrorAction SilentlyContinue)) { Start-Sleep -Milliseconds 300 }
    Check 'WM_CLOSE exits only the isolated shell' (-not (Get-Process -Id $desktopProcess.Id -ErrorAction SilentlyContinue))
    Start-Sleep -Seconds 2
    $cores = @(Get-CimInstance Win32_Process | Where-Object { $_.Name -ieq 'morphforge.exe' -and $_.CommandLine -like "*$DataDir*" })
    Check 'isolated sidecar cleaned after exit' ($cores.Count -eq 0)
    $report.success=$true
    $report.runtimeDir=$RuntimeDir
    $report.dataDir=$DataDir
    $report.window=$window
} catch { $report.error=$_.Exception.Message }
finally {
    foreach ($name in $tasks) {
        Stop-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue
        Unregister-ScheduledTask -TaskName $name -Confirm:$false -ErrorAction SilentlyContinue
    }
    if ($desktopProcess -and (Get-Process -Id $desktopProcess.Id -ErrorAction SilentlyContinue)) {
        & taskkill /PID $desktopProcess.Id /T /F 2>$null | Out-Null
    }
    $report.checks=@($checks)
    $report.finishedAt=[DateTimeOffset]::UtcNow.ToString('o')
    New-Item -ItemType Directory -Path (Split-Path -Parent $ReportPath) -Force | Out-Null
    [IO.File]::WriteAllText($ReportPath,($report|ConvertTo-Json -Depth 50),[Text.UTF8Encoding]::new($false))
}
if (-not $report.success) { throw $report.error }
Write-Host "Desktop candidate passed: $($checks.Count) checks"
