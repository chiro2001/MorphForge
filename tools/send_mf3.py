#!/usr/bin/env python3
"""Independent MF3 test sender. Production publishing is implemented in Rust."""
import struct,socket,time,argparse,json
from send_stage1 import osc_string,writer_lock

def packet(command,seq):
    if type(seq) is not int or not 0<=seq<16:raise ValueError('seq 0..15')
    op=command['op'];data=[0]*6
    if op=='clear':code=5
    elif op=='brush':
        style=command.get('styleId',1);tint=command.get('tintId',0);motion=command.get('motionPhase',0)
        if not 0<=style<=4095 or not 0<=tint<=63 or not 0<=motion<=63:raise ValueError('brush range')
        code=6;data=[style&63,style>>6,tint,motion,0,0]
    else:
        code={'box':1,'voxel':2,'erase':3,'remove':4}[op];minimum=command['min'];size=command.get('size',[1]*3)
        if len(minimum)!=3 or len(size)!=3 or any(type(v) is not int for v in minimum+size):raise ValueError('integer XYZ required')
        if any(not 0<=p<=255 or not 1<=s<=256 or p+s>256 for p,s in zip(minimum,size)):raise ValueError('wire cube bounds')
        data=minimum+[s-1 for s in size]
        if code==2:data[3:]=command.get("unitSourceSizes",[0,0,0])
    values=[(f'D{i:02d}',v)for i,v in enumerate(data)]+[('Ctl',(seq<<4)|code)]
    messages=[osc_string('/avatar/parameters/MF3_'+name)+osc_string(',i')+struct.pack('>i',v)for name,v in values]
    return b'#bundle\0'+struct.pack('>Q',1)+b''.join(struct.pack('>i',len(m))+m for m in messages)

def transmit(commands,port=9012,seq=1,interval=.25):
    with writer_lock(),socket.socket(socket.AF_INET,socket.SOCK_DGRAM)as sock:
        for c in commands:
            sock.sendto(packet(c,seq),('127.0.0.1',port));seq=(seq+1)%16;time.sleep(interval)
    return {'commands':len(commands),'nextSeq':seq,'interval':interval,'port':port}
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--input',required=True);p.add_argument('--port',type=int,default=9012);p.add_argument('--seq',type=int,default=1);p.add_argument('--interval',type=float,default=.25);a=p.parse_args();print(json.dumps(transmit(json.load(open(a.input)),a.port,a.seq,a.interval)))
