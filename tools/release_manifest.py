#!/usr/bin/env python3
"""Freeze and verify the exact inputs embedded in a MorphForge desktop build.

Run once before deployment, then `check` on Windows before and after the build.
The manifest contains hashes, never file contents or environment variables.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
PATTERNS = (
    "Cargo.toml", "Cargo.lock", "crates/**/Cargo.toml", "crates/**/*.rs",
    "crates/**/*.js", "crates/**/*.md", "crates/**/*.txt", "web/**/*",
    "fonts/*.otf", "fonts/*.ttf", "fonts/*.txt", "fonts/pixel16/*", "desktop/Cargo.toml",
    "desktop/Cargo.lock", "desktop/app-icon.svg", "desktop/ui/*",
    "desktop/src-tauri/Cargo.toml", "desktop/src-tauri/build.rs",
    "desktop/src-tauri/tauri.conf.json", "desktop/src-tauri/src/*.rs",
    "tools/build_windows.ps1", "references/**/*", "examples/**/*", "materials/**/*",
)


def snapshot():
    paths = sorted({p for pattern in PATTERNS for p in ROOT.glob(pattern)
                    if p.is_file() and not p.is_symlink()})
    entries = {p.relative_to(ROOT).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
               for p in paths}
    return entries


def aggregate(entries):
    canonical = json.dumps(entries, ensure_ascii=True, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(canonical.encode()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("write", "check"))
    parser.add_argument("manifest", type=Path)
    args = parser.parse_args()
    entries = snapshot()
    if args.action == "write":
        revision = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT,
                                  capture_output=True, text=True, check=True).stdout.strip()
        payload = {"format": "MorphForgeBuildInputs/1", "gitCommit": revision,
                   "sha256": aggregate(entries), "files": entries}
        args.manifest.parent.mkdir(parents=True, exist_ok=True)
        args.manifest.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
        print(json.dumps({"ok": True, "files": len(entries), "sha256": payload["sha256"]}))
    else:
        expected = json.loads(args.manifest.read_text(encoding="utf-8"))
        if expected.get("format") != "MorphForgeBuildInputs/1":
            parser.error("unsupported manifest format")
        reference = expected["files"]
        mismatches = sorted(name for name in set(entries) | set(reference)
                            if entries.get(name) != reference.get(name))
        valid = not mismatches and aggregate(reference) == expected["sha256"]
        print(json.dumps({"ok": valid, "files": len(entries), "sha256": aggregate(entries),
                          "mismatches": mismatches}))
        raise SystemExit(0 if valid else 1)


if __name__ == "__main__":
    main()
