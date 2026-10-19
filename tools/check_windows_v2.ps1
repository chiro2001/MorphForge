#requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidateNotNullOrEmpty()]
    [string]$CoreExe,

    [Parameter(Mandatory)]
    [ValidateNotNullOrEmpty()]
    [string]$DataDir,

    [string]$ReportPath = "",

    [string]$FreezeManifest = "",

    [string]$ExpectedToolsVersion = "morphforge-tools/2.3",

    [string]$ExpectedPromptVersion = "morphforge-system/2.3"
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
    throw "check_windows_v2.ps1 must run on Windows PowerShell 7."
}

$scriptRoot = $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($ReportPath)) {
    $ReportPath = [IO.Path]::GetFullPath(
        (Join-Path $scriptRoot "..\docs\quality-v2\windows-v2-validation.json")
    )
}
if ([string]::IsNullOrWhiteSpace($FreezeManifest)) {
    $FreezeManifest = [IO.Path]::GetFullPath(
        (Join-Path $scriptRoot "..\docs\quality-v2\source-freeze.json")
    )
}

$checks = [Collections.Generic.List[object]]::new()
$ownedProcess = $null
$stdoutLines = [Collections.Generic.List[string]]::new()
$startedAt = [DateTimeOffset]::UtcNow
$baseUrl = $null
$actualPort = $null
$fatalError = $null
$finalState = $null
$coreHash = $null
$coreVersion = $null
$resourceResult = [ordered]@{}

function Add-Check {
    param(
        [Parameter(Mandatory)][string]$Name,
        [Parameter(Mandatory)][bool]$Passed,
        [object]$Details = $null
    )

    $entry = [ordered]@{
        name = $Name
        passed = $Passed
    }
    if ($null -ne $Details) {
        $entry.details = $Details
    }
    $checks.Add([pscustomobject]$entry)
    if (-not $Passed) {
        throw "CHECK_FAILED: $Name"
    }
}

function Invoke-JsonHttp {
    param(
        [Parameter(Mandatory)][ValidateSet("GET", "POST")][string]$Method,
        [Parameter(Mandatory)][string]$Path,
        [object]$Body = $null,
        [int]$ExpectedStatus = 200,
        [int]$TimeoutSec = 120
    )

    $arguments = @{
        Uri = $baseUrl + $Path
        Method = $Method
        TimeoutSec = $TimeoutSec
        SkipHttpErrorCheck = $true
        MaximumRedirection = 0
        NoProxy = $true
        Headers = @{ Accept = "application/json" }
    }

    if ($null -ne $Body) {
        $json = $Body | ConvertTo-Json -Depth 100 -Compress
        $arguments.ContentType = "application/json; charset=utf-8"
        $arguments.Body = [Text.Encoding]::UTF8.GetBytes($json)
    }

    $response = Invoke-WebRequest @arguments
    $status = [int]$response.StatusCode
    $raw = [string]$response.Content
    $value = $null
    if (-not [string]::IsNullOrWhiteSpace($raw)) {
        try {
            $value = $raw | ConvertFrom-Json -Depth 100
        }
        catch {
            throw "HTTP_JSON_INVALID: $Method $Path returned status $status with non-JSON content."
        }
    }

    if ($status -ne $ExpectedStatus) {
        throw "HTTP_STATUS: $Method $Path expected $ExpectedStatus, received $status."
    }

    return [pscustomobject]@{
        Status = $status
        Raw = $raw
        Value = $value
    }
}

function Get-State {
    return (Invoke-JsonHttp -Method GET -Path "/api/state").Value
}

function Get-RemoteResourceHash {
    param([Parameter(Mandatory)][string]$Path)

    $handler = [Net.Http.HttpClientHandler]::new()
    $handler.UseProxy = $false
    $client = [Net.Http.HttpClient]::new($handler)
    try {
        $response = $client.GetAsync($baseUrl + $Path).GetAwaiter().GetResult()
        [byte[]]$bytes = $response.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult()
        $status = [int]$response.StatusCode
        $sha256 = [Security.Cryptography.SHA256]::Create()
        try {
            $hash = [Convert]::ToHexString($sha256.ComputeHash($bytes)).ToLowerInvariant()
        }
        finally {
            $sha256.Dispose()
        }
        return [pscustomobject]@{
            Status = $status
            Bytes = $bytes.Length
            Sha256 = $hash
        }
    }
    finally {
        $client.Dispose()
        $handler.Dispose()
    }
}

