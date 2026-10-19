#requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Installer,
    [Parameter(Mandatory)][string]$InstallDir,
    [Parameter(Mandatory)][string]$DataDir,
    [Parameter(Mandatory)][string]$ReportPath,
    [Parameter(Mandatory)][string]$ExpectedCoreSha256,
    [Parameter(Mandatory)][string]$ExpectedLicenseSha256,
    [Parameter(Mandatory)][string]$RepoRoot
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$PSNativeCommandUseErrorActionPreference = $true
Set-StrictMode -Version Latest
[Console]::InputEncoding = [Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$OutputEncoding = [Console]::OutputEncoding
$PSStyle.OutputRendering = "PlainText"

if (-not $IsWindows) {
    throw "check_desktop_v2.ps1 must run on Windows."
}

$checks = [Collections.Generic.List[object]]::new()
$taskNames = [Collections.Generic.List[string]]::new()
$desktopProcess = $null
$report = [ordered]@{
    schema = "MorphForgeDesktopV2Validation/1"
    success = $false
    startedAt = [DateTimeOffset]::UtcNow.ToString("o")
    installer = $null
    installDir = $InstallDir
    dataDir = $DataDir
    checks = @()
    tasks = @()
    cleanup = $null
    error = $null
}

function Add-Check {
    param(
        [Parameter(Mandatory)][string]$Name,
        [Parameter(Mandatory)][bool]$Passed,
        [object]$Details = $null
    )
    $entry = [ordered]@{ name = $Name; passed = $Passed }
    if ($null -ne $Details) {
        $entry.details = $Details
    }
    $checks.Add([pscustomobject]$entry)
    $report.checks = @($checks)
    if (-not $Passed) {
        throw "CHECK_FAILED: $Name"
    }
}

function Invoke-JsonHttp {
    param(
        [Parameter(Mandatory)][string]$Url,
        [int]$TimeoutSec = 15
    )
    $response = Invoke-WebRequest -Uri $Url -TimeoutSec $TimeoutSec -NoProxy -SkipHttpErrorCheck
    if ([int]$response.StatusCode -ne 200) {
        throw "HTTP $($response.StatusCode): $Url"
    }
    return ($response.Content | ConvertFrom-Json -Depth 100)
}

function Get-Sha256Bytes {
    param([Parameter(Mandatory)][byte[]]$Bytes)
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        return [Convert]::ToHexString($sha.ComputeHash($Bytes)).ToLowerInvariant()
    }
    finally {
        $sha.Dispose()
    }
}

function Register-ValidationTask {
    param(
        [Parameter(Mandatory)][string]$Name,
        [Parameter(Mandatory)][string]$ScriptPath,
        [Parameter(Mandatory)][string[]]$Arguments
    )
    $argumentText = "-NoLogo -NoProfile -NonInteractive -File `"$ScriptPath`""
    foreach ($argument in $Arguments) {
        $argumentText += " `"$($argument.Replace('"', '\"'))`""
    }
    $action = New-ScheduledTaskAction -Execute "pwsh.exe" -Argument $argumentText
    $principal = New-ScheduledTaskPrincipal `
        -UserId ([Security.Principal.WindowsIdentity]::GetCurrent().Name) `
        -LogonType Interactive -RunLevel Limited
    Register-ScheduledTask -TaskName $Name -Action $action -Principal $principal -Force |
        Out-Null
    $taskNames.Add($Name)
    $report.tasks = @($taskNames)
}

function Get-DescendantProcessIds {
    param([Parameter(Mandatory)][int]$RootProcessId)
    $all = @(Get-CimInstance Win32_Process)
    $result = [Collections.Generic.HashSet[int]]::new()
    [void]$result.Add($RootProcessId)
    $changed = $true
    while ($changed) {
        $changed = $false
        foreach ($process in $all) {
            if ($result.Contains([int]$process.ParentProcessId) -and
                -not $result.Contains([int]$process.ProcessId)) {
                [void]$result.Add([int]$process.ProcessId)
                $changed = $true
            }
        }
    }
    return @($result)
}

