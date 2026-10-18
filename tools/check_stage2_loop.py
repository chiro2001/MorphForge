#!/usr/bin/env python3
"""Check uninterrupted MF2 loops against every native completion's particle count."""
import json
import time
from check_stage2 import OUT,entry,pipeline,pull_artifacts
from unity_remote import DEFAULT_PROJECT,pwsh_literal,run_pwsh


def expected_count(completed):
    if completed<=2:return 0
    step=(completed-3)%24
    return step+1 if step<8 else 8 if step<16 else 23-step


def main():
    results=[]
    for interval in (.25,.2):
        result={"interval":interval,"cycles":3,"success":False};entered=False;started=False
        try:
            if pipeline("editor_status")["playMode"]!="stopped":raise RuntimeError("Requires Edit Mode")
            if entry("ActiveScene")!="Assets/MorphForge/Scenes/MorphForge_Stage2.unity":raise RuntimeError("Requires Stage2 scene")
            pipeline("set_autotick","--enable","true");pipeline("editor_play");entered=True
            entry("StartOsc");started=True
            args=["tools/loop_stage2.py","--port","9012","--interval",str(interval),"--cycles","3"]
            r=run_pwsh("Set-Location -LiteralPath "+pwsh_literal(DEFAULT_PROJECT)+"; & python -u "+" ".join(pwsh_literal(a) for a in args)+"; exit $LASTEXITCODE",capture_output=True)
            if r.returncode:raise RuntimeError(r.stderr.decode(errors="replace"))
            time.sleep(.3)
            result["capture"]=json.loads(entry("Capture",f"loop{int(interval*1000)}-final","",False))
            result["senderOutput"]=r.stdout.decode("utf-8-sig")
        except Exception as e:result["error"]=str(e)
        finally:
            if started:
                try:entry("StopOsc")
                except Exception as e:result["cleanupReceiverError"]=str(e)
            if entered:
                try:pipeline("editor_stop")
                except Exception as e:result["cleanupPlayError"]=str(e)
            try:
                pull_artifacts();trace=json.loads((OUT/"trace.json").read_text())
                (OUT/f"loop{int(interval*1000)}-trace.json").write_text(json.dumps(trace,indent=2)+"\n")
                completed={}
                for event in trace:
                    count=int(event["completed"])
                    if count and count not in completed:completed[count]={"actual":event["count"],"expected":expected_count(count)}
                capture=result.get("capture",{})
                result["completionCounts"]=completed
                result["checks"]={"74CommandsReceivedCompleted":capture.get("received")==74 and capture.get("completed")==74,
                    "EveryCompletionObserved":set(completed)==set(range(1,75)),
                    "NativeCountCorrectAfterEveryCommand":all(v["actual"]==v["expected"] for v in completed.values()),
                    "FinalEmpty":capture.get("count")==0 and not capture.get("busy",True) and not capture.get("inboxReady",True)}
                result["success"]=not any(k=="error" or k.endswith("Error") for k in result) and all(result["checks"].values())
            except Exception as e:result["artifactError"]=str(e)
        results.append(result);print(json.dumps({"interval":interval,"success":result["success"],"checks":result.get("checks"),"error":result.get("error")}),flush=True)
        if not result["success"]:break
    report={"success":len(results)==2 and all(r["success"] for r in results),"scope":"Unity native OSC; uninterrupted loops, one UDP packet per command","results":results}
    (OUT/"loop-validation.json").write_text(json.dumps(report,ensure_ascii=False,indent=2)+"\n")
    return 0 if report["success"] else 1


if __name__=="__main__":raise SystemExit(main())