function Assert-OscDisabled {
    param(
        [Parameter(Mandatory)][string]$Stage,
        [Parameter(Mandatory)][object]$State
    )

    $enabledProperty = $State.connection.PSObject.Properties["enabled"]
    $enabled = $null -ne $enabledProperty -and [bool]$enabledProperty.Value
    Add-Check -Name "OSC disabled: $Stage" -Passed (-not $enabled) -Details @{
        enabled = $enabled
    }

    $captureProperty = $State.PSObject.Properties["capture"]
    if ($null -ne $captureProperty -and $null -ne $captureProperty.Value) {
        $browserPidProperty = $captureProperty.Value.PSObject.Properties["browserPid"]
        $browserPid = if ($null -ne $browserPidProperty) {
            $browserPidProperty.Value
        }
        else {
            $null
        }
        Add-Check -Name "Capture browser absent: $Stage" -Passed ($null -eq $browserPid)
    }
}

function Stop-OwnedProcess {
    if ($null -eq $ownedProcess) {
        return
    }

    try {
        if (-not $ownedProcess.HasExited) {
            $ownedProcess.Kill($true)
        }
        [void]$ownedProcess.WaitForExit(10000)
    }
    catch {
        # The report records cleanup state; never search for or stop another process by name.
    }
}

function Write-ValidationReport {
    param([bool]$Success)

    $finishedAt = [DateTimeOffset]::UtcNow
    $report = [ordered]@{
        schema = "MorphForgeWindowsV2Validation/1"
        success = $Success
        startedAt = $startedAt.ToString("o")
        finishedAt = $finishedAt.ToString("o")
        host = [ordered]@{
            os = [Environment]::OSVersion.VersionString
            powershell = $PSVersionTable.PSVersion.ToString()
            pwsh = (Get-Command pwsh).Source
        }
        core = [ordered]@{
            path = $CoreExe
            sha256 = $coreHash
            version = $coreVersion
        }
        isolation = [ordered]@{
            dataDirectory = $DataDir
            randomLoopbackPort = $actualPort
            baseUrl = $baseUrl
            noCapture = $true
            providerEnvironment = "mock-local-only-no-provider-call"
        }
        checks = @($checks)
        embeddedResources = $resourceResult
        finalState = $finalState
        fatalError = $fatalError
        cleanup = [ordered]@{
            ownedProcessOnly = $true
            exited = if ($null -eq $ownedProcess) { $true } else { $ownedProcess.HasExited }
            exitCode = if ($null -eq $ownedProcess -or -not $ownedProcess.HasExited) {
                $null
            }
            else {
                $ownedProcess.ExitCode
            }
        }
    }

    $reportDirectory = Split-Path -Parent $ReportPath
    if (-not [string]::IsNullOrWhiteSpace($reportDirectory)) {
        New-Item -ItemType Directory -Path $reportDirectory -Force | Out-Null
    }
    $json = $report | ConvertTo-Json -Depth 100
    [IO.File]::WriteAllText($ReportPath, $json, [Text.UTF8Encoding]::new($false))
}