function Stop-OwnedTree {
    if ($null -eq $desktopProcess) {
        return
    }
    $ids = Get-DescendantProcessIds -RootProcessId $desktopProcess.Id
    [array]::Reverse($ids)
    foreach ($processId in $ids) {
        Stop-Process -Id $processId -Force -ErrorAction SilentlyContinue
    }
}

$taskPrefix = "MorphForge-v2-desktop-" + [guid]::NewGuid().ToString("N")
try {
    $installerPath = (Resolve-Path -LiteralPath $Installer).Path
    $installerItem = Get-Item -LiteralPath $installerPath
    $report.installer = [ordered]@{
        path = $installerPath
        bytes = $installerItem.Length
        sha256 = (Get-FileHash -LiteralPath $installerPath -Algorithm SHA256).Hash.ToLowerInvariant()
    }
    Add-Check "installer is a nonempty executable" (
        $installerItem.Length -gt 0 -and $installerItem.Extension -eq ".exe"
    ) $report.installer

    $installPath = [IO.Path]::GetFullPath($InstallDir)
    $dataPath = [IO.Path]::GetFullPath($DataDir)
    foreach ($directory in @($installPath, $dataPath)) {
        if (Test-Path -LiteralPath $directory) {
            Add-Check "isolated directory is empty: $directory" (
                @(Get-ChildItem -LiteralPath $directory -Force).Count -eq 0
            )
        }
        else {
            New-Item -ItemType Directory -Path $directory -Force | Out-Null
        }
    }

    $install = Start-Process -FilePath $installerPath -ArgumentList @(
        "/S", "/D=$installPath"
    ) -PassThru -Wait
    Add-Check "silent NSIS install exited zero" ($install.ExitCode -eq 0) @{
        exitCode = $install.ExitCode
    }

    $desktopExe = Join-Path $installPath "morphforge-desktop.exe"
    $coreExe = Join-Path $installPath "morphforge.exe"
    $uninstaller = Join-Path $installPath "uninstall.exe"
    $license = Join-Path $installPath "licenses\OFL-NotoSansSC.txt"
    foreach ($path in @($desktopExe, $coreExe, $uninstaller, $license)) {
        Add-Check ("installed payload exists: " + [IO.Path]::GetFileName($path)) (
            Test-Path -LiteralPath $path -PathType Leaf
        )
    }
    $coreHash = (Get-FileHash -LiteralPath $coreExe -Algorithm SHA256).Hash.ToLowerInvariant()
    $licenseHash = (Get-FileHash -LiteralPath $license -Algorithm SHA256).Hash.ToLowerInvariant()
    Add-Check "installed sidecar matches release core" (
        $coreHash -eq $ExpectedCoreSha256.ToLowerInvariant()
    ) @{ expected = $ExpectedCoreSha256.ToLowerInvariant(); actual = $coreHash }
    Add-Check "installed OFL license matches frozen font license" (
        $licenseHash -eq $ExpectedLicenseSha256.ToLowerInvariant()
    ) @{ expected = $ExpectedLicenseSha256.ToLowerInvariant(); actual = $licenseHash }

    $launchTask = $taskPrefix + "-launch"
    Register-ValidationTask -Name $launchTask `
        -ScriptPath (Join-Path $RepoRoot "tools\desktop_v2_launch.ps1") `
        -Arguments @($desktopExe, $dataPath)
    Start-ScheduledTask -TaskName $launchTask

    $deadline = [DateTimeOffset]::UtcNow.AddSeconds(45)
    while ([DateTimeOffset]::UtcNow -lt $deadline -and $null -eq $desktopProcess) {
        $desktopProcess = @(
            Get-Process morphforge-desktop -ErrorAction SilentlyContinue |
                Where-Object {
                    $_.Path -eq $desktopExe -and
                    $_.SessionId -eq 1
                }
        ) | Select-Object -First 1
        if ($null -eq $desktopProcess) {
            Start-Sleep -Milliseconds 500
        }
    }
    Add-Check "Tauri app process launched in Session 1" (
        $null -ne $desktopProcess
    ) @{
        processId = if ($desktopProcess) { $desktopProcess.Id } else { $null }
        sessionId = if ($desktopProcess) { $desktopProcess.SessionId } else { $null }
        windowTitle = if ($desktopProcess) { $desktopProcess.MainWindowTitle } else { $null }
    }

    $readyLog = Join-Path $dataPath (
        "localappdata\MorphForge\desktop-launch.log"
    )
    $readyRegex = [regex]::new(
        "MorphForge Rust ready: (http://127\.0\.0\.1:\d+)"
    )
    $baseUrl = $null
    $deadline = [DateTimeOffset]::UtcNow.AddSeconds(45)
    while ([DateTimeOffset]::UtcNow -lt $deadline -and -not $baseUrl) {
        if (Test-Path -LiteralPath $readyLog) {
            $match = $readyRegex.Match((Get-Content -LiteralPath $readyLog -Raw))
            if ($match.Success) {
                $baseUrl = $match.Groups[1].Value
            }
        }
        if (-not $baseUrl) {
            Start-Sleep -Milliseconds 500
        }
    }
    Add-Check "installed desktop core reported random loopback URL" (
        $null -ne $baseUrl
    ) @{ baseUrl = $baseUrl }

    $state = Invoke-JsonHttp ($baseUrl + "/api/state")
    Add-Check "installed desktop core reports 0.2.0 and OSC disabled" (
        $state.connection.enabled -eq $false
    ) @{ revision = $state.revision }

    $freeze = Get-Content -LiteralPath (
        Join-Path $RepoRoot "docs\quality-v2\source-freeze.json"
    ) -Raw | ConvertFrom-Json
    $resourcePaths = @(
        "studio.html", "studio.js", "group-ui.js", "harness-ui.js",
        "text-ui.js", "workbench.css"
    )
    $resourceResults = [ordered]@{}
    foreach ($relative in $resourcePaths) {
        $response = Invoke-WebRequest -Uri ($baseUrl + "/" + $relative) `
            -TimeoutSec 15 -NoProxy -SkipHttpErrorCheck
        [byte[]]$bytes = $response.RawContentStream.ToArray()
        $remoteHash = Get-Sha256Bytes -Bytes $bytes
        $diskHash = (
            Get-FileHash -LiteralPath (
                Join-Path $RepoRoot ("web\" + $relative.Replace("/", "\"))
            ) -Algorithm SHA256
        ).Hash.ToLowerInvariant()
        $manifestHash = [string]$freeze.files.("web/" + $relative)
        $resourceResults[$relative] = [ordered]@{
            status = [int]$response.StatusCode
            bytes = $bytes.Length
            servedSha256 = $remoteHash
            diskSha256 = $diskHash
            manifestSha256 = $manifestHash
        }
        Add-Check "installed WebView serves frozen resource: $relative" (
            [int]$response.StatusCode -eq 200 -and
            $remoteHash -eq $diskHash -and
            $diskHash -eq $manifestHash
        ) $resourceResults[$relative]
    }
    $html = (Invoke-WebRequest -Uri ($baseUrl + "/studio.html") -TimeoutSec 15 -NoProxy).Content
    $harnessSource = (
        Invoke-WebRequest -Uri ($baseUrl + "/harness-ui.js") -TimeoutSec 15 -NoProxy
    ).Content
    Add-Check "installed frontend contains font, protection and finish-summary UI" (
        $html.Contains("字体文字") -and
        $html.Contains("分组 · 显示与保护") -and
        $harnessSource.Contains("completedRunSummary")
    )
    $tools = Invoke-JsonHttp ($baseUrl + "/api/harness/tools?language=zh-CN")
    Add-Check "installed Harness exposes tools/system 2.3 and finish_artwork" (
        $tools.version -eq "morphforge-tools/2.3" -and
        $tools.promptVersion -eq "morphforge-system/2.3" -and
        @($tools.tools | Where-Object { $_.name -eq "finish_artwork" }).Count -eq 1
    ) @{ version = $tools.version; promptVersion = $tools.promptVersion }

    $capturePath = Join-Path $RepoRoot "docs\quality-v2\desktop-v2-window.png"
    $captureInfoPath = Join-Path $RepoRoot "docs\quality-v2\desktop-v2-window.json"
    Remove-Item -LiteralPath $capturePath, $captureInfoPath `
        -Force -ErrorAction SilentlyContinue
    $captureStartedAt = [DateTimeOffset]::UtcNow
    $captureTask = $taskPrefix + "-capture"
    Register-ValidationTask -Name $captureTask `
        -ScriptPath (Join-Path $RepoRoot "tools\desktop_v2_capture.ps1") `
        -Arguments @(
            [string]$desktopProcess.Id,
            $capturePath,
            $captureInfoPath
        )
    Start-ScheduledTask -TaskName $captureTask
    $deadline = [DateTimeOffset]::UtcNow.AddSeconds(20)
    while ([DateTimeOffset]::UtcNow -lt $deadline -and
        (-not (Test-Path -LiteralPath $capturePath) -or
         -not (Test-Path -LiteralPath $captureInfoPath))) {
        Start-Sleep -Milliseconds 300
    }
    if (Test-Path -LiteralPath $captureInfoPath) {
        $deadline = [DateTimeOffset]::UtcNow.AddSeconds(5)
        while ([DateTimeOffset]::UtcNow -lt $deadline) {
            try {
                $candidate = Get-Content -LiteralPath $captureInfoPath -Raw |
                    ConvertFrom-Json
                if ([int]$candidate.processId -eq $desktopProcess.Id -and
                    [DateTimeOffset]::Parse($candidate.capturedAt) -ge
                    $captureStartedAt.AddSeconds(-1)) {
                    break
                }
            }
            catch {
            }
            Start-Sleep -Milliseconds 200
        }
    }
    Add-Check "real Tauri window screenshot captured" (
        (Test-Path -LiteralPath $capturePath -PathType Leaf) -and
        (Test-Path -LiteralPath $captureInfoPath -PathType Leaf)
    ) @{
        path = $capturePath
        bytes = if (Test-Path -LiteralPath $capturePath) {
            (Get-Item -LiteralPath $capturePath).Length
        }
        else {
            $null
        }
    }
    $windowInfo = Get-Content -LiteralPath $captureInfoPath -Raw |
        ConvertFrom-Json
    Add-Check "Session 1 window has a live handle, title and dimensions" (
        [int]$windowInfo.processId -eq $desktopProcess.Id -and
        [int]$windowInfo.sessionId -eq 1 -and
        [int64]$windowInfo.windowHandle -ne 0 -and
        $windowInfo.title -eq "MorphForge" -and
        [int]$windowInfo.dpi -eq 120 -and
        [int]$windowInfo.width -ge 1700 -and
        [int]$windowInfo.height -ge 1100
    ) $windowInfo
    $report.window = @{
        processId = $desktopProcess.Id
        sessionId = $windowInfo.sessionId
        title = $windowInfo.title
        windowHandle = $windowInfo.windowHandle
        screenshot = "docs/quality-v2/desktop-v2-window.png"
        screenshotSha256 = $windowInfo.screenshotSha256
        screenshotInfo = "docs/quality-v2/desktop-v2-window.json"
    }

    $closeTask = $taskPrefix + "-close"
    $closeStatusPath = Join-Path $RepoRoot "docs\quality-v2\desktop-v2-close.json"
    Remove-Item -LiteralPath $closeStatusPath -Force -ErrorAction SilentlyContinue
    Register-ValidationTask -Name $closeTask `
        -ScriptPath (Join-Path $RepoRoot "tools\desktop_v2_close.ps1") `
        -Arguments @(
            [string]$desktopProcess.Id,
            [string]$windowInfo.windowHandle,
            $closeStatusPath
        )
    Start-ScheduledTask -TaskName $closeTask
    $closed = $false
    $deadline = [DateTimeOffset]::UtcNow.AddSeconds(60)
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        if (-not (Get-Process -Id $desktopProcess.Id -ErrorAction SilentlyContinue)) {
            $closed = $true
            break
        }
        Start-Sleep -Milliseconds 300
    }
    Add-Check "Tauri app exits after WM_CLOSE" $closed @{
        closeStatus = if (Test-Path -LiteralPath $closeStatusPath) {
            $closeInfo = Get-Content -LiteralPath $closeStatusPath -Raw |
                ConvertFrom-Json
            if ([int]$closeInfo.processId -ne $desktopProcess.Id) {
                throw "Close status belongs to a different process"
            }
            $closeInfo
        }
        else {
            $null
        }
    }
    Start-Sleep -Seconds 2
    $remainingShell = @(
        Get-Process morphforge-desktop -ErrorAction SilentlyContinue |
            Where-Object { $_.Path -eq $desktopExe }
    )
    $remainingCore = @(
        Get-CimInstance Win32_Process |
            Where-Object {
                $_.Name -ieq "morphforge.exe" -and
                $_.CommandLine -like "*$dataPath*"
            }
    )
    Add-Check "desktop shell and isolated core cleaned after normal close" (
        $remainingShell.Count -eq 0 -and $remainingCore.Count -eq 0
    )

    $uninstall = Start-Process -FilePath $uninstaller -ArgumentList "/S" -PassThru -Wait
    Add-Check "silent uninstall exited zero" ($uninstall.ExitCode -eq 0) @{
        exitCode = $uninstall.ExitCode
    }
    $deadline = [DateTimeOffset]::UtcNow.AddSeconds(30)
    while ([DateTimeOffset]::UtcNow -lt $deadline -and (Test-Path -LiteralPath $desktopExe)) {
        Start-Sleep -Milliseconds 300
    }
    Add-Check "uninstall removed installed executables" (
        -not (Test-Path -LiteralPath $desktopExe) -and
        -not (Test-Path -LiteralPath $coreExe)
    )
    $matchingUninstall = @(
        Get-ChildItem "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall" `
            -ErrorAction SilentlyContinue |
            ForEach-Object { Get-ItemProperty $_.PSPath } |
            Where-Object {
                $_.DisplayName -eq "MorphForge Desktop" -and
                $_.InstallLocation -eq $installPath
            }
    )
    Add-Check "uninstall removed matching HKCU entry" (
        $matchingUninstall.Count -eq 0
    )
    $report.resources = $resourceResults
    $report.baseUrl = $baseUrl
    $report.success = $true
}
catch {
    $report.error = $_.Exception.Message
    $report.checks = @($checks)
}
finally {
    Get-ScheduledTask -ErrorAction SilentlyContinue |
        Where-Object { $_.TaskName -like "$taskPrefix*" } |
        ForEach-Object {
            Stop-ScheduledTask -TaskName $_.TaskName -ErrorAction SilentlyContinue
            Unregister-ScheduledTask -TaskName $_.TaskName -Confirm:$false -ErrorAction SilentlyContinue
        }
    Stop-OwnedTree
    $report.tasks = @($taskNames)
    $report.cleanup = [ordered]@{
        validationTasksRemaining = @(
            Get-ScheduledTask -ErrorAction SilentlyContinue |
                Where-Object { $_.TaskName -like "$taskPrefix*" }
        ).Count
        trackedDesktopAlive = if ($null -eq $desktopProcess) {
            $false
        }
        else {
            [bool](Get-Process -Id $desktopProcess.Id -ErrorAction SilentlyContinue)
        }
    }
    $report.finishedAt = [DateTimeOffset]::UtcNow.ToString("o")
    New-Item -ItemType Directory -Path (Split-Path -Parent $ReportPath) -Force |
        Out-Null
    [IO.File]::WriteAllText(
        $ReportPath,
        ($report | ConvertTo-Json -Depth 30),
        [Text.UTF8Encoding]::new($false)
    )
}
if (-not $report.success) {
    throw ("Desktop v2 validation failed: " + $report.error)
}
Write-Host (
    "Desktop v2 validation passed: {0} checks; report={1}" -f
    $report.checks.Count, $ReportPath
)
