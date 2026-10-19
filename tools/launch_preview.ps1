#requires -Version 7.0
param([Parameter(Mandatory)][string]$PreviewRoot)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
try {
    $root=[IO.Path]::GetFullPath($PreviewRoot)
    $current=Get-Content -LiteralPath (Join-Path $root 'current.json') -Raw|ConvertFrom-Json
    $runtime=[IO.Path]::GetFullPath((Join-Path $root $current.runtime))
    $versions=[IO.Path]::GetFullPath((Join-Path $root 'versions'))+[IO.Path]::DirectorySeparatorChar
    if(-not $runtime.StartsWith($versions,[StringComparison]::OrdinalIgnoreCase)){throw 'Invalid preview runtime path.'}
    Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class MorphForgePreviewWindow {
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern bool SetWindowText(IntPtr handle,string text);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr handle,int command);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr handle);
}
'@
    $existing=@(Get-CimInstance Win32_Process|Where-Object {$_.Name -eq 'morphforge-desktop.exe' -and $_.ExecutablePath -and $_.ExecutablePath.StartsWith($versions,[StringComparison]::OrdinalIgnoreCase)})
    if($existing.Count -gt 0){
        $p=Get-Process -Id $existing[0].ProcessId
        if($p.MainWindowHandle -ne [IntPtr]::Zero){
            [MorphForgePreviewWindow]::ShowWindow($p.MainWindowHandle,9)|Out-Null
            [MorphForgePreviewWindow]::SetForegroundWindow($p.MainWindowHandle)|Out-Null
        }
        exit 0
    }
    $previewProfile=Join-Path $root 'Profile'
    $data=Join-Path $root 'Data'
    foreach($path in @($data,(Join-Path $previewProfile 'localappdata'),(Join-Path $previewProfile 'roaming'),(Join-Path $previewProfile 'WebView2'))){New-Item -ItemType Directory -Path $path -Force|Out-Null}
    # All overrides are confined to this launcher and its children.
    $env:MORPHFORGE_DATA=$data
    $env:LOCALAPPDATA=Join-Path $previewProfile 'localappdata'
    $env:APPDATA=Join-Path $previewProfile 'roaming'
    $env:WEBVIEW2_USER_DATA_FOLDER=Join-Path $previewProfile 'WebView2'
    $env:MORPHFORGE_DESKTOP_DISABLE_CAPTURE='0'
    $p=Start-Process -FilePath (Join-Path $runtime 'morphforge-desktop.exe') -WorkingDirectory $runtime -PassThru
    $deadline=[DateTimeOffset]::UtcNow.AddSeconds(20)
    while([DateTimeOffset]::UtcNow -lt $deadline){
        Start-Sleep -Milliseconds 200
        $p.Refresh()
        if($p.HasExited){break}
        if($p.MainWindowHandle -ne [IntPtr]::Zero){
            [MorphForgePreviewWindow]::SetWindowText($p.MainWindowHandle,('MorphForge - 开发测试副本 '+$current.version))|Out-Null
            break
        }
    }
} catch {
    Add-Type -AssemblyName System.Windows.Forms
    [Windows.Forms.MessageBox]::Show($_.Exception.Message,'MorphForge 开发测试副本启动失败')|Out-Null
    exit 1
}
