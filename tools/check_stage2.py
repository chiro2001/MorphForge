#!/usr/bin/env python3
"""End-to-end UDP/GM/FX/native-particle validation; never emits or edits receiver state in C#."""
import base64
import io
import itertools
import json
from pathlib import Path
import subprocess
import sys
import time
import zipfile

from unity_remote import DEFAULT_PROJECT,pwsh_literal,run_pwsh

ROOT=Path(__file__).resolve().parents[1]
OUT=ROOT/"docs/stage2"


def pipeline(*args):
    r=subprocess.run([sys.executable,str(ROOT/"tools/unity_remote.py"),"--check-result",*args],capture_output=True,text=True)
    if r.returncode:raise RuntimeError(r.stdout+r.stderr)
    v=json.loads(r.stdout)["data"]["result"]
    if isinstance(v,dict) and v.get("success") is False:raise RuntimeError(str(v))
    return v


def entry(name,*args):
    r=pipeline("run_script","--file","AgentScripts/ParticleStage2Entry.cs","--entry","ParticleStage2Entry."+name,
               "--args",json.dumps(list(args)),"--timeout_ms","60000")
    if not r.get("success"):raise RuntimeError(r)
    return r.get("result")


def remote_file(path,value):
    raw=json.dumps(value,ensure_ascii=False,indent=2).encode()
    b64=base64.b64encode(raw).decode()
    ps=f"$p=Join-Path {pwsh_literal(DEFAULT_PROJECT)} {pwsh_literal(path)}; New-Item -ItemType Directory -Path (Split-Path $p) -Force | Out-Null; [IO.File]::WriteAllBytes($p,[Convert]::FromBase64String('{b64}'))"
    r=run_pwsh(ps,capture_output=True)
    if r.returncode:raise RuntimeError(r.stderr.decode(errors="replace"))


def pull_artifacts():
    ps=f"""$src=Join-Path {pwsh_literal(DEFAULT_PROJECT)} 'Docs\\Stage2';
$dir=Join-Path $env:USERPROFILE 'tmp\\morphforge';New-Item -ItemType Directory -Path $dir -Force | Out-Null;
$zip=Join-Path $dir 'stage2-evidence.zip';
$files=Get-ChildItem -LiteralPath $src -File | Where-Object {{$_.Extension -in @('.json','.png')}};
Compress-Archive -LiteralPath $files.FullName -DestinationPath $zip -Force;
[Convert]::ToBase64String([IO.File]::ReadAllBytes($zip))"""
    r=run_pwsh(ps,capture_output=True)
    if r.returncode:raise RuntimeError(r.stderr.decode(errors="replace"))
    z=zipfile.ZipFile(io.BytesIO(base64.b64decode(r.stdout.decode("utf-8-sig").strip())))
    OUT.mkdir(parents=True,exist_ok=True)
    for entry in z.infolist():
        name=Path(entry.filename).name
        if name and Path(name).suffix in {".json",".png"}:(OUT/name).write_bytes(z.read(entry))


def box(origin,size=(1,1,1),style=1,tint=0,motion=0):
    return {"min":list(origin),"size":list(size),"styleId":style,"tintId":tint,"motionPhase":motion}


def native_center(b):return [(p+s/2-128)*.25 for p,s in zip(b["min"],b["size"])]


