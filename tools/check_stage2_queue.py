#!/usr/bin/env python3
"""Two quick local OSC commits must preserve the active work buffer and queue the next."""
import json
import time
from check_stage2 import OUT,box,entry,native_center,pipeline,pull_artifacts,remote_file
from unity_remote import DEFAULT_PROJECT,pwsh_literal,run_pwsh


def main():
    report={"success":False,"scope":"Editor local burst; tests queue isolation, not VRChat network throughput"}
    entered=False;started=False
    try:
        if pipeline("editor_status")["playMode"]!="stopped":raise RuntimeError("Requires Edit Mode")
        if entry("ActiveScene")!="Assets/MorphForge/Scenes/MorphForge_Stage2.unity":raise RuntimeError("Requires Stage2 scene")
        boxes=[box((128,128,128),(2,3,1),1,63,21),box((132,128,128),(3,1,4),4095,17,63)]
        remote_file("Docs/Stage2/queue-commands.json",[dict(op="box",**b) for b in boxes])
        remote_file("Docs/Stage2/queue-expected.json",{"boxes":boxes})
        pipeline("set_autotick","--enable","true");pipeline("editor_play");entered=True
        entry("StartOsc");started=True
        args=["tools/send_stage2.py","--interval","0.025","script","--input","Docs/Stage2/queue-commands.json"]
        r=run_pwsh("Set-Location -LiteralPath "+pwsh_literal(DEFAULT_PROJECT)+"; & python "+" ".join(pwsh_literal(a) for a in args)+"; exit $LASTEXITCODE",capture_output=True)
        if r.returncode:raise RuntimeError(r.stderr.decode(errors="replace"))
        report["sender"]=json.loads(r.stdout.decode("utf-8-sig"));time.sleep(.3)
        snap=json.loads(entry("Capture","queue-isolation","Docs/Stage2/queue-expected.json",True));report["capture"]=snap
        checks={"bothReceivedCompleted":snap["received"]==2 and snap["completed"]==2,"twoParticles":snap["count"]==2,"referenceImage":snap["referenceMeanAbsoluteError"]<1.5}
        for i,b in enumerate(boxes):
            p=next((p for p in snap["particles"] if max(abs(a-z) for a,z in zip(p["position"],native_center(b)))<2e-4),None)
            checks[f"immutableBirth{i}"]=p is not None and max(abs(a-z*.25) for a,z in zip(p["size"],b["size"]))<2e-4 and p["codes"]==[b["styleId"]&63,b["styleId"]>>6,b["tintId"],b["motionPhase"]]
        report["checks"]=checks
    except Exception as e:report["error"]=str(e)
    finally:
        if started:
            try:entry("StopOsc")
            except Exception as e:report["cleanupReceiverError"]=str(e)
        if entered:
            try:pipeline("editor_stop")
            except Exception as e:report["cleanupPlayError"]=str(e)
        try:
            pull_artifacts();trace=json.loads((OUT/"trace.json").read_text())
            report["queueObservedDuringBusy"]=any(t["busy"] and t["inboxReady"] for t in trace)
            (OUT/"queue-trace.json").write_text(json.dumps(trace,indent=2)+"\n")
            report["success"]=not any(k.endswith("Error") or k=="error" for k in report) and all(report.get("checks",{}).values()) and report["queueObservedDuringBusy"]
        except Exception as e:report["artifactError"]=str(e)
        (OUT/"queue-validation.json").write_text(json.dumps(report,ensure_ascii=False,indent=2)+"\n")
    print(json.dumps({k:v for k,v in report.items() if k not in ("sender","capture")},ensure_ascii=False,indent=2))
    return 0 if report["success"] else 1


if __name__=="__main__":raise SystemExit(main())
