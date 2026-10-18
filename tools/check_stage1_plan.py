#!/usr/bin/env python3
"""Exercise automatically compiled cuboid subtraction through external OSC."""
import json
import math

from check_stage1 import OUT, entry, native_center, pipeline, pull_artifacts, remote_file
from compile_stage1 import compile_commands, demo
from unity_remote import DEFAULT_PROJECT, pwsh_literal, run_pwsh


def main():
    report={"success":False,"scope":"Compiled geometry -> independent UDP -> GM native OSC -> Avatar FX; Editor only","checks":[]}
    entered=False;started=False
    def check(name,condition):
        report["checks"].append({"name":name,"pass":bool(condition)})
        print(("PASS " if condition else "FAIL ")+name,flush=True)
        if not condition:raise AssertionError(name)
    try:
        s=pipeline("editor_status")
        if s["playMode"]!="stopped" or s["compiling"]:raise RuntimeError("Requires idle Edit Mode")
        pipeline("set_autotick","--enable","true")
        if entry("ActiveScene")!="Assets/MorphForge/Scenes/MorphForge_Stage1.unity":raise RuntimeError("Requires Stage1 scene")
        plan=compile_commands(demo());report["plan"]=plan
        remote_file("Docs/Stage1/compiled-demo-commands.json",plan["commands"])
        remote_file("Docs/Stage1/compiled-demo-expected.json",{"boxes":plan["boxes"]})
        pipeline("editor_play");entered=True
        entry("StartOsc");started=True
        args=["tools/send_stage1.py","--output","Docs/Stage1/compiled-demo-sent.json","script","--input","Docs/Stage1/compiled-demo-commands.json"]
        r=run_pwsh("Set-Location -LiteralPath "+pwsh_literal(DEFAULT_PROJECT)+"; & python "+" ".join(pwsh_literal(a) for a in args)+"; exit $LASTEXITCODE",capture_output=True)
        if r.returncode:raise RuntimeError(r.stderr.decode(errors="replace"))
        report["sender"]=json.loads(r.stdout.decode("utf-8-sig"))
        snap=json.loads(entry("Capture","compiled-demo","Docs/Stage1/compiled-demo-expected.json",True));report["capture"]=snap
        check("Compiled plan has 5 disjoint boxes and wall volume 199",len(plan["boxes"])==5 and sum(math.prod(b["size"]) for b in plan["boxes"] if b["styleId"]==2)==199)
        check("Native particle count matches compiled target",snap["count"]==len(plan["boxes"]))
        actual=list(snap["particles"])
        for b in plan["boxes"]:
            center=native_center(b)
            p=next((p for p in actual if max(abs(a-z) for a,z in zip(p["position"],center))<2e-4),None)
            check("Native fragment "+str(b["min"]),p is not None)
            check("Native fragment size and metadata",p is not None and max(abs(a-z*.25) for a,z in zip(p["size"],b["size"]))<2e-4 and p["codes"]==[b["styleId"]&63,b["styleId"]>>6,b["tintId"],b["motionPhase"]])
            if p is not None:actual.remove(p)
        check("Receiver settled",not snap["busy"] and not snap["pumpActive"] and not snap["triggerEnabled"])
        check("Compiled scene renders and matches independent geometry",snap["foregroundPixels"]>100 and snap["referenceMeanAbsoluteError"]<1.5)
        report["success"]=True
    except Exception as e:
        report["error"]=str(e);print("ERROR "+str(e),flush=True)
    finally:
        if started:
            try:entry("StopOsc")
            except Exception as e:report["cleanupReceiverError"]=str(e);report["success"]=False
        if entered:
            try:pipeline("editor_stop")
            except Exception as e:report["cleanupPlayError"]=str(e);report["success"]=False
        try:pull_artifacts()
        except Exception as e:report["artifactError"]=str(e);report["success"]=False
        OUT.mkdir(parents=True,exist_ok=True)
        (OUT/"compiled-validation.json").write_text(json.dumps(report,ensure_ascii=False,indent=2)+"\n")
    return 0 if report["success"] else 1


if __name__=="__main__":raise SystemExit(main())