def main():
    OUT.mkdir(parents=True,exist_ok=True)
    report={"protocol":"MF2/single-commit","interval":.25,"success":False,"environment":"Unity 2022.3 Play Mode + Gesture Manager native OSC; no VRChat client", "checks":[],"captures":[],"senderReports":[],"knownExclusions":["multiplayer","remote packet loss","runtime AI","shared motion","texture packs","GPU instancing","VRChat network throughput"]}
    entered=False;started=False;seq=0;serial=0;expectedCompleted=0

    def check(name,condition):
        report["checks"].append({"name":name,"pass":bool(condition)})
        print(("PASS " if condition else "FAIL ")+name,flush=True)
        if not condition:raise AssertionError(name)

    def send(commands,reuse_seq=None):
        nonlocal seq,serial,expectedCompleted
        serial+=1;name=f"commands-{serial:03d}"
        path=f"Docs/Stage2/{name}.json";remote_file(path,commands)
        args=["tools/send_stage2.py","--seq",str(seq if reuse_seq is None else reuse_seq),"--interval","0.25","--output",f"Docs/Stage2/{name}-sent.json","script","--input",path]
        script="Set-Location -LiteralPath "+pwsh_literal(DEFAULT_PROJECT)+"; & 'python' "+" ".join(pwsh_literal(a) for a in args)+"; exit $LASTEXITCODE"
        r=run_pwsh(script,capture_output=True)
        if r.returncode:raise RuntimeError(r.stderr.decode(errors="replace"))
        sent=json.loads(r.stdout.decode("utf-8-sig"));report["senderReports"].append(sent)
        if reuse_seq is None:seq=sent["nextSeq"];expectedCompleted+=len(commands)
        return sent

    def capture(label,expected,images=False):
        path=f"Docs/Stage2/{label}-expected.json";remote_file(path,{"boxes":expected})
        snap=json.loads(entry("Capture",label,path,images));report["captures"].append(snap)
        check(label+": receiver finished",not snap["busy"] and not snap["pumpActive"] and not snap["triggerEnabled"] and not snap["inboxReady"])
        check(label+": every submission received and completed",snap["received"]==expectedCompleted and snap["completed"]==expectedCompleted)
        check(label+": particle count",snap["count"]==len(expected))
        actual=list(snap["particles"])
        for b in expected:
            center=native_center(b)
            found=next((p for p in actual if max(abs(a-z) for a,z in zip(p["position"],center))<2e-4),None)
            check(label+": expected center "+str(b["min"]),found is not None)
            if found is None:continue
            actual.remove(found)
            check(label+": birth size "+str(b["size"]),max(abs(a-z*.25) for a,z in zip(found["size"],b["size"]))<2e-4)
            check(label+": birth metadata",found["codes"]==[b["styleId"]&63,b["styleId"]>>6,b.get("tintId",0),b.get("motionPhase",0)])
            check(label+": infinite native lifetime",float(found["remainingLifetime"])==float("inf"))
        if images and expected:
            check(label+": native rendered pixels",snap["foregroundPixels"]>100)
            check(label+": independent CPU reference image",snap["referenceMeanAbsoluteError"]<1.5)
        return snap

    def clear(label):
        send([{"op":"clear"}]);return capture(label,[])

    try:
        status=pipeline("editor_status")
        if status["playMode"]!="stopped" or status["compiling"]:raise RuntimeError("Runner requires idle Edit Mode and does not take over existing Play Mode")
        pipeline("set_autotick","--enable","true")
        if entry("ActiveScene")!="Assets/MorphForge/Scenes/MorphForge_Stage2.unity":raise RuntimeError("Open generated Stage2 scene first")
        pipeline("editor_play");entered=True
        receiver=json.loads(entry("StartOsc"));started=True;report["receiver"]=receiver
        check("Native Store begins empty",receiver["initialParticleCount"]==0)
        check("OSC source fields are Int",receiver["sourceTypes"]==["Int"]*10)
        clear("initial-clear")

        edges=[]
        for value in [0,63,64,127,128,254,255]:
            b=box((value,128,128),style=4095,tint=63,motion=63);send([dict(op="box",**b)]);edges.append(b)
            s=capture(f"integer-{value}",edges)
            check(f"Int {value} copied un-clamped",s["source"][0]==value and s["work"][0]==value)
        clear("after-integers")

        a=box((128,128,128));wall=box((132,128,128),(20,10,1),2);solid=box((120,128,128),(2,3,4),3)
        send([dict(op="box",**a),dict(op="box",**wall),dict(op="box",**solid)])
        capture("cuboids",[a,wall,solid],True)
        faces=json.loads(entry("CaptureFaces","cuboid-faces","Docs/Stage2/cuboids-expected.json"));report["faceViews"]=faces
        for face in faces["views"]:
            check("Six faces visible: "+face["face"],face["foregroundPixels"]>100)
            check("Six faces match reference: "+face["face"],face["referenceMeanAbsoluteError"]<1.5)
        time.sleep(1)
        capture("retained-without-osc",[a,wall,solid])
        send([{"op":"remove","min":wall["min"],"size":wall["size"]}])
        capture("wall-removed",[a,solid],True)
        time.sleep(.7);capture("wall-not-revived",[a,solid])

        sent=send([dict(op="box",**solid)])
        capture("overwrite",[a,solid])
        send([dict(op="box",**solid)],reuse_seq=sent["operations"][0]["seq"])
        capture("duplicate-sequence",[a,solid])
        recolor=box(solid["min"],solid["size"],4)
        send([dict(op="box",**recolor)])
        capture("recolor",[a,recolor],True)
        resized=box(solid["min"],(3,5,2),2)
        send([{"op":"remove","min":solid["min"],"size":solid["size"]},dict(op="box",**resized)])
        capture("resize-via-remove-set",[a,resized],True)
        clear("before-neighbors")

        neighbors=[box(p) for p in itertools.product(range(126,129),repeat=3)]
        send([dict(op="box",**b) for b in neighbors]);capture("neighbors27",neighbors,True)
        send([{"op":"erase","min":[127,127,127],"size":[1,1,1]}])
        remaining=[b for b in neighbors if b["min"]!=[127,127,127]]
        capture("center-erased-neighbors26",remaining,True)
        send([{"op":"erase","min":[126,126,126],"size":[3,1,3]}])
        remaining=[b for b in remaining if b["min"][1]!=126]
        capture("region-erased",remaining,True)
        clear("before-wall-hole")

        full=box((0,0,0),(256,256,256),4095,63,63)
        send([dict(op="box",**full)]);capture("max-size256",[full],True)
        clear("after-max-size")

        original=box((128,128,128),(20,10,1),2)
        send([dict(op="box",**original)]);capture("wall-full",[original],True)
        fragments=[box((128,128,128),(10,10,1),2),box((139,128,128),(9,10,1),2),box((138,128,128),(1,5,1),2),box((138,134,128),(1,4,1),2)]
        send([{"op":"remove","min":original["min"],"size":original["size"]}]+[dict(op="box",**b) for b in fragments])
        capture("wall-hole",fragments,True)
        check("Wall minus one voxel has volume 199",sum(b["size"][0]*b["size"][1]*b["size"][2] for b in fragments)==199)
        clear("before-adjacent")
        large=box((128,128,128),(20,10,1),2);small=box((148,132,128),style=3)
        send([dict(op="box",**large),dict(op="box",**small),{"op":"remove","min":small["min"],"size":small["size"]}])
        capture("small-erased-large-kept",[large],True)
        clear("before-capacity")

        capacity=[box(p,style=3) for p in itertools.product(range(124,128),repeat=3)]
        send([dict(op="box",**b) for b in capacity]);capture("capacity64",capacity,True)
        victim=capacity[0];send([{"op":"remove","min":victim["min"],"size":victim["size"]}]);capture("capacity-free63",capacity[1:])
        replacement=box((130,130,130),style=4);send([dict(op="box",**replacement)]);capture("capacity-reused64",capacity[1:]+[replacement],True)
        overflow=box((132,132,132));send([dict(op="box",**overflow)])
        overflow_snap=json.loads(entry("Capture","capacity-overflow","",False))
        report["captures"].append(overflow_snap);check("Overflow never exceeds native capacity",overflow_snap["count"]<=64)
        clear("final-clear")
        report["success"]=True
    except Exception as e:
        report["error"]=str(e);print("ERROR: "+str(e),flush=True)
    finally:
        if started:
            try:entry("StopOsc")
            except Exception as e:report["cleanupReceiverError"]=str(e);report["success"]=False
        if entered:
            try:pipeline("editor_stop")
            except Exception as e:report["cleanupPlayError"]=str(e);report["success"]=False
        try:pull_artifacts()
        except Exception as e:report["artifactError"]=str(e);report["success"]=False
        (OUT/"validation.json").write_text(json.dumps(report,ensure_ascii=False,indent=2)+"\n")
        print(f"RESULT success={report['success']} checks={len(report['checks'])} report={OUT/'validation.json'}",flush=True)
    return 0 if report["success"] else 1


if __name__=="__main__":sys.exit(main())
