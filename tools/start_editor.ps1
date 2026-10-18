# Start a GUI Editor in the logged-in desktop, independent of the SSH session.
$ErrorActionPreference = 'Stop'
$projectRoot = 'D:\Projects\VRChatProjects2\MorphForge'
$editorExe = 'D:\Software\Unity\2022.3.22f1\Editor\Unity.exe'
$existing = @(Get-CimInstance Win32_Process -Filter "Name='Unity.exe'" | Where-Object {
    $_.ExecutablePath -eq $editorExe -and
    $_.CommandLine.Replace('/','\').IndexOf($projectRoot,[StringComparison]::OrdinalIgnoreCase) -ge 0
})
if ($existing.Count -gt 0) {
    [pscustomobject]@{started=$false; processIds=@($existing.ProcessId); project=$projectRoot} | ConvertTo-Json -Compress
    exit 0
}

$taskName = 'MorphForge-UnityEditor'
$logFile = Join-Path $projectRoot 'Logs\MorphForge-Editor.log'
New-Item -ItemType Directory -Path (Split-Path -Parent $logFile) -Force | Out-Null
$arguments = '-projectPath "' + $projectRoot + '" -logFile "' + $logFile + '"'
$action = New-ScheduledTaskAction -Execute $editorExe -Argument $arguments -WorkingDirectory $projectRoot
$user = [Security.Principal.WindowsIdentity]::GetCurrent().Name
$principal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::Zero) -MultipleInstances IgnoreNew -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
$registered = Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue
if ($registered) {
    $matches = @($registered.Actions | Where-Object { $_.Execute -eq $editorExe -and $_.Arguments -eq $arguments })
    if ($matches.Count -ne 1) { throw 'A different task already uses the MorphForge launcher name.' }
} else {
    Register-ScheduledTask -TaskName $taskName -Action $action -Principal $principal -Settings $settings | Out-Null
}
# No trigger: the task only starts on explicit request and never relaunches on a timer.
Start-ScheduledTask -TaskName $taskName
[pscustomobject]@{started=$true; launcherTask=$taskName; project=$projectRoot; interactiveUser=$user; log=$logFile} | ConvertTo-Json -Compress