try {
    $corePath = (Resolve-Path -LiteralPath $CoreExe).Path
    Add-Check -Name "core executable exists" -Passed (Test-Path -LiteralPath $corePath -PathType Leaf)
    Add-Check -Name "core executable is morphforge.exe" -Passed (
        [IO.Path]::GetFileName($corePath).Equals(
            "morphforge.exe",
            [StringComparison]::OrdinalIgnoreCase
        )
    )
    $coreHash = (Get-FileHash -LiteralPath $corePath -Algorithm SHA256).Hash.ToLowerInvariant()
    $coreVersion = (& $corePath --version | Select-Object -First 1).Trim()
    Add-Check -Name "core version is available" -Passed (-not [string]::IsNullOrWhiteSpace($coreVersion))

    $dataFullPath = [IO.Path]::GetFullPath($DataDir)
    if (Test-Path -LiteralPath $dataFullPath) {
        $dataItem = Get-Item -LiteralPath $dataFullPath
        Add-Check -Name "data path is a directory" -Passed $dataItem.PSIsContainer
        $reparseMask = [IO.FileAttributes]::ReparsePoint
        Add-Check -Name "data directory is not a reparse point" -Passed (
            ($dataItem.Attributes -band $reparseMask) -eq 0
        )
        $existingItems = @(Get-ChildItem -LiteralPath $dataFullPath -Force)
        Add-Check -Name "data directory is empty" -Passed ($existingItems.Count -eq 0)
    }
    else {
        New-Item -ItemType Directory -Path $dataFullPath -Force | Out-Null
        Add-Check -Name "data directory initialized empty" -Passed (
            @(Get-ChildItem -LiteralPath $dataFullPath -Force).Count -eq 0
        )
    }
    $DataDir = $dataFullPath

    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $corePath
    $startInfo.WorkingDirectory = Split-Path -Parent $corePath
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $startInfo.StandardOutputEncoding = [Text.UTF8Encoding]::new($false)
    $startInfo.StandardErrorEncoding = [Text.UTF8Encoding]::new($false)
    foreach ($argument in @("serve", "--data", $dataFullPath, "--port", "0", "--no-capture")) {
        $startInfo.ArgumentList.Add($argument)
    }
    $startInfo.Environment["DEEPSEEK_API_KEY"] = "mock-local-only-no-provider-call"
    $startInfo.Environment["MORPHFORGE_DEEPSEEK_BASE_URL"] = "http://127.0.0.1:1"

    $ownedProcess = [Diagnostics.Process]::Start($startInfo)
    Add-Check -Name "owned core process started" -Passed ($null -ne $ownedProcess -and $ownedProcess.Id -gt 0)
    $stderrTask = $ownedProcess.StandardError.ReadToEndAsync()

    $readyPrefix = "MorphForge Rust ready:"
    $readyRegex = [regex]::new("^MorphForge Rust ready: http://127\.0\.0\.1:(\d+)\s")
    $deadline = [DateTimeOffset]::UtcNow.AddSeconds(30)
    $lineTask = $ownedProcess.StandardOutput.ReadLineAsync()
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        if ($lineTask.Wait(500)) {
            $line = $lineTask.Result
            if ($null -eq $line) {
                break
            }
            $stdoutLines.Add($line)
            $match = $readyRegex.Match($line)
            if ($match.Success) {
                $actualPort = [int]$match.Groups[1].Value
                $baseUrl = "http://127.0.0.1:$actualPort"
                break
            }
            $lineTask = $ownedProcess.StandardOutput.ReadLineAsync()
        }
        if ($ownedProcess.HasExited) {
            break
        }
    }

    Add-Check -Name "core reported random loopback port" -Passed (
        $null -ne $actualPort -and $actualPort -gt 0
    ) -Details @{
        port = $actualPort
    }
    Add-Check -Name "random port avoids reserved and known test ports" -Passed (
        $actualPort -notin @(8766, 8767, 8781, 8799, 9000, 9012, 9515)
    )
    Add-Check -Name "core emits no-capture OSC-disabled ready line" -Passed (
        $stdoutLines.Count -gt 0 -and
        @($stdoutLines | Where-Object { $_ -like "$readyPrefix*" }).Count -eq 1
    )

    $stateInitial = Get-State
    Assert-OscDisabled -Stage "initial" -State $stateInitial
    Add-Check -Name "fresh isolated project starts with zero boxes" -Passed (
        [int]$stateInitial.stats.boxCount -eq 0
    )
    Add-Check -Name "fresh isolated project starts with no harness run" -Passed (
        $stateInitial.harness.run -eq $null
    )

    $manifestPath = (Resolve-Path -LiteralPath $FreezeManifest).Path
    $freeze = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    Add-Check -Name "freeze manifest format is valid" -Passed (
        $freeze.format -eq "MorphForgeBuildInputs/1" -and
        @($freeze.files.PSObject.Properties).Count -eq 69
    ) -Details @{
        path = $manifestPath
        aggregate = $freeze.sha256
        files = @($freeze.files.PSObject.Properties).Count
    }
    $repoRoot = [IO.Path]::GetFullPath((Join-Path $scriptRoot ".."))
    $embeddedResources = @(
        "studio.html",
        "studio.js",
        "group-ui.js",
        "harness-ui.js",
        "text-ui.js",
        "workbench.css",
        "licenses/OFL-NotoSansSC.txt"
    )
    foreach ($relative in $embeddedResources) {
        $manifestKey = "web/" + $relative
        $manifestProperty = $freeze.files.PSObject.Properties[$manifestKey]
        $diskPath = Join-Path $repoRoot ("web\" + $relative.Replace("/", "\"))
        Add-Check -Name "embedded resource exists on disk: $manifestKey" -Passed (
            $null -ne $manifestProperty -and
            (Test-Path -LiteralPath $diskPath -PathType Leaf)
        )
        $diskHash = (
            Get-FileHash -LiteralPath $diskPath -Algorithm SHA256
        ).Hash.ToLowerInvariant()
        $remote = Get-RemoteResourceHash -Path ("/" + $relative)
        $manifestHash = ([string]$manifestProperty.Value).ToLowerInvariant()
        $resourceResult[$manifestKey] = [ordered]@{
            diskSha256 = $diskHash
            httpSha256 = $remote.Sha256
            manifestSha256 = $manifestHash
            bytes = $remote.Bytes
        }
        Add-Check -Name "embedded HTTP bytes match disk and freeze: $manifestKey" -Passed (
            $remote.Status -eq 200 -and
            $remote.Bytes -gt 0 -and
            $remote.Sha256 -eq $diskHash -and
            $diskHash -eq $manifestHash
        ) -Details $resourceResult[$manifestKey]
    }

    $fontsResponse = Invoke-JsonHttp -Method GET -Path "/api/fonts"
    $fonts = $fontsResponse.Value
    $builtinFont = @($fonts.fonts | Where-Object { $_.id -eq "builtin:noto-sans-sc" })
    Add-Check -Name "builtin Chinese font exposed" -Passed ($builtinFont.Count -eq 1) -Details @{
        defaultFontId = $fonts.defaultFontId
        fontCount = @($fonts.fonts).Count
    }
    Add-Check -Name "builtin font license and hash are present" -Passed (
        $builtinFont.Count -eq 1 -and
        $builtinFont[0].license -eq "OFL-1.1" -and
        ([string]$builtinFont[0].sha256).Length -eq 64
    )

    $sceneInitial = Invoke-JsonHttp -Method GET -Path "/api/scene"
    $materials = @($sceneInitial.Value.materials)
    Add-Check -Name "material catalog is available" -Passed ($materials.Count -gt 0)
    $styleId = [int]$materials[0].id
    $baseRevision = [int]$stateInitial.revision

    $previewRequestId = [guid]::NewGuid().ToString()
    $textRequest = [ordered]@{
        text = "中文体素"
        fontId = "builtin:noto-sans-sc"
        fontSize = 8
        depth = 1
        origin = @(0, 0, 0)
        plane = "xy"
        facing = "positive"
        align = "left"
        letterSpacing = 0
        lineSpacing = 0
        maxWidth = $null
        styleId = $styleId
        tintId = 0
        motionPhase = 0
        group = "windows-v2-text"
        requestId = $previewRequestId
    }
    $previewResponse = Invoke-JsonHttp -Method POST -Path "/api/text/preview" -Body $textRequest
    $preview = $previewResponse.Value
    $previewBoxes = @($preview.operations | Where-Object { $_.op -eq "box" })
    Add-Check -Name "Chinese text preview returns canonical boxes" -Passed (
        $preview.ok -eq $true -and
        $preview.requestId -eq $previewRequestId -and
        $previewBoxes.Count -gt 0 -and
        ([string]$preview.requestFingerprint).Length -eq 64
    ) -Details @{
        requestId = $preview.requestId
        boxCount = $previewBoxes.Count
        layout = $preview.layout
    }
    $stateAfterPreview = Get-State
    Assert-OscDisabled -Stage "after text preview" -State $stateAfterPreview
    Add-Check -Name "text preview does not change revision" -Passed (
        [int]$stateAfterPreview.revision -eq $baseRevision
    )

    $missingRequest = [ordered]@{
        text = "A😀B"
        fontId = "builtin:noto-sans-sc"
        fontSize = 8
        depth = 1
        origin = @(0, 0, 0)
        plane = "xy"
        facing = "positive"
        align = "left"
        letterSpacing = 0
        lineSpacing = 0
        maxWidth = $null
        styleId = $styleId
        tintId = 0
        motionPhase = 0
        group = "windows-v2-missing"
        requestId = [guid]::NewGuid().ToString()
    }
    $missingResponse = Invoke-JsonHttp -Method POST -Path "/api/text/preview" -Body $missingRequest -ExpectedStatus 400
    Add-Check -Name "missing glyph is a typed top-level error" -Passed (
        $missingResponse.Value.code -eq "FONT_MISSING_GLYPHS" -and
        @($missingResponse.Value.missingGlyphs).Count -gt 0 -and
        $missingResponse.Value.error -is [string]
    ) -Details @{
        status = $missingResponse.Status
        code = $missingResponse.Value.code
        missingGlyphs = @($missingResponse.Value.missingGlyphs)
    }

    $applyRequestId = [guid]::NewGuid().ToString()
    $applyRequest = [ordered]@{
        text = "中文体素"
        fontId = "builtin:noto-sans-sc"
        fontSize = 8
        depth = 1
        origin = @(0, 0, 0)
        plane = "xy"
        facing = "positive"
        align = "left"
        letterSpacing = 0
        lineSpacing = 0
        maxWidth = $null
        styleId = $styleId
        tintId = 0
        motionPhase = 0
        group = "windows-v2-text"
        baseRevision = $baseRevision
        requestId = $applyRequestId
        previewRequestId = $previewRequestId
        previewFingerprint = $preview.requestFingerprint
    }
    $applyResponse = Invoke-JsonHttp -Method POST -Path "/api/text/apply" -Body $applyRequest -TimeoutSec 180
    $apply = $applyResponse.Value
    Add-Check -Name "text apply creates one revision" -Passed (
        $apply.ok -eq $true -and
        $apply.requestId -eq $applyRequestId -and
        [int]$apply.revision -eq ($baseRevision + 1) -and
        [int]$apply.scene.revision -eq [int]$apply.revision -and
        [int]$apply.stats.boxCount -gt 0
    ) -Details @{
        revision = $apply.revision
        boxCount = $apply.stats.boxCount
    }

    $stateAfterApply = Get-State
    Assert-OscDisabled -Stage "after text apply" -State $stateAfterApply
    $appliedRevision = [int]$stateAfterApply.revision

    $applyReplay = (Invoke-JsonHttp -Method POST -Path "/api/text/apply" -Body $applyRequest -TimeoutSec 180).Value
    $stateAfterReplay = Get-State
    Add-Check -Name "text apply requestId replays the durable receipt" -Passed (
        $applyReplay.requestId -eq $applyRequestId -and
        [int]$applyReplay.revision -eq $appliedRevision -and
        [int]$stateAfterReplay.revision -eq $appliedRevision -and
        [int]$applyReplay.scene.revision -eq $appliedRevision
    ) -Details @{
        requestId = $applyReplay.requestId
        revision = $applyReplay.revision
    }
    Assert-OscDisabled -Stage "after idempotent apply" -State $stateAfterReplay

    $undoResponse = Invoke-JsonHttp -Method POST -Path "/api/undo" -Body @{
        revision = $baseRevision
    }
    $stateAfterUndo = Get-State
    Add-Check -Name "text apply can be undone" -Passed (
        [int]$stateAfterUndo.revision -gt $appliedRevision -and
        [int]$stateAfterUndo.stats.boxCount -eq [int]$stateInitial.stats.boxCount
    ) -Details @{
        undoResponseRevision = $undoResponse.Value.revision
        finalRevision = $stateAfterUndo.revision
        boxCount = $stateAfterUndo.stats.boxCount
    }
    Assert-OscDisabled -Stage "after text undo" -State $stateAfterUndo

    $zhResponse = Invoke-JsonHttp -Method GET -Path "/api/harness/tools?language=zh-CN"
    $enResponse = Invoke-JsonHttp -Method GET -Path "/api/harness/tools?language=en"
    $zh = $zhResponse.Value
    $en = $enResponse.Value
    $requiredToolNames = @(
        "get_context", "run_recipe", "inspect_region", "capture_views",
        "commit", "finish_artwork", "publish", "sync_status", "undo_to",
        "artworks", "compact_context", "voxel_text"
    )
    $zhNames = @($zh.tools | ForEach-Object { $_.name } | Sort-Object)
    $enNames = @($en.tools | ForEach-Object { $_.name } | Sort-Object)
    $expectedNames = @($requiredToolNames | Sort-Object)
    Add-Check -Name "Chinese and English tool schemas have the same complete names" -Passed (
        ($zhNames -join ",") -eq ($expectedNames -join ",") -and
        ($enNames -join ",") -eq ($expectedNames -join ",") -and
        $zh.version -eq $en.version -and
        $zh.promptVersion -eq $en.promptVersion -and
        $zh.version -eq $ExpectedToolsVersion -and
        $zh.promptVersion -eq $ExpectedPromptVersion
    ) -Details @{
        toolCount = $zhNames.Count
        version = $zh.version
        promptVersion = $zh.promptVersion
    }
    $zhContext = $zh.tools | Where-Object { $_.name -eq "get_context" }
    $enContext = $en.tools | Where-Object { $_.name -eq "get_context" }
    $han = [regex]::new("[\u4e00-\u9fff]")
    Add-Check -Name "Chinese and English tool descriptions are language-specific" -Passed (
        $zh.language -eq "zh-CN" -and
        $en.language -eq "en" -and
        $han.IsMatch($zhContext.description) -and
        -not $han.IsMatch($enContext.description)
    )
    $zhToolsByName = @{}
    $enToolsByName = @{}
    foreach ($tool in $zh.tools) {
        $zhToolsByName[[string]$tool.name] = $tool
    }
    foreach ($tool in $en.tools) {
        $enToolsByName[[string]$tool.name] = $tool
    }
    $parameterMismatches = @(
        foreach ($name in $expectedNames) {
            $zhParameters = $zhToolsByName[$name].parameters |
                ConvertTo-Json -Depth 50 -Compress
            $enParameters = $enToolsByName[$name].parameters |
                ConvertTo-Json -Depth 50 -Compress
            if ($zhParameters -ne $enParameters) {
                $name
            }
        }
    )
    Add-Check -Name "Chinese and English tool parameter schemas are identical" -Passed (
        $parameterMismatches.Count -eq 0
    ) -Details @{
        mismatches = @($parameterMismatches)
    }
    $voxelTextSchema = $zhToolsByName["voxel_text"]
    $voxelTextProperties = @($voxelTextSchema.parameters.properties.PSObject.Properties.Name)
    Add-Check -Name "voxel_text schema exposes font preview apply fields" -Passed (
        @($voxelTextSchema.parameters.required) -contains "mode" -and
        @($voxelTextSchema.parameters.properties.mode.enum) -join "," -eq "fonts,preview,apply" -and
        $voxelTextProperties -contains "fontId" -and
        $voxelTextProperties -contains "plane" -and
        $voxelTextProperties -contains "previewFingerprint" -and
        $voxelTextProperties -contains "baseRevision" -and
        $voxelTextProperties -contains "requestId"
    )

    $stateBeforeTextRecipe = Get-State
    $sceneBeforeTextRecipe = Invoke-JsonHttp -Method GET -Path "/api/scene"
    $textRecipeCode = @"
mf.text('中文', [0,0,0], {
  fontId: 'builtin:noto-sans-sc',
  fontSize: 8,
  depth: 1,
  plane: 'xy',
  facing: 'positive',
  style: $styleId,
  group: 'windows-v2-text-recipe'
});
"@
    $textRecipeBody = @{
        name = "run_recipe"
        arguments = @{
            code = $textRecipeCode
            mode = "apply"
            baseRevision = [int]$stateBeforeTextRecipe.revision
            seed = 20261019
            maxOps = 2000
            dryRun = $true
        }
    }
    $textRecipe = (Invoke-JsonHttp -Method POST -Path "/api/harness/tool" -Body $textRecipeBody -TimeoutSec 180).Value
    $stateAfterTextRecipe = Get-State
    $sceneAfterTextRecipe = Invoke-JsonHttp -Method GET -Path "/api/scene"
    Add-Check -Name "mf.text Recipe dry-run expands to boxes without applying" -Passed (
        $textRecipe.ok -eq $true -and
        $textRecipe.dryRun -eq $true -and
        [int]$textRecipe.stats.boxCount -gt 0 -and
        [int]$stateAfterTextRecipe.revision -eq [int]$stateBeforeTextRecipe.revision -and
        $sceneAfterTextRecipe.Raw -eq $sceneBeforeTextRecipe.Raw
    ) -Details @{
        revision = $textRecipe.revision
        boxCount = $textRecipe.stats.boxCount
        voxelCount = $textRecipe.stats.voxelCount
    }
    Assert-OscDisabled -Stage "after mf.text Recipe dry-run" -State $stateAfterTextRecipe

    $stateBeforeBulkRecipe = $stateAfterTextRecipe
    $sceneBeforeBulkRecipe = $sceneAfterTextRecipe
    $bulkRecipeCode = @"
for (let i = 0; i < 16384; i++) {
  const x = i % 32;
  const y = Math.floor(i / 1024);
  const z = Math.floor(i / 32) % 32;
  mf.box(
    [x * 3 - 48, y * 3, z * 3 - 48],
    [1, 1, 1],
    {style: $styleId, group: 'windows-v2-bulk'}
  );
}
"@
    $bulkRecipeBody = @{
        name = "run_recipe"
        arguments = @{
            code = $bulkRecipeCode
            mode = "apply"
            baseRevision = [int]$stateBeforeBulkRecipe.revision
            seed = 16384
            maxOps = 20000
            dryRun = $true
        }
    }
    $bulkRecipe = (Invoke-JsonHttp -Method POST -Path "/api/harness/tool" -Body $bulkRecipeBody -TimeoutSec 300).Value
    $stateAfterBulkRecipe = Get-State
    $sceneAfterBulkRecipe = Invoke-JsonHttp -Method GET -Path "/api/scene"
    Add-Check -Name "16384 operation Recipe dry-run returns exact count without applying" -Passed (
        $bulkRecipe.ok -eq $true -and
        $bulkRecipe.dryRun -eq $true -and
        [int]$bulkRecipe.stats.boxCount -eq 16384 -and
        [int]$stateAfterBulkRecipe.revision -eq [int]$stateBeforeBulkRecipe.revision -and
        $sceneAfterBulkRecipe.Raw -eq $sceneBeforeBulkRecipe.Raw
    ) -Details @{
        revision = $bulkRecipe.revision
        boxCount = $bulkRecipe.stats.boxCount
        voxelCount = $bulkRecipe.stats.voxelCount
    }
    Assert-OscDisabled -Stage "after 16384 Recipe dry-run" -State $stateAfterBulkRecipe

    $eventsResponse = Invoke-JsonHttp -Method GET -Path "/api/events?after=0"
    $eventKinds = @($eventsResponse.Value.events | ForEach-Object { $_.kind })
    $forbiddenEventKinds = @(
        "provider.request", "harness.started",
        "harness.message.accepted", "harness.message.applied"
    )
    $forbiddenEvents = @(
        $eventKinds | Where-Object { $_ -in $forbiddenEventKinds }
    )
    Add-Check -Name "no provider or Harness model-loop events were produced" -Passed (
        $forbiddenEvents.Count -eq 0
    ) -Details @{
        eventCount = $eventKinds.Count
        forbiddenKinds = @($forbiddenEvents)
    }

    $finalState = Get-State
    Assert-OscDisabled -Stage "final" -State $finalState
    Add-Check -Name "final Harness state has no run or queued model work" -Passed (
        $finalState.harness.run -eq $null -and
        @($finalState.harness.interaction.queue).Count -eq 0
    )
}
catch {
    $fatalError = $_.Exception.Message
}
finally {
    Stop-OwnedProcess
}

if ($null -ne $ownedProcess -and -not $ownedProcess.HasExited) {
    $checks.Add([pscustomobject][ordered]@{
        name = "owned core process stopped"
        passed = $false
    })
    if ([string]::IsNullOrWhiteSpace($fatalError)) {
        $fatalError = "OWNED_PROCESS_NOT_STOPPED"
    }
}
else {
    $checks.Add([pscustomobject][ordered]@{
        name = "owned core process stopped"
        passed = $true
    })
}

$passed = [string]::IsNullOrWhiteSpace($fatalError) -and
    @($checks | Where-Object { -not $_.passed }).Count -eq 0
Write-ValidationReport -Success $passed

if (-not $passed) {
    Write-Error "Windows v2 validation failed: $fatalError"
    exit 1
}

Write-Host (
    "Windows v2 validation passed: {0} checks; report={1}" -f
    $checks.Count,
    $ReportPath
)
exit 0
