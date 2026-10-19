#requires -Version 7.0
param(
    [Parameter(Mandatory)][int]$ProcessId,
    [Parameter(Mandatory)][string]$OutputPath,
    [Parameter(Mandatory)][string]$InfoPath
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class MorphForgeV2Capture {
    public delegate bool EnumWindowsProc(IntPtr window, IntPtr parameter);
    [DllImport("user32.dll")]
    public static extern bool EnumWindows(EnumWindowsProc callback, IntPtr parameter);
    [DllImport("user32.dll")]
    public static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);
    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")]
    public static extern int GetWindowTextLength(IntPtr window);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)]
    public static extern int GetWindowText(IntPtr window, StringBuilder text, int count);
    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr window, out RECT rect);
    [DllImport("user32.dll")]
    public static extern uint GetDpiForWindow(IntPtr window);
    [DllImport("user32.dll")]
    public static extern bool SetProcessDpiAwarenessContext(IntPtr context);
    [DllImport("user32.dll")]
    public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    [DllImport("user32.dll")]
    public static extern bool PrintWindow(IntPtr window, IntPtr deviceContext, uint flags);
    [DllImport("dwmapi.dll")]
    public static extern int DwmGetWindowAttribute(
        IntPtr window,
        int attribute,
        out RECT value,
        int valueSize
    );
    public struct RECT {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }
    public static IntPtr LargestVisibleWindow(uint targetProcessId) {
        IntPtr best = IntPtr.Zero;
        long bestArea = 0;
        EnumWindows(delegate(IntPtr window, IntPtr parameter) {
            uint owner;
            GetWindowThreadProcessId(window, out owner);
            if (owner != targetProcessId || !IsWindowVisible(window)) {
                return true;
            }
            RECT rect;
            if (!GetWindowRect(window, out rect)) {
                return true;
            }
            long area = (long)(rect.Right - rect.Left) * (rect.Bottom - rect.Top);
            if (area > bestArea) {
                best = window;
                bestArea = area;
            }
            return true;
        }, IntPtr.Zero);
        return best;
    }
    public static string WindowTitle(IntPtr window) {
        int length = GetWindowTextLength(window);
        var text = new StringBuilder(length + 1);
        GetWindowText(window, text, text.Capacity);
        return text.ToString();
    }
}
"@

$dpiContext = [IntPtr]::new(-4)
$processDpiResult = [MorphForgeV2Capture]::SetProcessDpiAwarenessContext(
    $dpiContext
)
$previousThreadContext = [MorphForgeV2Capture]::SetThreadDpiAwarenessContext(
    $dpiContext
)
$process = Get-Process -Id $ProcessId -ErrorAction Stop
$window = [MorphForgeV2Capture]::LargestVisibleWindow([uint32]$ProcessId)
if ($window -eq [IntPtr]::Zero) {
    throw "No visible window found for process $ProcessId"
}
$windowRect = New-Object MorphForgeV2Capture+RECT
if (-not [MorphForgeV2Capture]::GetWindowRect($window, [ref]$windowRect)) {
    throw "GetWindowRect failed"
}
$frameRect = New-Object MorphForgeV2Capture+RECT
$dwmHr = [MorphForgeV2Capture]::DwmGetWindowAttribute(
    $window,
    9,
    [ref]$frameRect,
    [Runtime.InteropServices.Marshal]::SizeOf($frameRect)
)
if ($dwmHr -eq 0) {
    $screenshotRect = $frameRect
    $boundsSource = "dwm-extended-frame"
}
else {
    $screenshotRect = $windowRect
    $boundsSource = "get-window-rect"
}
$width = $screenshotRect.Right - $screenshotRect.Left
$height = $screenshotRect.Bottom - $screenshotRect.Top
if ($width -le 0 -or $height -le 0) {
    throw "Invalid window dimensions"
}

$bitmap = [Drawing.Bitmap]::new($width, $height)
$graphics = [Drawing.Graphics]::FromImage($bitmap)
try {
    $context = $graphics.GetHdc()
    try {
        if (-not [MorphForgeV2Capture]::PrintWindow($window, $context, 2)) {
            throw "PrintWindow failed"
        }
    }
    finally {
        $graphics.ReleaseHdc($context)
    }
    New-Item -ItemType Directory -Path (Split-Path -Parent $OutputPath) -Force | Out-Null
    $bitmap.Save($OutputPath, [Drawing.Imaging.ImageFormat]::Png)
}
finally {
    $graphics.Dispose()
    $bitmap.Dispose()
}

$info = [ordered]@{
    processId = $ProcessId
    sessionId = $process.SessionId
    windowHandle = $window.ToInt64()
    title = [MorphForgeV2Capture]::WindowTitle($window)
    dpi = [MorphForgeV2Capture]::GetDpiForWindow($window)
    processDpiAwarenessSet = $processDpiResult
    previousThreadDpiAwarenessContext = $previousThreadContext.ToInt64()
    boundsSource = $boundsSource
    dwmResult = $dwmHr
    windowRect = [ordered]@{
        left = $windowRect.Left
        top = $windowRect.Top
        right = $windowRect.Right
        bottom = $windowRect.Bottom
        width = $windowRect.Right - $windowRect.Left
        height = $windowRect.Bottom - $windowRect.Top
    }
    screenshotRect = [ordered]@{
        left = $screenshotRect.Left
        top = $screenshotRect.Top
        right = $screenshotRect.Right
        bottom = $screenshotRect.Bottom
    }
    width = $width
    height = $height
    screenshot = $OutputPath
    screenshotBytes = (Get-Item -LiteralPath $OutputPath).Length
    screenshotSha256 = (
        Get-FileHash -LiteralPath $OutputPath -Algorithm SHA256
    ).Hash.ToLowerInvariant()
    capturedAt = [DateTimeOffset]::UtcNow.ToString("o")
}
New-Item -ItemType Directory -Path (Split-Path -Parent $InfoPath) -Force |
    Out-Null
[IO.File]::WriteAllText(
    $InfoPath,
    ($info | ConvertTo-Json -Depth 10),
    [Text.UTF8Encoding]::new($false)
)
