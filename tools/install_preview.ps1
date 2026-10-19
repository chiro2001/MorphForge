#requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$PackageDir,
    [Parameter(Mandatory)][string]$ExpectedVersion,
    [string]$PreviewRoot=(Join-Path $env:LOCALAPPDATA 'MorphForge Preview'),
    [string]$SeedDataFrom='',
    [string]$SeedBaseUrl='',
    [string]$ReportPath=''
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$checks=[Collections.Generic.List[object]]::new()
$report=[ordered]@{success=$false;scope='Side-by-side portable deployment; never runs NSIS, stops an app, or replaces a running runtime';checks=$checks;error=$null}
function Check([string]$Name,[bool]$Passed){$checks.Add(@{name=$Name;passed=$Passed});if(-not $Passed){throw $Name}}
function AtomicBytes([string]$Path,[byte[]]$Bytes){
    New-Item -ItemType Directory -Path (Split-Path -Parent $Path) -Force|Out-Null
    $temp=$Path+'.'+[guid]::NewGuid().ToString('N')+'.tmp'
    [IO.File]::WriteAllBytes($temp,$Bytes)
    [IO.File]::Move($temp,$Path,$true)
}
function AtomicJson([string]$Path,$Value){
    AtomicBytes $Path ([Text.Encoding]::UTF8.GetBytes(($Value|ConvertTo-Json -Depth 100)))
}
try {
    $package=[IO.Path]::GetFullPath($PackageDir)
    $root=[IO.Path]::GetFullPath($PreviewRoot)
    $stable=[IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'MorphForge Desktop'))
    $stableData=[IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'MorphForge'))
    Check 'preview root is separate from stable runtime and data' ($root -ne $stable -and $root -ne $stableData -and -not $root.StartsWith($stable+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase) -and -not $root.StartsWith($stableData+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase))
    foreach($name in @('morphforge-desktop.exe','morphforge.exe','licenses')){Check ('package contains '+$name) (Test-Path -LiteralPath (Join-Path $package $name))}
    $version=(& (Join-Path $package 'morphforge.exe') --version|Out-String).Trim()
    Check 'package reports expected core version' ($LASTEXITCODE -eq 0 -and $version -eq ('morphforge '+$ExpectedVersion))
    $payload=[ordered]@{}
    foreach($name in @('morphforge-desktop.exe','morphforge.exe')){$payload[$name]=(Get-FileHash -LiteralPath (Join-Path $package $name) -Algorithm SHA256).Hash.ToLowerInvariant()}
    foreach($file in (Get-ChildItem -LiteralPath (Join-Path $package 'licenses') -File -Recurse|Sort-Object FullName)){$name=[IO.Path]::GetRelativePath($package,$file.FullName);$payload[$name]=(Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}
    $bytes=[Text.Encoding]::UTF8.GetBytes(($payload|ConvertTo-Json -Compress))
    $identity=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes)).ToLowerInvariant()
    $relative='versions\'+$ExpectedVersion+'-'+$identity.Substring(0,12)
    $runtime=Join-Path $root $relative
    $data=Join-Path $root 'Data'
    New-Item -ItemType Directory -Path (Join-Path $root 'versions') -Force|Out-Null
    if(-not (Test-Path -LiteralPath $runtime)){
        $stage=Join-Path $root ('versions\staging-'+[guid]::NewGuid().ToString('N'))
        New-Item -ItemType Directory -Path $stage|Out-Null
        foreach($name in @('morphforge-desktop.exe','morphforge.exe','licenses')){Copy-Item -LiteralPath (Join-Path $package $name) -Destination (Join-Path $stage $name) -Recurse}
        AtomicJson (Join-Path $stage 'build-info.json') @{version=$ExpectedVersion;payloadSha256=$identity;files=$payload}
        [IO.Directory]::Move($stage,$runtime)
    }
    foreach($name in $payload.Keys){Check ('immutable runtime payload: '+$name) ((Get-FileHash -LiteralPath (Join-Path $runtime $name) -Algorithm SHA256).Hash.ToLowerInvariant() -eq $payload[$name])}
    if(-not (Test-Path -LiteralPath $data) -and $SeedDataFrom){
        Check 'initial snapshot is read through the existing backend' ($SeedBaseUrl -match '^http://127\.0\.0\.1:\d+$')
        $seed=[IO.Path]::GetFullPath($SeedDataFrom)
        Check 'seed data is separate from preview data' ($seed -ne $data)
        $snapshot=Invoke-RestMethod ($SeedBaseUrl+'/api/export') -NoProxy
        $stageData=Join-Path $root ('Data-seed-'+[guid]::NewGuid().ToString('N'))
        New-Item -ItemType Directory -Path $stageData|Out-Null
        # No queued AI, conversation, cache ledger, connection state or browser profile is cloned.
        foreach($name in @('artifacts','artworks','artwork-archives','packs','snapshots')){if(Test-Path -LiteralPath (Join-Path $seed $name)){Copy-Item -LiteralPath (Join-Path $seed $name) -Destination (Join-Path $stageData $name) -Recurse}}
        foreach($name in @('provider-connection.json','provider.env','harness.json')){if(Test-Path -LiteralPath (Join-Path $seed $name)){Copy-Item -LiteralPath (Join-Path $seed $name) -Destination (Join-Path $stageData $name)}}
        if(Test-Path -LiteralPath (Join-Path $seed 'runs')){
            $sources=@(Get-ChildItem -LiteralPath (Join-Path $seed 'runs') -Filter '*-sources.json' -File)
            if($sources.Count){New-Item -ItemType Directory -Path (Join-Path $stageData 'runs')|Out-Null;foreach($source in $sources){Copy-Item -LiteralPath $source.FullName -Destination (Join-Path $stageData 'runs')}}
        }
        AtomicJson (Join-Path $stageData 'project.json') $snapshot.project
        [IO.Directory]::Move($stageData,$data)
        $report.seed=@{projectId=$snapshot.project.id;revision=$snapshot.project.scene.revision;boxCount=$snapshot.project.scene.boxes.Count;conversationCopied=$false}
    }
    New-Item -ItemType Directory -Path $data -Force|Out-Null
    AtomicBytes (Join-Path $root 'Launch-Preview.ps1') ([IO.File]::ReadAllBytes((Join-Path $PSScriptRoot 'launch_preview.ps1')))
    $rootLiteral=$root.Replace("'","''")
    $launcherLiteral=(Join-Path $root 'Launch-Preview.ps1').Replace("'","''")
    $command="& '$launcherLiteral' -PreviewRoot '$rootLiteral'"
    $encoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($command))
    $vbs="CreateObject(""WScript.Shell"").Run ""pwsh.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -EncodedCommand $encoded"", 0, False`r`n"
    AtomicBytes (Join-Path $root 'Launch-Preview.vbs') ([Text.Encoding]::ASCII.GetBytes($vbs))
    AtomicJson (Join-Path $root 'current.json') @{version=$ExpectedVersion;runtime=$relative;payloadSha256=$identity}
    $shell=New-Object -ComObject WScript.Shell
    $shortcutPath=Join-Path ([Environment]::GetFolderPath('Desktop')) 'MorphForge - 开发测试副本.lnk'
    $shortcut=$shell.CreateShortcut($shortcutPath)
    $shortcut.TargetPath=Join-Path $env:WINDIR 'System32\wscript.exe'
    $shortcut.Arguments='"'+(Join-Path $root 'Launch-Preview.vbs')+'"'
    $shortcut.WorkingDirectory=$root
    $shortcut.IconLocation=(Join-Path $runtime 'morphforge-desktop.exe')+',0'
    $shortcut.Description='独立开发测试副本；与当前验收版同时运行，作品和上下文分别保存。'
    $shortcut.Save()
    $report.version=$ExpectedVersion;$report.previewRoot=$root;$report.runtime=$runtime;$report.dataDir=$data;$report.shortcut=$shortcutPath;$report.payloadSha256=$identity;$report.success=$true
} catch {$report.error=$_.Exception.Message}
finally {
    $report.finishedAt=[DateTimeOffset]::UtcNow.ToString('o')
    if(-not $ReportPath){$ReportPath=Join-Path $env:USERPROFILE 'tmp\morphforge\preview-deployment.json'}
    AtomicJson $ReportPath $report
}
if(-not $report.success){throw $report.error}
Write-Host "Preview deployed: $ExpectedVersion; active windows left running."
