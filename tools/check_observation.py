#!/usr/bin/env python3
"""Verify the saved observation scene and leave the OSC staircase visible in Play Mode."""
import json
from check_stage1 import entry, native_center, pipeline, pull_artifacts, remote_file, OUT
from unity_remote import DEFAULT_PROJECT,pwsh_literal,run_pwsh


def main():
    s=pipeline("editor_status")
    if s["playMode"]!="stopped" or s["compiling"]:raise RuntimeError("Requires idle Edit Mode")
    pipeline("set_autotick","--enable","true")
    if entry("ActiveScene")!="Assets/MorphForge/Scenes/MorphForge_Stage1.unity":raise RuntimeError("Requires Stage1 scene")
    boxes=[{"min":[128+i*2,128,128],"size":[2,i+1,2],"styleId":i%4+1,"tintId":0,"motionPhase":0} for i in range(8)]
    remote_file("Docs/Stage1/observation-commands.json",[{"op":"clear"}]+[dict(op="box",**b) for b in boxes])
    remote_file("Docs/Stage1/observation-expected.json",{"boxes":boxes})
    pipeline("editor_play")
    try:
        pipeline("run_script","--file","AgentScripts/ParticleStage1Live.cs","--entry","ParticleStage1Live.Begin","--timeout_ms","60000")
        args=["tools/send_stage1.py","script","--input","Docs/Stage1/observation-commands.json"]
        r=run_pwsh("Set-Location -LiteralPath "+pwsh_literal(DEFAULT_PROJECT)+"; & python "+" ".join(pwsh_literal(a) for a in args)+"; exit $LASTEXITCODE",capture_output=True)
        if r.returncode:raise RuntimeError(r.stderr.decode(errors="replace"))
        snap=json.loads(entry("Capture","observation-particles","Docs/Stage1/observation-expected.json",True))
        state=json.loads(pipeline("run_script","--file","AgentScripts/ParticleStage1Scene.cs","--entry","ParticleStage1Scene.Reveal","--timeout_ms","60000")["result"])
        checks={"eightParticles":snap["count"]==8,"geometryMatchesReference":snap["referenceMeanAbsoluteError"]<1.5,
                "normalLayer":state["layer"]==0,"sceneGizmos":state["sceneView"]["drawGizmos"],
                "sceneParticleEffects":state["sceneView"]["particles"],"notHidden":not state["hidden"],
                "mainCamera":any(c["name"]=="Main Camera" and c["tag"]=="MainCamera" and c["cullingMask"]==-1 for c in state["cameras"])}
        expected_world=[[p+o for p,o in zip(native_center(b),[1,1,0])] for b in boxes]
        checks["worldPositions"]=len(state["particleWorldPositions"])==8 and all(any(max(abs(p-q) for p,q in zip(a,b))<2e-4 for b in state["particleWorldPositions"]) for a in expected_world)
        report={"success":all(checks.values()),"checks":checks,"state":state,"capture":snap,
                "expectedWorldPositions":expected_world,"worldBounds":[[1,1,0],[5,3,.5]],"scope":"Unity Play Mode; leaves final display visible"}
        remote_file("Docs/Stage1/observation-validation.json",report)
        pipeline("screenshot","--view","scene","--width","1280","--height","720","--output","Docs/Stage1/observation-scene.png")
        pipeline("screenshot","--view","game","--width","1280","--height","720","--output","Docs/Stage1/observation-game.png")
        print(json.dumps({"success":report["success"],"checks":checks},ensure_ascii=False,indent=2),flush=True)
        if not report["success"]:raise AssertionError("Observation verification failed")
    finally:
        entry("StopOsc")
        pull_artifacts()
    return 0


if __name__=="__main__":raise SystemExit(main())
