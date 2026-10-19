#requires -Version 7.0
param(
    [Parameter(Mandatory)][string]$DesktopExe,
    [Parameter(Mandatory)][string]$DataDir,
    [int]$RemoteDebugPort = 0
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$localAppData = Join-Path $DataDir "localappdata"
New-Item -ItemType Directory -Path $localAppData -Force | Out-Null
$env:MORPHFORGE_DATA = $DataDir
$env:LOCALAPPDATA = $localAppData
$env:MORPHFORGE_DESKTOP_DISABLE_CAPTURE = "1"
if ($RemoteDebugPort -gt 0) {
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = (
        "--remote-debugging-port=$RemoteDebugPort --remote-allow-origins=*"
    )
}
Start-Process -FilePath $DesktopExe
