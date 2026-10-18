#!/usr/bin/env python3
"""Send Int OSC to the native Stage1 particle controller; no Unity scripting receiver."""
import argparse
from contextlib import contextmanager
import json
import os
from pathlib import Path
import socket
import struct
import time

PREFIX = "/avatar/parameters/MF1_"


def osc_string(value):
    data = value.encode() + b"\0"
    return data + b"\0" * (-len(data) % 4)


def packet(values):
    messages = []
    for name, value in values:
        if type(value) is not int or not 0 <= value <= 255:
            raise ValueError("Stage1 source parameters are unsigned byte-valued Ints")
        messages.append(osc_string(PREFIX+name)+osc_string(",i")+struct.pack(">i", value))
    return b"#bundle\0"+struct.pack(">Q", 1)+b"".join(struct.pack(">i", len(m))+m for m in messages)


def payload(command):
    op = {"box": 1, "erase": 2, "remove": 3, "clear": 4}[command["op"]]
    if op == 4:
        return op, [0]*10
    origin, size = command["min"], command["size"]
    if len(origin) != 3 or len(size) != 3 or any(type(x) is not int for x in origin+size):
        raise ValueError("min and size must be three integer values")
    if any(not 0 <= p <= 255 or not 1 <= s <= 256 or p+s > 256 for p,s in zip(origin,size)):
        raise ValueError("Box outside the 256-cell grid")
    style = command.get("styleId", 1) if op == 1 else 0
    tint = command.get("tintId", 0)
    motion = command.get("motionPhase", 0)
    if any(type(v) is not int for v in (style,tint,motion)):
        raise ValueError("Metadata must use integer codes")
    if not 0 <= style <= 4095 or not 0 <= tint < 64 or not 0 <= motion < 64:
        raise ValueError("Invalid metadata")
    return op, origin+[s-1 for s in size]+[style&63, style>>6, tint, motion]


@contextmanager
def writer_lock():
    path = Path.home()/"tmp"/"morphforge"/"stage1-osc.lock"
    path.parent.mkdir(parents=True,exist_ok=True)
    with path.open("a+b") as handle:
        handle.seek(0);handle.write(b"0");handle.flush();handle.seek(0)
        if os.name == "nt":
            import msvcrt
            msvcrt.locking(handle.fileno(),msvcrt.LK_NBLCK,1)
        else:
            import fcntl
            fcntl.flock(handle.fileno(),fcntl.LOCK_EX|fcntl.LOCK_NB)
        try:
            yield
        finally:
            handle.seek(0)
            if os.name == "nt":
                msvcrt.locking(handle.fileno(),msvcrt.LK_UNLCK,1)
            else:
                fcntl.flock(handle.fileno(),fcntl.LOCK_UN)


def transmit(commands,host,port,seq,prepare,execute):
    reports=[]
    with writer_lock(),socket.socket(socket.AF_INET,socket.SOCK_DGRAM) as sender:
        for command in commands:
            op,data=payload(command)
            prep=op+(seq<<4);fire=prep+8
            for ctl,hold in [(prep,prepare),(fire,execute)]:
                values=[(f"D{i:02d}",v) for i,v in enumerate(data)]+[("Ctl",ctl)]
                encoded=packet(values);end=time.monotonic()+hold
                while True:
                    sender.sendto(encoded,(host,port))
                    if time.monotonic()>=end:break
                    time.sleep(min(.04,max(0,end-time.monotonic())))
            reports.append({"command":command,"seq":seq,"payload":data,"prepareControl":prep,"executeControl":fire})
            seq=(seq+1)%16
    return {"sent":True,"source":"independent Python UDP OSC process","type":"i","port":port,"commands":len(reports),"nextSeq":seq,"operations":reports}


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--host",default="127.0.0.1");p.add_argument("--port",type=int,default=9012)
    p.add_argument("--seq",type=int,default=0);p.add_argument("--prepare",type=float,default=.25);p.add_argument("--execute",type=float,default=.6)
    p.add_argument("--output",type=Path);p.add_argument("--dry-run",action="store_true")
    sub=p.add_subparsers(dest="op",required=True)
    for name in ["box","erase","remove"]:
        q=sub.add_parser(name);q.add_argument("--min",type=int,nargs=3,required=True);q.add_argument("--size",type=int,nargs=3,required=True)
        if name=="box":q.add_argument("--style",type=int,default=1);q.add_argument("--tint",type=int,default=0);q.add_argument("--motion",type=int,default=0)
    sub.add_parser("clear")
    q=sub.add_parser("script");q.add_argument("--input",type=Path,required=True)
    a=p.parse_args()
    if not 0<=a.seq<16 or a.prepare<=0 or a.execute<=0:p.error("Positive hold times and seq 0..15 required")
    if a.op=="script":commands=json.loads(a.input.read_text(encoding="utf-8"))
    elif a.op=="clear":commands=[{"op":"clear"}]
    else:commands=[{"op":a.op,"min":a.min,"size":a.size,"styleId":getattr(a,"style",1),"tintId":getattr(a,"tint",0),"motionPhase":getattr(a,"motion",0)}]
    for c in commands:payload(c)
    result={"dryRun":True,"commands":commands,"payloads":[payload(c) for c in commands]} if a.dry_run else transmit(commands,a.host,a.port,a.seq,a.prepare,a.execute)
    if a.output:a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
    print(json.dumps(result))


if __name__=="__main__":main()
