#!/usr/bin/env python3
"""MF2 single-commit staircase loop for an already running client or Editor receiver."""
import argparse
import datetime
import json
from pathlib import Path
import socket
import time

from send_stage1 import writer_lock
from send_stage2 import packet


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--host",default="127.0.0.1");p.add_argument("--port",type=int,default=9000)
    p.add_argument("--interval",type=float,default=.25);p.add_argument("--seq",type=int,default=0)
    p.add_argument("--cycles",type=int,default=0,help="0 loops until the stop file appears")
    a=p.parse_args()
    if a.interval<=0 or not 0<=a.seq<32 or a.cycles<0:p.error("Invalid interval, sequence or cycle count")
    out=Path.home()/"tmp"/"morphforge";out.mkdir(parents=True,exist_ok=True)
    stop=out/"stage2-loop.stop";status=out/"stage2-loop-status.json"
    seq=a.seq;cycle=0;commands=0;previous=None;deltas=[];running=True
    def publish(stage):
        value={"running":running,"protocol":"MF2/single-commit","port":a.port,"interval":a.interval,
            "cycle":cycle,"stage":stage,"commands":commands,"nextSeq":seq,
            "updatedAt":datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "recentIntervalMin":min(deltas) if deltas else None,"recentIntervalMax":max(deltas) if deltas else None}
        temp=status.with_suffix(".tmp");temp.write_text(json.dumps(value,indent=2)+"\n",encoding="utf-8")
        try:temp.replace(status)
        except PermissionError:pass
    def send(command,stage):
        nonlocal seq,commands,previous
        if stop.exists():return False
        encoded=packet(command,seq);sender.sendto(encoded,(a.host,a.port));now=time.monotonic()
        if previous is not None:deltas.append(now-previous);del deltas[:-256]
        previous=now;seq=(seq+1)%32;commands+=1;publish(stage)
        deadline=now+a.interval
        while time.monotonic()<deadline:
            if stop.exists():return False
            time.sleep(min(.025,max(0,deadline-time.monotonic())))
        return True
    with writer_lock(),socket.socket(socket.AF_INET,socket.SOCK_DGRAM) as sender:
        stop.unlink(missing_ok=True)
        try:
            for _ in range(2):
                if not send({"op":"clear"},"initial-clear"):return
            while not stop.exists() and (a.cycles==0 or cycle<a.cycles):
                cycle+=1;print(f"MF2 cycle {cycle}: one command every {a.interval}s",flush=True)
                boxes=[{"min":[128+i*2,128,128],"size":[2,i+1,2],"styleId":(i+cycle)%4+1} for i in range(8)]
                for i,b in enumerate(boxes):
                    if not send(dict(op="box",**b),f"build {i+1}/8"):return
                for i,b in enumerate(boxes):
                    if not send(dict(b,op="box",styleId=b["styleId"]%4+1),f"recolor {i+1}/8"):return
                for i,b in enumerate(reversed(boxes)):
                    if not send({"op":"remove","min":b["min"],"size":b["size"]},f"erase {i+1}/8"):return
        finally:running=False;publish("stopped");print("Stopped MF2 loop.",flush=True)


if __name__=="__main__":main()
