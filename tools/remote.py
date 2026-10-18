#!/usr/bin/env python3
"""Execute UTF-8 PowerShell on the MorphForge workstation through SSH."""

import argparse
from pathlib import Path
import sys

from unity_remote import DEFAULT_HOST, run_pwsh


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default=DEFAULT_HOST)
    parser.add_argument("--file", type=Path)
    options = parser.parse_args()
    script = options.file.read_text(encoding="utf-8") if options.file else sys.stdin.read()
    if not script.strip():
        parser.error("Provide --file or a PowerShell script on stdin.")
    result = run_pwsh(script, options.host, capture_output=True)
    sys.stdout.buffer.write(result.stdout)
    sys.stderr.buffer.write(result.stderr)
    return result.returncode


if __name__ == "__main__":
    sys.exit(main())
