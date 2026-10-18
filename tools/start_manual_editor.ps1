# Start the local manual editor in the signed-in user's session, without an SSH parent.
$ErrorActionPreference = 'Stop'
$projectRoot = 'D:\Projects\VRChatProjects2\MorphForge'
if (-not (Test-Path -LiteralPath (Join-Path $projectRoot 'web\index.html'))) { throw 'Deploy web sources first.' }
$taskName = 'MorphForge-ManualEditor'
$scriptPath = Join-Path $projectRoot 'tools\editor_server.py'
$running = @(Get-CimInstance Win32_Process | Where-Object {
    $_.Name -match '^python(w)?(3([.]\d+)?)?[.]exe$' -and $_.CommandLine -and
    ($_.CommandLine.Contains($scriptPath) -or $_.CommandLine.Contains('tools/editor_server.py'))
})
if ($running.Count -gt 0) {
    [pscustomobject]@{started=$false; processIds=@($running.ProcessId); url='http://127.0.0.1:8765'} | ConvertTo-Json -Compress
    exit 0
}
$pythonExe = (Get-Command python.exe -ErrorAction Stop).Source
$logDir = Join-Path $projectRoot 'Logs'
New-Item -ItemType Directory -Path $logDir -Force | Out-Null
# PowerShell redirects output so diagnostics remain available without opening a terminal.
$launcher = Join-Path $env:USERPROFILE 'tmp\morphforge\manual-editor-launch.ps1'
New-Item -ItemType Directory -Path (Split-Path $launcher) -Force | Out-Null
$body = "Set-Location -LiteralPath '$($projectRoot.Replace("'","''"))'`n& '$($pythonExe.Replace("'","''"))' -u '$($scriptPath.Replace("'","''"))' --listen 127.0.0.1 --port 8765 *> '$($logDir.Replace("'","''"))\manual-editor.log'"
[IO.File]::WriteAllText($launcher,$body,[Text.UTF8Encoding]::new($false))
$pwshExe=(Get-Command pwsh.exe -ErrorAction Stop).Source
$arguments='-NoLogo -NoProfile -NonInteractive -WindowStyle Hidden -File "'+$launcher+'"'
$action=New-ScheduledTaskAction -Execute $pwshExe -Argument $arguments -WorkingDirectory $projectRoot
$user=[Security.Principal.WindowsIdentity]::GetCurrent().Name
$principal=New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Limited
$settings=New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::Zero) -MultipleInstances IgnoreNew -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
$registered=Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue
if ($registered) {
    if (@($registered.Actions | Where-Object {$_.Execute -eq $pwshExe -and $_.Arguments -eq $arguments}).Count -ne 1) { throw 'Launcher task name is already used by another action.' }
} else { Register-ScheduledTask -TaskName $taskName -Action $action -Principal $principal -Settings $settings | Out-Null }
# No trigger: only explicit start; no automatic restart, startup hook or network exposure.
Start-ScheduledTask -TaskName $taskName
[pscustomobject]@{started=$true; task=$taskName; url='http://127.0.0.1:8765'; log=(Join-Path $logDir 'manual-editor.log')} | ConvertTo-Json -Compress
