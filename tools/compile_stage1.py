#!/usr/bin/env python3
"""Compile an empty-scene voxel edit sequence into Stage1 native box commands.

This is a bounded geometry prototype, not the future Draft/Published service.
Every input sequence must start with clear; output is fully checked before sending.
"""
import argparse
import json
from pathlib import Path

from send_stage1 import payload


def descriptor(command):
    b={"min":list(command["min"]),"size":list(command["size"]),
       "styleId":command.get("styleId",1),"tintId":command.get("tintId",0),
       "motionPhase":command.get("motionPhase",0)}
    payload(dict(op="box",**b))
    return b


def intersects(a,b):
    return all(max(p,q)<min(p+s,q+t) for p,s,q,t in zip(a["min"],a["size"],b["min"],b["size"]))


def subtract_box(original,cut):
    """Half-open integer bounds; preserve appearance, return at most six boxes."""
    if not intersects(original,cut):return [descriptor(original)]
    low=original["min"][:];high=[p+s for p,s in zip(low,original["size"])]
    lo=[max(p,q) for p,q in zip(low,cut["min"])]
    hi=[min(p,q+s) for p,q,s in zip(high,cut["min"],cut["size"])]
    result=[]
    for axis in range(3):
        for side in (0,1):
            a=low[:];b=high[:]
            if side==0:b[axis]=lo[axis]
            else:a[axis]=hi[axis]
            if all(x<y for x,y in zip(a,b)):
                result.append(dict(descriptor(original),min=a,size=[y-x for x,y in zip(a,b)]))
        low[axis]=lo[axis];high[axis]=hi[axis]
    return result


def compile_commands(edits,capacity=64):
    if not edits or edits[0].get("op")!="clear":raise ValueError("A prototype edit sequence must start with clear")
    boxes=[];commands=[]
    for edit in edits:
        op=edit["op"]
        if op=="clear":boxes=[];commands.append({"op":"clear"});continue
        if op not in {"box","erase"}:raise ValueError("High-level edits support clear, box and erase")
        region=descriptor(edit)
        victims=[b for b in boxes if intersects(b,region)]
        kept=[b for b in boxes if not intersects(b,region)]
        fragments=[part for b in victims for part in subtract_box(b,region)]
        target=kept+fragments+([region] if op=="box" else [])
        if len(target)>capacity:raise ValueError(f"Edit requires {len(target)} particles; capacity is {capacity}")
        commands.extend({"op":"remove","min":b["min"],"size":b["size"]} for b in victims)
        commands.extend(dict(op="box",**b) for b in fragments)
        if op=="box":commands.append(dict(op="box",**region))
        boxes=target
    return {"commands":commands,"boxes":boxes,"capacity":capacity}


def demo():
    return [{"op":"clear"},
            {"op":"box","min":[128,128,128],"size":[20,10,1],"styleId":2},
            {"op":"erase","min":[138,133,128],"size":[1,1,1]},
            {"op":"box","min":[150,128,128],"size":[2,3,4],"styleId":3}]


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    source=parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--input",type=Path);source.add_argument("--demo",action="store_true")
    parser.add_argument("--output",type=Path,required=True)
    parser.add_argument("--report",type=Path);parser.add_argument("--capacity",type=int,default=64)
    args=parser.parse_args()
    result=compile_commands(demo() if args.demo else json.loads(args.input.read_text(encoding="utf-8")),args.capacity)
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(result["commands"],ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
    if args.report:
        args.report.parent.mkdir(parents=True,exist_ok=True)
        args.report.write_text(json.dumps(result,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
    print(json.dumps({"commands":len(result["commands"]),"targetParticles":len(result["boxes"]),"output":str(args.output)}))


if __name__=="__main__":main()
