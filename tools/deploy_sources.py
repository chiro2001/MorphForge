#!/usr/bin/env python3
"""Deploy sources and licensed font resources without copying commercial assets."""

import base64
import argparse
import json
from pathlib import Path
import sys

from unity_remote import DEFAULT_PROJECT, pwsh_literal, run_pwsh

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--skip-unity-assets", action="store_true", help="Do not touch Assets/ while a Play Mode validation is running.")
    options = parser.parse_args()
    files = []
    files.extend(p for p in ROOT.glob("examples/**/*") if p.is_file() and not p.is_symlink())
    for pattern in ("references/**/*.js", "references/**/*.json", "references/**/*.md", "references/**/*.txt", "references/**/*.png", "references/**/*.webp", "tools/*.py", "tools/*.ps1", "tools/*.mjs", "crates/**/*.rs", "crates/**/*.js", "crates/**/*.txt", "crates/**/*.md", "crates/**/Cargo.toml", "materials/*.txt", "materials/**/*.json", "fonts/*.otf", "fonts/*.ttf", "fonts/*.txt", "fonts/pixel16/*", "AgentScripts/*.cs", "Assets/MorphForge/Editor/*.cs", "Assets/MorphForge/Runtime/*.cs", "Assets/MorphForge/Shaders/*.shader", "Assets/MorphForge/Shaders/*.cginc", "docs/**/*.md", ".agents/skills/**/*.md", "web/**/*.html", "web/**/*.css", "web/**/*.js", "web/**/*.mjs", "web/**/*.txt", "web/**/*.json"):
        files.extend(p for p in ROOT.glob(pattern) if p.is_file() and not p.is_symlink())
    for pattern in ("desktop/*.toml", "desktop/*.lock", "desktop/*.md", "desktop/*.svg", "desktop/ui/*", "desktop/src-tauri/*.toml", "desktop/src-tauri/*.json", "desktop/src-tauri/*.rs", "desktop/src-tauri/src/*.rs"):
        files.extend(p for p in ROOT.glob(pattern) if p.is_file() and not p.is_symlink())
    files.extend(ROOT / name for name in ("README.md", "AGENTS.md", "UNITY-GUIDE.md", "Cargo.toml", "Cargo.lock") if (ROOT / name).is_file())
    if options.skip_unity_assets:
        files = [p for p in files if p.relative_to(ROOT).parts[0] != "Assets"]
    payload = {p.relative_to(ROOT).as_posix(): base64.b64encode(p.read_bytes()).decode("ascii") for p in sorted(set(files))}
    encoded = base64.b64encode(json.dumps(payload).encode()).decode("ascii")
    script = f"""
$root = {pwsh_literal(DEFAULT_PROJECT)}
if (-not (Test-Path -LiteralPath (Join-Path $root 'ProjectSettings\\ProjectVersion.txt'))) {{ throw 'Wrong target project.' }}
$files = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{encoded}')) | ConvertFrom-Json -AsHashtable
$deployed = 0
$unchanged = 0
foreach ($relative in $files.Keys) {{
    if ($relative -match '(^|/)\\.\\.(/|$)' -or [IO.Path]::IsPathRooted($relative)) {{ throw 'Invalid relative source path.' }}
    $extension = [IO.Path]::GetExtension($relative).ToLowerInvariant()
    $isReference = $relative -match '^(references|examples)/[A-Za-z0-9_./-]+\\.(png|webp)$'
    $isFont = $relative -match '^fonts/[A-Za-z0-9_./-]+\\.(otf|ttf|bin)$'
    if (-not $isReference -and -not $isFont -and $extension -notin @('.py','.ps1','.cs','.shader','.cginc','.md','.html','.css','.js','.txt','.json','.rs','.toml','.lock','.mjs','.svg')) {{ throw 'Only source files and bundled fonts are allowed.' }}
    $path = Join-Path $root $relative
    $bytes = [Convert]::FromBase64String($files[$relative])
    if (Test-Path -LiteralPath $path -PathType Leaf) {{
        $hasher = [Security.Cryptography.SHA256]::Create()
        try {{ $newHash = [BitConverter]::ToString($hasher.ComputeHash($bytes)).Replace('-','') }} finally {{ $hasher.Dispose() }}
        if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -eq $newHash) {{ $unchanged++; continue }}
    }}
    New-Item -ItemType Directory -Path (Split-Path -Parent $path) -Force | Out-Null
    [IO.File]::WriteAllBytes($path, $bytes)
    $deployed++
}}
[pscustomobject]@{{project=$root; deployed=$deployed; unchanged=$unchanged}} | ConvertTo-Json -Compress
"""
    result = run_pwsh(script, capture_output=True)
    sys.stdout.buffer.write(result.stdout)
    sys.stderr.buffer.write(result.stderr)
    return result.returncode


if __name__ == "__main__":
    sys.exit(main())
