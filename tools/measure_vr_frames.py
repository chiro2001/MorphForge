#!/usr/bin/env python3
"""Read SteamVR compositor timings without attaching to or modifying VRChat.

Run in the same Windows interactive session as SteamVR. The application frame
interval is distinct from headset refresh and compositor/reprojection cadence.
"""
import argparse
import ctypes
import datetime
import json
import os
from pathlib import Path
import statistics
import time
import traceback


def distribution(values):
    values = sorted(values)
    if not values:
        return None
    return {"mean": statistics.mean(values), "median": statistics.median(values),
            "p95": values[int((len(values) - 1) * .95)], "max": values[-1]}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--label", required=True)
    p.add_argument("--seconds", type=float, default=20)
    p.add_argument("--expected-pid", type=int, required=True)
    a = p.parse_args()
    if not 2 <= a.seconds <= 60:
        p.error("Use a 2..60 second sampling window")
    a.output.parent.mkdir(parents=True, exist_ok=True)
    report = {"label": a.label, "success": False, "status": "initializing",
              "startedUtc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "expectedScenePid": a.expected_pid, "readOnly": True}
    if os.name == "nt":
        session = ctypes.c_ulong()
        ctypes.windll.kernel32.ProcessIdToSessionId(os.getpid(), ctypes.byref(session))
        report["sessionId"] = session.value
        token = ctypes.c_void_p()
        if ctypes.windll.advapi32.OpenProcessToken(ctypes.c_void_p(-1), 8, ctypes.byref(token)):
            for info_class, key in [(20, "elevated"), (29, "appContainer")]:
                value, size = ctypes.c_ulong(), ctypes.c_ulong()
                if ctypes.windll.advapi32.GetTokenInformation(token, info_class, ctypes.byref(value), 4, ctypes.byref(size)):
                    report[key] = bool(value.value)
            ctypes.windll.kernel32.CloseHandle(token)
    def save():
        temp = a.output.with_suffix(".tmp")
        temp.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        temp.replace(a.output)
    save()
    initialized = False
    try:
        import openvr
        # Never start SteamVR or VRChat. The utility only joins an existing runtime.
        if not openvr.isHmdPresent():
            raise RuntimeError("No active headset detected")
        system = openvr.init(openvr.VRApplication_Background)
        initialized = True
        compositor = openvr.VRCompositor()
        pid = compositor.getCurrentSceneFocusProcess()
        if pid != a.expected_pid:
            raise RuntimeError(f"Scene focus PID {pid} differs from VRChat {a.expected_pid}")
        report.update(status="sampling", scenePid=pid,
                      displayHz=system.getFloatTrackedDeviceProperty(0, openvr.Prop_DisplayFrequency_Float))
        save()
        timings = (openvr.Compositor_FrameTiming * 128)()
        for item in timings:
            item.m_nSize = ctypes.sizeof(openvr.Compositor_FrameTiming)
        start = time.monotonic()
        first = compositor.getFrameTiming().m_nFrameIndex
        frames = {}
        while time.monotonic() - start < a.seconds:
            if compositor.getCurrentSceneFocusProcess() != pid:
                raise RuntimeError("VR scene focus changed during sampling")
            n = compositor.getFrameTimings(timings)
            for item in timings[:n]:
                if item.m_nFrameIndex <= first:
                    continue
                frames[item.m_nFrameIndex] = {
                    name.removeprefix("m_"): getattr(item, name)
                    for name, _ in item._fields_ if name != "m_HmdPose"
                }
            time.sleep(.2)
        rows = sorted(frames.values(), key=lambda x: x["nFrameIndex"])
        if len(rows) < 30:
            raise RuntimeError("Too few unique VR application frames")
        intervals = [r["flClientFrameIntervalMs"] for r in rows if 0 < r["flClientFrameIntervalMs"] < 1000]
        report.update(
            success=True, status="complete", wallSeconds=time.monotonic()-start,
            frameCount=len(rows),
            applicationFps=1000/statistics.mean(intervals) if intervals else None,
            applicationFrameIntervalMs=distribution(intervals),
            applicationGpuMs=distribution([r["flTotalRenderGpuMs"] for r in rows]),
            preSubmitGpuMs=distribution([r["flPreSubmitGpuMs"] for r in rows]),
            frameReadyOffsetMs=distribution([r["flNewFrameReadyMs"] for r in rows]),
            framesPresentedMoreThanOnce=sum(r["nNumFramePresents"] > 1 for r in rows),
            droppedFrames=sum(r["nNumDroppedFrames"] for r in rows),
            reprojectionFlagCounts={str(flag): sum(r["nReprojectionFlags"] == flag for r in rows)
                                    for flag in sorted({r["nReprojectionFlags"] for r in rows})},
            frames=rows,
            scope="SteamVR scene application timing. No injection, process memory read, or desktop Present FPS.")
    except Exception as error:
        report.update(status="failed", error=str(error), errorType=type(error).__name__,
                      traceback=traceback.format_exc())
        raise
    finally:
        if initialized:
            openvr.shutdown()
        report["completedUtc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        save()


if __name__ == "__main__":
    main()
