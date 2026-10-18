#!/usr/bin/env python3
"""Send a slow Stage1 demonstration to an already running VRChat client."""
import argparse
import datetime
import json
from pathlib import Path
import time

from compile_stage1 import compile_commands
from send_stage1 import transmit


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host",default="127.0.0.1")
    parser.add_argument("--port",type=int,default=9000)
    parser.add_argument("--prepare",type=float,default=.25)
    parser.add_argument("--execute",type=float,default=.6)
    parser.add_argument("--hold",type=float,default=5)
    args=parser.parse_args()
    if args.prepare<=0 or args.execute<=0 or args.hold<0:parser.error("Invalid hold time")
    seq=0
    report={"startedAt":datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "host":args.host,"port":args.port,"prepareSeconds":args.prepare,"executeSeconds":args.execute,
        "transmissionCompleted":False,"clientObserved":None,"remoteObserved":None,"stages":[]}
    path=Path("Docs/Stage1/vrchat-live-transmission.json");path.parent.mkdir(parents=True,exist_ok=True)
    def send(commands,label,hold=None):
        nonlocal seq
        print(label,flush=True)
        sent=transmit(commands,args.host,args.port,seq,args.prepare,args.execute)
        seq=sent["nextSeq"]
        report["stages"].append({"label":label,"sentAt":datetime.datetime.now(datetime.timezone.utc).isoformat(),"sender":sent})
        path.write_text(json.dumps(report,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
        time.sleep(args.hold if hold is None else hold)
    try:
        send([{"op":"clear"},{"op":"clear"}],"0. Clear previous display",2)
        wall={"op":"box","min":[128,128,128],"size":[20,10,1],"styleId":2}
        edits=[{"op":"clear"},wall]
        send([wall],"1. Brown wall (20 x 10 x 1)")
        before=compile_commands(edits)
        edits.append({"op":"erase","min":[136,131,128],"size":[4,4,1]})
        after=compile_commands(edits)
        send(after["commands"][len(before["commands"]):],"2. Cut a 4 x 4 window in the wall")
        blue={"op":"box","min":[151,128,128],"size":[3,6,3],"styleId":3}
        send([blue],"3. Add a blue cuboid beside the wall")
        send([dict(blue,styleId=4)],"4. Change blue cuboid to green")
        send([{"op":"remove","min":blue["min"],"size":blue["size"]}],"5. Remove the green cuboid; wall remains")
        send([{"op":"clear"}],"6. Clear everything",3)
        for i in range(8):
            send([{"op":"box","min":[128+i*2,128,128],"size":[2,i+1,2],"styleId":i%4+1}],f"7.{i+1}. Grow staircase",.5)
        report["transmissionCompleted"]=True
        print("DONE: 8 staircase cuboids should remain. Sender stopped; visual result awaits observer confirmation.",flush=True)
    finally:
        report["nextSeq"]=seq
        path.write_text(json.dumps(report,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")


if __name__=="__main__":main()
