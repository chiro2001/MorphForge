# Copy the verified Unity 2022 Pipeline and Nemesis dependencies from Roadster.
# This script only populates missing destinations; it never mirrors/deletes assets.
$ErrorActionPreference = 'Stop'
$sourceRoot = 'D:\Projects\VRChatProjects2\TeslaRoadster'
$targetRoot = 'D:\Projects\VRChatProjects2\MorphForge'
$packageNames = @(
    'com.unity.pipeline',
    'jp.lilxyzw.liltoon',
    'nadena.dev.ndmf',
    'nadena.dev.modular-avatar',
    'minminmean.supine',
    'vrchat.blackstartx.gesture-manager'
)
$reportRoot = Join-Path $targetRoot 'Docs\Setup'

if (-not (Test-Path -LiteralPath (Join-Path $targetRoot 'ProjectSettings\ProjectVersion.txt'))) {
    throw 'MorphForge Unity project is missing.'
}
if (-not (Test-Path -LiteralPath (Join-Path $sourceRoot 'Assets\Nemesis\Nemesis.prefab'))) {
    throw 'The source Nemesis prefab is missing.'
}
$sourceVpmPath = Join-Path $sourceRoot 'Packages\vpm-manifest.json'
$targetVpmPath = Join-Path $targetRoot 'Packages\vpm-manifest.json'
$sourceVpm = Get-Content -LiteralPath $sourceVpmPath -Raw | ConvertFrom-Json -AsHashtable
$targetVpm = Get-Content -LiteralPath $targetVpmPath -Raw | ConvertFrom-Json -AsHashtable
foreach ($sdk in @('com.vrchat.base', 'com.vrchat.avatars')) {
    if ($sourceVpm.dependencies[$sdk].version -ne $targetVpm.dependencies[$sdk].version) {
        throw "Existing SDK differs from the reference project: $sdk"
    }
}
foreach ($name in $packageNames) {
    if (-not (Test-Path -LiteralPath (Join-Path $sourceRoot "Packages\$name\package.json"))) {
        throw "Source package is missing: $name"
    }
    if (Test-Path -LiteralPath (Join-Path $targetRoot "Packages\$name")) {
        throw "Destination already exists; inspect instead of overwriting: $name"
    }
}
foreach ($relative in @('Assets\Nemesis', 'Assets\Nemesis.meta')) {
    if (Test-Path -LiteralPath (Join-Path $targetRoot $relative)) {
        throw "Destination already exists; inspect instead of overwriting: $relative"
    }
}

New-Item -ItemType Directory -Path $reportRoot -Force | Out-Null
$backupPath = Join-Path $reportRoot 'vpm-manifest.before.json'
if (Test-Path -LiteralPath $backupPath) { throw 'Setup backup already exists; inspect previous setup.' }
Copy-Item -LiteralPath $targetVpmPath -Destination $backupPath

$copied = @()
foreach ($name in $packageNames) {
    $src = Join-Path $sourceRoot "Packages\$name"
    $dst = Join-Path $targetRoot "Packages\$name"
    & robocopy $src $dst /E /COPY:DAT /DCOPY:DAT /R:1 /W:1 /NFL /NDL /NJH /NJS /NP | Out-Null
    if ($LASTEXITCODE -ge 8) { throw "Package copy failed: $name ($LASTEXITCODE)" }
    $pkg = Get-Content -LiteralPath (Join-Path $dst 'package.json') -Raw | ConvertFrom-Json
    $copied += [pscustomobject]@{name=$pkg.name; version=$pkg.version}
}
& robocopy (Join-Path $sourceRoot 'Assets\Nemesis') (Join-Path $targetRoot 'Assets\Nemesis') /E /COPY:DAT /DCOPY:DAT /R:1 /W:1 /NFL /NDL /NJH /NJS /NP | Out-Null
if ($LASTEXITCODE -ge 8) { throw "Nemesis copy failed ($LASTEXITCODE)" }
Copy-Item -LiteralPath (Join-Path $sourceRoot 'Assets\Nemesis.meta') -Destination (Join-Path $targetRoot 'Assets\Nemesis.meta')

foreach ($name in $packageNames) {
    foreach ($section in @('dependencies', 'locked')) {
        if ($sourceVpm.ContainsKey($section) -and $sourceVpm[$section].ContainsKey($name)) {
            if (-not $targetVpm.ContainsKey($section)) { $targetVpm[$section] = @{} }
            $targetVpm[$section][$name] = $sourceVpm[$section][$name]
        }
    }
}
$vpmJson = $targetVpm | ConvertTo-Json -Depth 30
[IO.File]::WriteAllText($targetVpmPath, $vpmJson + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))

$prefabRelative = 'Assets\Nemesis\Nemesis.prefab'
$srcPrefabHash = (Get-FileHash -LiteralPath (Join-Path $sourceRoot $prefabRelative) -Algorithm SHA256).Hash
$dstPrefabHash = (Get-FileHash -LiteralPath (Join-Path $targetRoot $prefabRelative) -Algorithm SHA256).Hash
if ($srcPrefabHash -ne $dstPrefabHash) { throw 'Nemesis prefab hash verification failed.' }
$report = [pscustomobject]@{
    timestamp=[DateTimeOffset]::Now.ToString('o')
    source=$sourceRoot
    target=$targetRoot
    packages=$copied
    nemesisFiles=@(Get-ChildItem -LiteralPath (Join-Path $targetRoot 'Assets\Nemesis') -Recurse -File).Count
    nemesisPrefabSHA256=$dstPrefabHash.ToLowerInvariant()
    verifiedPrefabCopy=$true
    unityImportVerified=$false
}
$reportJson = $report | ConvertTo-Json -Depth 8
[IO.File]::WriteAllText((Join-Path $reportRoot 'bootstrap.json'), $reportJson, [Text.UTF8Encoding]::new($false))
Write-Output $reportJson
exit 0
