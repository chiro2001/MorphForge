#!/usr/bin/env python3
"""Continuous two-phase OSC test: one UDP state every slot, until a stop file appears."""
import argparse
import datetime
import json
from pathlib import Path
import socket
import time

from send_stage1 import packet,payload,writer_lock


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--port",type=int,default=9000);p.add_argument("--slot",type=float,default=.2)
    p.add_argument("--seq",type=int,default=4)
    a=p.parse_args()
    if a.slot<=0 or not 0<=a.seq<16:p.error("Positive slot and seq 0..15 required")
    out=Path.home()/"tmp"/"morphforge";out.mkdir(parents=True,exist_ok=True)
    stop=out/"stage1-loop.stop";status=out/"stage1-loop-status.json"
    stop.unlink(missing_ok=True)
    seq=a.seq;cycle=0;packets=0;commands=0;previous=None;interval_sum=0;minimum=None;maximum=None
    running=True
    def publish(stage,phase):
        value={"running":running,"port":a.port,"slotSeconds":a.slot,"commandSeconds":2*a.slot,
            "cycle":cycle,"stage":stage,"phase":phase,"packets":packets,"commands":commands,"nextSeq":seq,
            "updatedAt":datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "intervalMin":minimum,"intervalMax":maximum,"intervalMean":interval_sum/(packets-1) if packets>1 else None}
        temp=status.with_suffix(".tmp");temp.write_text(json.dumps(value,indent=2)+"\n",encoding="utf-8")
        try:temp.replace(status)
        except PermissionError:pass
    def send(command,stage):
        nonlocal seq,packets,commands,previous,interval_sum,minimum,maximum
        op,data=payload(command)
        for phase in (0,1):
            if stop.exists():return False
            control=op+(phase<<3)+(seq<<4)
            encoded=packet([(f"D{i:02d}",v) for i,v in enumerate(data)]+[("Ctl",control)])
            sender.sendto(encoded,("127.0.0.1",a.port));now=time.monotonic()
            if previous is not None:
                delta=now-previous;interval_sum+=delta
                minimum=delta if minimum is None else min(minimum,delta);maximum=delta if maximum is None else max(maximum,delta)
            previous=now;packets+=1;publish(stage,"prepare" if phase==0 else "execute")
            deadline=now+a.slot
            while time.monotonic()<deadline:
                if stop.exists():return False
                time.sleep(min(.025,max(0,deadline-time.monotonic())))
        seq=(seq+1)%16;commands+=1
        return True
    with writer_lock(),socket.socket(socket.AF_INET,socket.SOCK_DGRAM) as sender:
        try:
            for _ in range(2):
                if not send({"op":"clear"},"initial-clear"):return
            while not stop.exists():
                cycle+=1;print(f"Cycle {cycle}: build -> recolor -> erase; slot={a.slot}s",flush=True)
                boxes=[{"min":[128+i*2,128,128],"size":[2,i+1,2],"styleId":(i+cycle)%4+1} for i in range(8)]
                for i,b in enumerate(boxes):
                    if not send(dict(op="box",**b),f"build {i+1}/8"):return
                for i,b in enumerate(boxes):
                    if not send(dict(b,op="box",styleId=(b["styleId"]%4)+1),f"recolor {i+1}/8"):return
                for i,b in enumerate(reversed(boxes)):
                    if not send({"op":"remove","min":b["min"],"size":b["size"]},f"erase {i+1}/8"):return
        finally:
            running=False;publish("stopped","stopped");print("Stopped OSC loop.",flush=True)


if __name__=="__main__":main()
