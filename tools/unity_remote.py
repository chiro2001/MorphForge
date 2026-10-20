#!/usr/bin/env python3
"""Run Unity CLI/Pipeline through SSH and PowerShell on the project workstation."""

import argparse
import base64
import json
import os
import subprocess
import sys

# Use the configured alias so BatchMode, identity and multiplexing settings
# are applied consistently to Pipeline and ordinary remote commands.
DEFAULT_HOST = os.environ.get("MORPHFORGE_SSH_HOST", "gpu5080")
DEFAULT_PROJECT = r"D:\Projects\VRChatProjects2\MorphForge"


def pwsh_literal(value: str) -> str:
    return "'" + value.replace("'", "''") + "'"


def run_pwsh(script: str, host: str = DEFAULT_HOST, capture_output: bool = False):
    prefix = (
        "$ErrorActionPreference = 'Stop'; $ProgressPreference = 'SilentlyContinue'; "
        "$PSStyle.OutputRendering = 'PlainText'; "
        "[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false); "
        "$OutputEncoding = [Console]::OutputEncoding; "
    )
    encoded = base64.b64encode((prefix + script).encode("utf-16le")).decode("ascii")
    stdin = None
    if len(encoded) > 6000:
        # Windows OpenSSH may invoke its default shell through cmd.exe, whose
        # command line limit is shorter than the C# diagnostic snapshots.
        bootstrap = "[Console]::InputEncoding=[Text.UTF8Encoding]::new($false); & ([scriptblock]::Create([Console]::In.ReadToEnd()))"
        encoded = base64.b64encode(bootstrap.encode("utf-16le")).decode("ascii")
        stdin = (prefix + script).encode("utf-8")
    control = ["-S", os.environ["MORPHFORGE_SSH_CONTROL"]] if os.environ.get("MORPHFORGE_SSH_CONTROL") else []
    return subprocess.run([
        "ssh", *control, "-T", "-o", "BatchMode=yes", "-o", "ConnectTimeout=15",
        "-o", "LogLevel=ERROR", host,
        "pwsh -NoLogo -NoProfile -NonInteractive -EncodedCommand " + encoded,
    ], capture_output=capture_output, input=stdin)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default=DEFAULT_HOST)
    parser.add_argument(
        "--project-path", default=DEFAULT_PROJECT
    )
    parser.add_argument(
        "--cli", action="store_true", help="Forward a top-level Unity CLI command."
    )
    parser.add_argument(
        "--check-result", action="store_true",
        help="Return a failure exit code when the Pipeline payload reports failure (requires JSON output).",
    )
    parser.add_argument("arguments", nargs=argparse.REMAINDER)
    options = parser.parse_args()
    if not options.arguments:
        parser.error("Supply a Pipeline command, or use --cli with a Unity CLI command.")

    arguments = options.arguments
    if arguments[0] == "--":
        arguments = arguments[1:]
    if not options.cli:
        arguments = [
            "command", "--project-path", options.project_path,
            "--caller", "codex", "--skill", "unity-pipeline", "--json",
            *arguments,
        ]
    script = (
        f"Set-Location -LiteralPath {pwsh_literal(options.project_path)}; "
        "& 'unity' " + " ".join(pwsh_literal(arg) for arg in arguments)
        + "; exit $LASTEXITCODE"
    )
    result = run_pwsh(script, options.host, options.check_result)
    if options.check_result:
        sys.stdout.buffer.write(result.stdout)
        sys.stderr.buffer.write(result.stderr)
        if result.returncode:
            return result.returncode
        try:
            envelope = json.loads(result.stdout)
            payload = (envelope.get("data") or {}).get("result")
            while isinstance(payload, dict):
                if payload.get("success") is False or payload.get("failed") is True:
                    return 6
                nested = payload.get("result")
                if isinstance(nested, str):
                    try:
                        nested = json.loads(nested)
                    except ValueError:
                        break
                if not isinstance(nested, dict):
                    break
                payload = nested
        except (ValueError, AttributeError):
            print("Cannot validate non-JSON Unity CLI output.", file=sys.stderr)
            return 6
    return result.returncode


if __name__ == "__main__":
    sys.exit(main())
