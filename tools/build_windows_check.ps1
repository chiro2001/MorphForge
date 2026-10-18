[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$target = (rustc --print host-tuple).Trim()
if ($target -ne "x86_64-pc-windows-msvc") {
    throw "Windows x64 host required; current target is '$target'."
}

Push-Location $root
try {
    $configPath = Join-Path $root "desktop\src-tauri\tauri.conf.json"
    $config = Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json
    $capabilities = @($config.app.security.capabilities)
    if ($capabilities.Count -ne 1 -or
        @($capabilities[0].permissions).Count -ne 0 -or
        $null -ne $capabilities[0].remote) {
        throw "Desktop must have exactly one local main-window capability with zero permissions and no remote access."
    }
    if ($config.app.withGlobalTauri -ne $false) {
        throw "withGlobalTauri must remain false."
    }
    if (($config.bundle.externalBin -join "|") -ne "binaries/morphforge") {
        throw "Unexpected Tauri sidecar configuration."
    }
    if ($config.productName -ne "MorphForge Desktop") {
        throw "Product name must isolate the install directory from the MorphForge data root."
    }
    $mainSource = Get-Content -LiteralPath (Join-Path $root "desktop\src-tauri\src\main.rs") -Raw
    if ($mainSource -match "invoke_handler|generate_handler!") {
        throw "The desktop shell must not expose Tauri commands to the HTTP page."
    }
    if ($mainSource -notmatch "set_process_group\(true\)" -or
        $mainSource -notmatch "JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE" -or
        $mainSource -notmatch "terminate_core_tree") {
        throw "Windows process-tree cleanup invariants are missing."
    }
    $startupSource = Get-Content -LiteralPath (Join-Path $root "desktop\ui\startup.js") -Raw
    if ($startupSource -match "__TAURI__|invoke\(") {
        throw "The startup page must not call the Tauri API."
    }
    foreach ($required in @(
        "desktop\ui\index.html",
        "desktop\ui\startup.js",
        "desktop\ui\startup.css",
        "desktop\app-icon.svg"
    )) {
        if (-not (Test-Path -LiteralPath (Join-Path $root $required))) {
            throw "Missing desktop asset: $required"
        }
    }
    & (Join-Path $PSScriptRoot "build_windows.ps1") -CheckOnly
    if ($LASTEXITCODE -ne 0) { throw "Windows check build failed." }
} finally {
    Pop-Location
}

Write-Host "MorphForge Windows checks passed. No installer was produced."

