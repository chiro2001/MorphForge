#!/usr/bin/env python3
"""Run actual MF3 16k UDP delivery at .25s on Windows; progress is durable."""
import json,time,socket
from pathlib import Path
from send_mf3 import packet
from send_stage1 import writer_lock

def main():
    folder=Path('Docs/Stage4');folder.mkdir(parents=True,exist_ok=True)
    baseline=json.loads((folder/'capacity-baseline.json').read_text());assert baseline['count']==0 and not baseline['busy'] and not baseline['inbox'];
    report={'baseline':baseline,'expectedCompleted':baseline['completed']+16386,'scope':'Actual Windows UDP9012 at .25s; no particle prefill','total':16384,'sent':0,'started':time.time(),'finished':False}
    path=folder/'capacity-progress.json'
    def save():
        report['updated']=time.time();path.write_text(json.dumps(report,indent=2))
    seq=9
    with writer_lock(),socket.socket(socket.AF_INET,socket.SOCK_DGRAM)as sock:
        def send(c):
            nonlocal seq
            started=time.monotonic();sock.sendto(packet(c,seq),('127.0.0.1',9012));seq=(seq+1)%16;time.sleep(max(0,.25-(time.monotonic()-started)))
        send({'op':'clear'});send({'op':'brush','styleId':1})
        save()
        for i in range(16384):
            send({'op':'voxel','min':[80+i%32*3,i//1024*3,80+i//32%32*3]})
            report['sent']=i+1
            if (i+1)%128==0:save()
        report['finished']=True;report['elapsedSeconds']=time.time()-report['started'];report['nextSeq']=seq;save()
    print(json.dumps(report),flush=True)
if __name__=='__main__':main()
