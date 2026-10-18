#!/usr/bin/env python3
"""MF2 single commit protocol: 3 opcode bits + 5 sequence bits, 10 unchanged payload Ints."""
import argparse
import json
from pathlib import Path
import socket
import struct
import time

from send_stage1 import osc_string,payload,writer_lock

PREFIX="/avatar/parameters/MF2_"


def packet(command,seq):
    if type(seq) is not int or not 0<=seq<32:raise ValueError("seq must be 0..31")
    op,data=payload(command)
    values=[(f"D{i:02d}",v) for i,v in enumerate(data)]+[("Ctl",(seq<<3)|op)]
    messages=[osc_string(PREFIX+n)+osc_string(",i")+struct.pack(">i",v) for n,v in values]
    return b"#bundle\0"+struct.pack(">Q",1)+b"".join(struct.pack(">i",len(m))+m for m in messages)


def transmit(commands,host,port,seq,interval,repeat=False):
    reports=[];previous=None;intervals=[]
    with writer_lock(),socket.socket(socket.AF_INET,socket.SOCK_DGRAM) as sender:
        for command in commands:
            encoded=packet(command,seq);started=time.monotonic()
            sender.sendto(encoded,(host,port))
            if previous is not None:intervals.append(started-previous)
            previous=started
            deadline=started+interval
            while time.monotonic()<deadline:
                time.sleep(min(.04,max(0,deadline-time.monotonic())))
                if repeat and time.monotonic()<deadline:sender.sendto(encoded,(host,port))
            op,data=payload(command)
            reports.append({"command":command,"seq":seq,"control":(seq<<3)|op,"payload":data})
            seq=(seq+1)%32
    return {"sent":True,"protocol":"MF2/single-commit","source":"independent Python UDP OSC process","port":port,
            "commands":len(reports),"nextSeq":seq,"interval":interval,"repeat":repeat,"actualIntervals":intervals,"operations":reports}


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--host",default="127.0.0.1");p.add_argument("--port",type=int,default=9012)
    p.add_argument("--seq",type=int,default=0);p.add_argument("--interval",type=float,default=.25)
    p.add_argument("--repeat",action="store_true");p.add_argument("--output",type=Path);p.add_argument("--dry-run",action="store_true")
    sub=p.add_subparsers(dest="op",required=True)
    for name in ("box","erase","remove"):
        q=sub.add_parser(name);q.add_argument("--min",type=int,nargs=3,required=True);q.add_argument("--size",type=int,nargs=3,required=True)
        if name=="box":q.add_argument("--style",type=int,default=1);q.add_argument("--tint",type=int,default=0);q.add_argument("--motion",type=int,default=0)
    sub.add_parser("clear");q=sub.add_parser("script");q.add_argument("--input",type=Path,required=True)
    a=p.parse_args()
    if a.interval<=0 or not 0<=a.seq<32:p.error("Positive interval and seq 0..31 required")
    if a.op=="script":commands=json.loads(a.input.read_text(encoding="utf-8"))
    elif a.op=="clear":commands=[{"op":"clear"}]
    else:commands=[{"op":a.op,"min":a.min,"size":a.size,"styleId":getattr(a,"style",1),"tintId":getattr(a,"tint",0),"motionPhase":getattr(a,"motion",0)}]
    for c in commands:packet(c,a.seq)
    result={"dryRun":True,"commands":commands,"protocol":"MF2/single-commit"} if a.dry_run else transmit(commands,a.host,a.port,a.seq,a.interval,a.repeat)
    if a.output:a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
    print(json.dumps(result))


if __name__=="__main__":main()
