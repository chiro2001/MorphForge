#!/usr/bin/env python3
"""Exercise real UDP OSC through Gesture Manager and compare GPU geometry to a CPU reference."""

import base64
import json
from pathlib import Path
import subprocess
import sys
import time

from unity_remote import DEFAULT_PROJECT, pwsh_literal, run_pwsh

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "docs/probe14"
OUTPUT_FILES = set()


def pipeline(*args):
    result = subprocess.run([sys.executable, str(ROOT/"tools/unity_remote.py"), "--check-result", *args], capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(result.stdout + result.stderr)
    envelope = json.loads(result.stdout)
    value = envelope["data"]["result"]
    if isinstance(value, dict) and value.get("success") is False:
        raise RuntimeError(value)
    return value


def evaluate(expression):
    value = pipeline("eval", expression, "--timeout", "60")
    if not value.get("success"):
        raise RuntimeError(value)
    return value.get("result")


def send(*args):
    arguments = ["tools/send_probe.py", *args]
    script = "Set-Location -LiteralPath " + pwsh_literal(DEFAULT_PROJECT) + "; & 'python' "
    script += " ".join(pwsh_literal(a) for a in arguments) + "; exit $LASTEXITCODE"
    result = run_pwsh(script, capture_output=True)
    if result.returncode:
        raise RuntimeError(result.stderr.decode("utf-8", errors="replace"))
    return json.loads(result.stdout.decode("utf-8-sig"))


def capture(label, input_path=""):
    expression = "return MorphForge.Editor.Probe14Harness.Capture(" + json.dumps(label) + "," + json.dumps(input_path) + ");"
    snapshot = json.loads(evaluate(expression))
    for key in ("actualImage", "referenceImage", "input"):
        if snapshot.get(key):
            OUTPUT_FILES.add(snapshot[key])
    OUTPUT_FILES.add("Docs/Probe14/"+label+".json")
    return snapshot


def pull_files():
    OUT.mkdir(parents=True, exist_ok=True)
    for path in sorted(OUTPUT_FILES):
        full = DEFAULT_PROJECT + "\\" + path.replace("/", "\\")
        result = run_pwsh("[Convert]::ToBase64String([IO.File]::ReadAllBytes("+pwsh_literal(full)+"))", capture_output=True)
        if result.returncode:
            raise RuntimeError(result.stderr.decode("utf-8", errors="replace"))
        (OUT/Path(path).name).write_bytes(base64.b64decode(result.stdout.decode("utf-8-sig").strip()))


def carrier_hash():
    path = DEFAULT_PROJECT + r"\Assets\MorphForge\Probe14\Carrier.asset"
    result = run_pwsh("(Get-FileHash -LiteralPath " + pwsh_literal(path) + " -Algorithm SHA256).Hash", capture_output=True)
    if result.returncode:
        raise RuntimeError(result.stderr.decode("utf-8", errors="replace"))
    return result.stdout.decode("utf-8-sig").strip().lower()


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    report = {
        "environment": "Unity Play Mode + Gesture Manager 3.9.9 native UDP OSC",
        "scope": "Steps 1 and 4 only; direct parameters; local-only; not VRChat client or multiplayer validation",
        "osc_input_port": 9010,
        "checks": [],
        "captures": [],
        "success": False,
    }

    def check(name, condition):
        report["checks"].append({"name": name, "pass": bool(condition)})
        print(("PASS: " if condition else "FAIL: ") + name, flush=True)
        if not condition:
            raise AssertionError(name)

    entered_play = False
    receiver_started = False
    try:
        status = pipeline("editor_status")
        if status["playMode"] != "stopped":
            raise RuntimeError("The probe runner will not take over an existing Play Mode session")
        if status["compiling"]:
            raise RuntimeError("Wait for compilation before the probe")
        active_scene=evaluate("return UnityEngine.SceneManagement.SceneManager.GetActiveScene().path;")
        if active_scene != "Assets/MorphForge/Scenes/MorphForge_Probe14.unity":
            raise RuntimeError("Open MorphForge_Probe14.unity before this test")
        report["carrier_hash_before"] = carrier_hash()
        pipeline("editor_play")
        entered_play = True
        for _ in range(30):
            status = pipeline("editor_status")
            if status["playMode"] == "playing":
                break
            time.sleep(.25)
        else:
            raise RuntimeError("Editor did not enter Play Mode")
        report["receiver"] = evaluate("return MorphForge.Editor.Probe14Harness.StartOsc();")
        receiver_started = True

        signals = []
        for i, value in enumerate([0.0, .25, .5, .75, 1.0]):
            sent = send("signal", "--value", str(value))
            check(f"Signal {value}: external sender sent UDP", sent["sent"])
            time.sleep(.3)
            snapshot = capture(f"signal-{i}")
            report["captures"].append(snapshot)
            signals.append(snapshot)
            check(f"Signal {value}: received by actual emulated FX", abs(snapshot["control"]-value) < 1e-5 and snapshot["visible"])
            check(f"Signal {value}: actual visible pixels", snapshot["foregroundPixels"] > 100)
            check(f"Signal {value}: 75 configured probe parameters", snapshot["parameterCount"] == 75)
        check("Signal color changes blue to red", signals[0]["foregroundB"] > signals[0]["foregroundR"]+50 and signals[-1]["foregroundR"] > signals[-1]["foregroundB"]+50)
        check("Signal projected size changes", signals[-1]["foregroundPixels"] > signals[0]["foregroundPixels"]*3)

        for shape, seed in [("tetrahedron", 1), ("roof", 1), ("random", 13579)]:
            label = "geometry-"+shape
            path = "Docs/Probe14/"+label+"-input.json"
            sent = send("geometry", "--shape", shape, "--seed", str(seed), "--output", path)
            check(shape+": external sender sent coordinates", sent["sent"] and sent["coordinates"] == 72)
            time.sleep(.3)
            snapshot = capture(label, path)
            report["captures"].append(snapshot)
            check(shape+": all coordinate parameters match external input", snapshot["coordinateParameterMaxError"] < 1e-5)
            check(shape+": actual geometry has visible area", snapshot["foregroundPixels"] > 500)
            check(shape+": GPU render agrees with independent CPU mesh", snapshot["referenceMeanAbsoluteError"] <= .3)
            check(shape+": fixed carrier CPU vertices and topology unchanged", snapshot["carrierCpuVerticesZero"] and snapshot["carrierIndicesUnchanged"] and snapshot["vertexCount"] == 24)

        send("hide")
        time.sleep(.3)
        hidden = capture("hidden")
        report["captures"].append(hidden)
        check("External hide removes actual signal display", not hidden["visible"] and hidden["foregroundPixels"] == 0)
        report["carrier_hash_after"] = carrier_hash()
        check("Serialized carrier asset unchanged across all external shapes", report["carrier_hash_before"] == report["carrier_hash_after"])
        report["success"] = True
    except Exception as error:
        report["error"] = str(error)
        raise
    finally:
        if receiver_started:
            try:
                evaluate("MorphForge.Editor.Probe14Harness.StopOsc(); return true;")
            except Exception as error:
                report["cleanup_receiver_error"] = str(error)
                report["success"] = False
        if entered_play:
            try:
                pipeline("editor_stop")
            except Exception as error:
                report["cleanup_play_error"] = str(error)
                report["success"] = False
        (OUT/"validation.json").write_text(json.dumps(report, ensure_ascii=False, indent=2)+"\n")
        pull_files()
        print("Report:", OUT/"validation.json", flush=True)
    return 0 if report["success"] else 1


if __name__ == "__main__":
    sys.exit(main())
