#!/usr/bin/env python3
"""Slow external OSC demonstration; leave the final scene visible for the observer."""
import json
import time
from check_stage1 import entry, pipeline, remote_file
from compile_stage1 import compile_commands
from unity_remote import DEFAULT_PROJECT, pwsh_literal, run_pwsh


def main():
    status=pipeline("editor_status")
    if status["playMode"]!="stopped" or status["compiling"]:raise RuntimeError("Requires idle Edit Mode")
    pipeline("set_autotick","--enable","true")
    if entry("ActiveScene")!="Assets/MorphForge/Scenes/MorphForge_Stage1.unity":raise RuntimeError("Requires Stage1 scene")
    pipeline("editor_play")
    result=pipeline("run_script","--file","AgentScripts/ParticleStage1Live.cs","--entry","ParticleStage1Live.Begin","--timeout_ms","60000")
    if not result.get("success"):raise RuntimeError(result)
    seq=0
    def send(commands,label,hold=5):
        nonlocal seq
        print(label,flush=True)
        remote_file("Docs/Stage1/live-commands.json",commands)
        args=["tools/send_stage1.py","--seq",str(seq),"script","--input","Docs/Stage1/live-commands.json"]
        r=run_pwsh("Set-Location -LiteralPath "+pwsh_literal(DEFAULT_PROJECT)+"; & python "+" ".join(pwsh_literal(a) for a in args)+"; exit $LASTEXITCODE",capture_output=True)
        if r.returncode:raise RuntimeError(r.stderr.decode(errors="replace"))
        seq=json.loads(r.stdout.decode("utf-8-sig"))["nextSeq"]
        time.sleep(hold)
    edits=[{"op":"clear"}]
    send(edits,"0. Empty scene",3)
    wall={"op":"box","min":[128,128,128],"size":[20,10,1],"styleId":2}
    send([wall],"1. Brown wall: 20 x 10 x 1",6);edits.append(wall)
    cut={"op":"erase","min":[136,131,128],"size":[4,4,1]}
    before=compile_commands(edits);edits.append(cut);after=compile_commands(edits)
    send(after["commands"][len(before["commands"]):],"2. Cut a visible 4 x 4 window",7)
    blue={"op":"box","min":[151,128,128],"size":[3,6,3],"styleId":3}
    send([blue],"3. Add a blue 3 x 6 x 3 box; wall stays",6)
    green=dict(blue,styleId=4)
    send([green],"4. Replace blue box with green; no duplicate",6)
    send([{"op":"remove","min":green["min"],"size":green["size"]}],"5. Remove green box; wall stays",5)
    send([{"op":"clear"}],"6. Clear all",4)
    for index in range(8):
        send([{"op":"box","min":[128+index*2,128,128],"size":[2,index+1,2],"styleId":index%4+1}],f"7.{index+1}. Grow staircase",.6)
    snapshot=entry("Capture","live-final","",False)
    remote_file("Docs/Stage1/live-final-report.json",json.loads(snapshot))
    entry("StopOsc")
    print("DONE: final staircase remains in Play Mode; OSC sender stopped.",flush=True)


if __name__=="__main__":main()
