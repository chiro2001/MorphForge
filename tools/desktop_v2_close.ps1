#requires -Version 7.0
param(
    [Parameter(Mandatory)][int]$ProcessId,
    [Parameter(Mandatory)][int64]$WindowHandle,
    [Parameter(Mandatory)][string]$StatusPath
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class MorphForgeV2Close {
    [DllImport("user32.dll")]
    public static extern bool PostMessage(IntPtr window, uint message, IntPtr wParam, IntPtr lParam);
}
"@

$process = Get-Process -Id $ProcessId -ErrorAction Stop
$window = [IntPtr]::new($WindowHandle)
$posted = [MorphForgeV2Close]::PostMessage(
    $window,
    0x0010,
    [IntPtr]::Zero,
    [IntPtr]::Zero
)
$lastError = [Runtime.InteropServices.Marshal]::GetLastWin32Error()
$closeMain = if (-not $posted) {
    $process.CloseMainWindow()
}
else {
    $false
}
$status = [ordered]@{
    processId = $ProcessId
    windowHandle = $WindowHandle
    postMessage = $posted
    lastWin32Error = $lastError
    closeMainWindow = $closeMain
    sessionId = $process.SessionId
    time = [DateTimeOffset]::UtcNow.ToString("o")
}
New-Item -ItemType Directory -Path (Split-Path -Parent $StatusPath) -Force |
    Out-Null
[IO.File]::WriteAllText(
    $StatusPath,
    ($status | ConvertTo-Json -Depth 10),
    [Text.UTF8Encoding]::new($false)
)
if (-not $posted) {
    throw "PostMessage WM_CLOSE failed with Win32 error $lastError"
}
