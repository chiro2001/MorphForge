[CmdletBinding()]
param(
    [ValidateSet("x86_64-pc-windows-msvc")]
    [string]$Target = "x86_64-pc-windows-msvc",
    [switch]$CheckOnly,
    [switch]$SkipBundle,
    [switch]$ReuseSidecar
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$root = Split-Path -Parent $PSScriptRoot
$coreManifest = Join-Path $root "Cargo.toml"
$desktop = Join-Path $root "desktop"
$tauriDir = Join-Path $desktop "src-tauri"
$sidecarDir = Join-Path $tauriDir "binaries"
$sidecar = Join-Path $sidecarDir "morphforge-$Target.exe"
$coreExe = Join-Path $root "target\$Target\release\morphforge.exe"
$iconSource = Join-Path $desktop "app-icon.svg"
$iconDir = Join-Path $tauriDir "icons"

function Require-Command([string]$Name) {
    $command = Get-Command $Name -ErrorAction SilentlyContinue
    if (-not $command) {
        throw "Missing required command '$Name'. Install the documented Windows prerequisites; this script does not install dependencies."
    }
    return $command
}

function Invoke-Checked([string]$File, [string[]]$Arguments, [string]$WorkingDirectory) {
    Write-Host "> $File $($Arguments -join ' ')"
    Push-Location $WorkingDirectory
    try {
        & $File @Arguments
        if ($LASTEXITCODE -ne 0) {
            throw "Command failed with exit code $LASTEXITCODE`: $File $($Arguments -join ' ')"
        }
    } finally {
        Pop-Location
    }
}

$cargo = Require-Command "cargo"
$rustc = Require-Command "rustc"
$tauri = Require-Command "tauri"
$hostTriple = (& $rustc.Source --print host-tuple).Trim()
if ($hostTriple -ne $Target) {
    throw "This script must run on a $Target host. Current host is '$hostTriple'."
}

$tauriProbe = & $tauri.Source --version 2>$null
if ($LASTEXITCODE -ne 0) {
    throw "Tauri CLI v2 is missing. Install it once with: cargo install tauri-cli --version '^2' --locked"
}
Write-Host "Tauri CLI: $tauriProbe"

if ($ReuseSidecar) {
    if (-not (Test-Path -LiteralPath $sidecar)) {
        throw "-ReuseSidecar requires a previously staged sidecar at $sidecar"
    }
    if ([IO.Path]::GetFileName($sidecar) -ne "morphforge-$Target.exe") {
        throw "Unexpected staged sidecar name: $sidecar"
    }
    $coreToStage = $sidecar
    Write-Warning "Reusing the last successfully built sidecar; core source was not rebuilt."
} else {
    Invoke-Checked $cargo.Source @("build", "--manifest-path", $coreManifest, "--release", "--package", "morphforge-app", "--bin", "morphforge", "--target", $Target) $root
    if (-not (Test-Path -LiteralPath $coreExe)) {
        throw "Core executable was not produced at $coreExe"
    }
    $coreToStage = $coreExe
    $coreName = [IO.Path]::GetFileName($coreToStage)
    if ($coreName -ne "morphforge.exe") {
        throw "Core binary name changed to '$coreName'. current_exe()-based recipe-worker requires morphforge.exe."
    }
}
$recipeSource = Get-Content -LiteralPath (Join-Path $root "crates\morphforge-app\src\recipe.rs") -Raw
if ($recipeSource -notmatch "std::env::current_exe" -or $recipeSource -notmatch 'arg\("recipe-worker"\)') {
    throw "The recipe worker no longer invokes current_exe() with the recipe-worker subcommand."
}

New-Item -ItemType Directory -Path $sidecarDir -Force | Out-Null
if ($coreToStage -ne $sidecar) {
    Copy-Item -LiteralPath $coreToStage -Destination $sidecar -Force
}
if ([IO.Path]::GetFileName($sidecar) -ne "morphforge-$Target.exe") {
    throw "Unexpected sidecar staging name: $sidecar"
}

New-Item -ItemType Directory -Path $iconDir -Force | Out-Null
$iconFile = Join-Path $iconDir "icon.ico"
$needsIcons = -not (Test-Path -LiteralPath $iconFile)
if (-not $needsIcons) {
    $needsIcons = (Get-Item -LiteralPath $iconSource).LastWriteTimeUtc -gt
        (Get-Item -LiteralPath $iconFile).LastWriteTimeUtc
}
if ($needsIcons) {
    Invoke-Checked $tauri.Source @("icon", $iconSource, "--output", $iconDir) $desktop
}

if ($CheckOnly) {
    Invoke-Checked $cargo.Source @(
        "test", "--manifest-path", $coreManifest, "--workspace", "--target", $Target,
        "--", "--skip", "periodic_refresh_waits_until_the_entire_previous_rebuild_was_sent"
    ) $root
    Invoke-Checked $cargo.Source @("test", "--manifest-path", (Join-Path $desktop "Cargo.toml"), "--workspace", "--target", $Target) $desktop
    Write-Host "Windows FullFT/desktop checks passed; no installer produced."
    return
}

if ($SkipBundle) {
    Invoke-Checked $cargo.Source @("build", "--manifest-path", (Join-Path $desktop "Cargo.toml"), "--target", $Target) $desktop
    $debugRuntime = Join-Path $desktop "target\$Target\debug"
    New-Item -ItemType Directory -Path $debugRuntime -Force | Out-Null
    Copy-Item -LiteralPath $sidecar -Destination (Join-Path $debugRuntime "morphforge.exe") -Force
    Write-Host "Desktop shell built without installer: $debugRuntime\morphforge-desktop.exe"
    Write-Host "Sidecar staged as $debugRuntime\morphforge.exe; recipe-worker current_exe invariant preserved."
    return
}

Invoke-Checked $tauri.Source @("build", "--target", $Target, "--bundles", "nsis") $desktop
$releaseRuntime = Join-Path $desktop "target\$Target\release"
$bundledCore = Join-Path $releaseRuntime "morphforge.exe"
if (-not (Test-Path -LiteralPath $bundledCore)) {
    throw "Tauri build did not stage the sidecar as morphforge.exe at $bundledCore"
}
$bundle = Join-Path $releaseRuntime "bundle\nsis"
if (-not (Test-Path -LiteralPath $bundle)) {
    throw "NSIS bundle directory was not produced at $bundle"
}
Write-Host "MorphForge Windows build complete: $bundle"
Write-Host "Verified bundled sidecar: $bundledCore (base name morphforge.exe)."

