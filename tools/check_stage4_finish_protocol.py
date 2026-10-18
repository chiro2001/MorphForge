#!/usr/bin/env python3
"""Real MF3 UDP 9012 short regression after the Stage4 Fire .03 finish.

This observes the product native ParticleSystem only. It never writes particles
through Unity and never overwrites Docs/Stage4 capacity progress/report files.
"""
from __future__ import annotations

import argparse
import json
import time
from pathlib import Path

from check_stage2 import pipeline, remote_file
from unity_remote import DEFAULT_PROJECT, pwsh_literal, run_pwsh

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "docs" / "materials"
OUT.mkdir(parents=True, exist_ok=True)
COMMANDS = "Docs/Materials/protocol-regression-256-commands.json"
REPORT = OUT / "protocol-regression-256.json"
PORT = 9012
COUNT = 256


def entry(name: str) -> dict:
    result = pipeline(
        "run_script",
        "--file",
        "AgentScripts/Stage4Entry.cs",
        "--entry",
        "Stage4Entry." + name,
        "--timeout_ms",
        "60000",
    )
    if not result.get("success"):
        raise RuntimeError(result)
    return json.loads(result["result"])


def send(commands: list[dict], seq: int, interval: float) -> dict:
    remote_file(COMMANDS, commands)
    args = [
        "tools/send_mf3.py",
        "--input",
        COMMANDS,
        "--port",
        str(PORT),
        "--seq",
        str(seq),
        "--interval",
        str(interval),
    ]
    result = run_pwsh(
        "Set-Location -LiteralPath "
        + pwsh_literal(DEFAULT_PROJECT)
        + "; & python "
        + " ".join(pwsh_literal(arg) for arg in args)
        + "; exit $LASTEXITCODE",
        capture_output=True,
    )
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors="replace"))
    return json.loads(result.stdout.decode("utf-8-sig"))


def voxel(index: int) -> dict:
    return {
        "op": "voxel",
        "min": [80 + index % 32 * 3, 0, 80 + index // 32 * 3],
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--interval", type=float, default=0.25)
    parser.add_argument("--output", type=Path, default=REPORT)
    options = parser.parse_args()
    if options.interval <= 0:
        parser.error("interval must be positive")
    report = {
        "success": False,
        "port": PORT,
        "interval": options.interval,
        "expected": COUNT,
        "scope": "Actual Windows UDP9012 -> automatic GM -> MF3 native FX/particles; no injected product particles",
        "preservedOriginalProgress": "Docs/Stage4/capacity-progress.json",
        "checks": [],
        "captures": [],
    }

    def check(name: str, passed: bool, detail=None) -> None:
        report["checks"].append({"name": name, "pass": bool(passed), "detail": detail})
        print(("PASS " if passed else "FAIL ") + name, flush=True)
        if not passed:
            raise AssertionError(name)

    def capture(label: str) -> dict:
        value = entry("Capture")
        value["label"] = label
        report["captures"].append(value)
        return value

    try:
        initial = capture("initial")
        check("Play receiver is running", entry("Receiver").get("Running") is True)
        send(
            [{"op": "clear"}, {"op": "brush", "styleId": 1, "tintId": 0, "motionPhase": 0}],
            1,
            options.interval,
        )
        baseline = None
        for _ in range(20):
            current = capture("baseline-clear-brush")
            if current["count"] == 0 and not current["busy"] and not current["inbox"]:
                baseline = current
                break
            time.sleep(0.25)
        if baseline is None:
            raise AssertionError("baseline clear/brush did not settle")
        base_rx = baseline["received"]
        base_done = baseline["completed"]
        started = time.monotonic()
        sender = send([voxel(i) for i in range(COUNT)], 3, options.interval)
        elapsed = time.monotonic() - started
        sample = None
        for _ in range(40):
            current = capture("after-256")
            if (
                current["received"] - base_rx >= COUNT
                and current["completed"] - base_done >= COUNT
                and not current["busy"]
                and not current["inbox"]
            ):
                sample = current
                break
            time.sleep(0.5)
        if sample is None:
            raise AssertionError("256 commands did not settle")

        expected = set(range(COUNT))
        unique = set()
        wrong_center = 0
        wrong_size = 0
        metadata_mismatch = 0
        for particle in sample["particles"]:
            x = (particle["position"][0] + 11.875) / 0.75
            y = (particle["position"][1] + 31.875) / 0.25
            z = (particle["position"][2] + 11.875) / 0.75
            ix = round(x)
            iy = round(y)
            iz = round(z)
            if (
                abs(x - ix) > 0.01
                or abs(y - iy) > 0.01
                or abs(z - iz) > 0.01
                or not (0 <= ix < 32 and iy == 0 and 0 <= iz < 8)
            ):
                wrong_center += 1
            else:
                unique.add(ix + iz * 32)
            if any(abs(value - 0.25) > 0.001 for value in particle["size"]):
                wrong_size += 1
            if particle["codes"] != [1, 0, 0, 0]:
                metadata_mismatch += 1
        missing = len(expected - unique)
        missing_indices = sorted(expected - unique)
        check("received catches completed capture", sample["received"] == sample["completed"])
        check("received delta is exactly 256", sample["received"] - base_rx == COUNT)
        check("completed delta is exactly 256", sample["completed"] - base_done == COUNT)
        check("native count is 256", sample["count"] == COUNT, sample["count"])
        check("native unique centers are 256", len(unique) == COUNT, len(unique))
        check("no missing unit centers", missing == 0, missing)
        check("no malformed centers", wrong_center == 0, wrong_center)
        check("all unit sizes are 0.25m", wrong_size == 0, wrong_size)
        check("all brush metadata is style=1 tint=0 motion=0", metadata_mismatch == 0, metadata_mismatch)
        check("inbox is empty after sustained stream", sample["inbox"] is False)
        check("busy is false after sustained stream", sample["busy"] is False)
        check("sender reports 256 commands", sender.get("commands") == COUNT, sender)
        check(
            "sustained interval was at least requested cadence",
            elapsed >= (COUNT - 1) * options.interval * 0.90,
            elapsed,
        )
        report["success"] = True
        report["baseline"] = baseline
        report["final"] = sample
        report["sender"] = sender
        report["elapsedSeconds"] = elapsed
        report["delta"] = {"received": sample["received"] - base_rx, "completed": sample["completed"] - base_done}
        report["missingIndices"] = missing_indices
        report["geometry"] = "first 256 non-adjacent Unit centers from Stage4Capacity grid; no merge possible"
    except Exception as exc:
        report["error"] = str(exc)
        raise
    finally:
        options.output.parent.mkdir(parents=True, exist_ok=True)
        options.output.write_text(
            json.dumps(report, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )


if __name__ == "__main__":
    main()
