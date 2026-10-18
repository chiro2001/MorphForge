#!/usr/bin/env python3
"""Single-commit rate regression and pose-invariant native particle texture checks."""
import json
import time

from check_stage2 import OUT,box,entry,native_center,pipeline,pull_artifacts,remote_file
from unity_remote import DEFAULT_PROJECT,pwsh_literal,run_pwsh


def probe(name,*args):
    r=pipeline("run_script","--file","AgentScripts/ParticleStage2PoseCheck.cs","--entry","ParticleStage2PoseCheck."+name,
               "--args",json.dumps(list(args)),"--timeout_ms","60000")
    return json.loads(r["result"])


def main():
    report={"success":False,"protocol":"MF2/single-commit","checks":[],"captures":[],"senders":[],"rates":[]}
    entered=False;started=False;seq=0;serial=0;expected_done=0
    def check(name,condition):
        report["checks"].append({"name":name,"pass":bool(condition)});print(("PASS " if condition else "FAIL ")+name,flush=True)
        if not condition:raise AssertionError(name)
    def send(commands,interval=.25):
        nonlocal seq,serial,expected_done
        serial+=1;path=f"Docs/Stage2/extended-{serial:03d}-commands.json";remote_file(path,commands)
        args=["tools/send_stage2.py","--seq",str(seq),"--interval",str(interval),"script","--input",path]
        r=run_pwsh("Set-Location -LiteralPath "+pwsh_literal(DEFAULT_PROJECT)+"; & python "+" ".join(pwsh_literal(a) for a in args)+"; exit $LASTEXITCODE",capture_output=True)
        if r.returncode:raise RuntimeError(r.stderr.decode(errors="replace"))
        sent=json.loads(r.stdout.decode("utf-8-sig"));report["senders"].append(sent);seq=sent["nextSeq"];expected_done+=len(commands)
    def capture(label,expected,images=False):
        path=f"Docs/Stage2/{label}-expected.json";remote_file(path,{"boxes":expected})
        snap=json.loads(entry("Capture",label,path,images));report["captures"].append(snap)
        check(label+": every command received/completed",snap["received"]==expected_done and snap["completed"]==expected_done)
        check(label+": idle with expected count",not snap["busy"] and not snap["inboxReady"] and snap["count"]==len(expected))
        actual=list(snap["particles"])
        for b in expected:
            p=next((p for p in actual if max(abs(a-z) for a,z in zip(p["position"],native_center(b)))<2e-4),None)
            check(label+": geometry/metadata "+str(b["min"]),p is not None and max(abs(a-z*.25) for a,z in zip(p["size"],b["size"]))<2e-4 and p["codes"]==[b["styleId"]&63,b["styleId"]>>6,b["tintId"],b["motionPhase"]])
            if p is not None:actual.remove(p)
        if images:check(label+": rendered reference",snap["foregroundPixels"]>100 and snap["referenceMeanAbsoluteError"]<1.5)
    try:
        status=pipeline("editor_status")
        if status["playMode"]!="stopped" or status["compiling"]:raise RuntimeError("Requires idle Edit Mode")
        pipeline("set_autotick","--enable","true")
        if entry("ActiveScene")!="Assets/MorphForge/Scenes/MorphForge_Stage2.unity":raise RuntimeError("Requires Stage2 scene")
        pipeline("editor_play");entered=True;entry("StartOsc");started=True
        send([{"op":"clear"}])
        samples=[box((128,128,128),(20,10,1),2),box((151,128,128),(2,3,4),3),box((123,128,128),(2,2,2),4)]
        send([dict(op="box",**b) for b in samples]);capture("texture-fixture",samples,True)
        texture=probe("Check");report["texture"]=texture
        check("UV stable under translation, yaw, tilt, scale and combined pose",texture["success"])
        check("Negative control detects legacy world-space texture drift",texture["legacyTranslationError"]>.2)
        for interval in (.25,.2):
            send([{"op":"clear"}],interval)
            before=probe("Stats")
            for cycle in range(4):
                stairs=[box((128+i*2,128,128),(2,i+1,2),(i+cycle)%4+1) for i in range(8)]
                send([dict(op="box",**b) for b in stairs],interval)
                capture(f"rate{int(interval*1000)}-c{cycle}-build",stairs,cycle==0)
                recolor=[dict(b,styleId=b["styleId"]%4+1) for b in stairs]
                send([dict(op="box",**b) for b in recolor],interval)
                capture(f"rate{int(interval*1000)}-c{cycle}-recolor",recolor)
                send([{"op":"remove","min":b["min"],"size":b["size"]} for b in reversed(stairs)],interval)
                capture(f"rate{int(interval*1000)}-c{cycle}-erase",[])
            after=probe("Stats")
            report["rates"].append({"commandInterval":interval,"commands":96,"before":before,"after":after,
                "observedGameFPS":(after["frame"]-before["frame"])/(after["realtime"]-before["realtime"])})
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
        OUT.mkdir(parents=True,exist_ok=True);(OUT/"extended-validation.json").write_text(json.dumps(report,ensure_ascii=False,indent=2)+"\n")
        print("RESULT "+json.dumps({"success":report["success"],"checks":len(report["checks"])}),flush=True)
    return 0 if report["success"] else 1


if __name__=="__main__":raise SystemExit(main())
