#!/usr/bin/env python3
"""Local browser editor and exclusive, paced MF2 OSC writer. Python standard library only."""
import argparse
from contextlib import ExitStack
import copy
from collections import deque
import json
import math
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
from pathlib import Path
import secrets
import socket
import struct
import threading
import time
from urllib.parse import urlsplit

from editor_model import BoxDocument,CAPACITY,full_key,geometry_key,next_command,volume,wire_box
from send_stage1 import osc_string,writer_lock
from send_stage2 import packet

ROOT=Path(__file__).resolve().parents[1]


def placement_packet(values):
    messages=[]
    for name,value in values.items():
        if name not in ("MF_World","MF_Handle","MF_Scale","MF_Reset","MF_Grab"):raise ValueError("Unknown placement parameter")
        if type(value) is bool:kind=b",T\0\0" if value else b",F\0\0";data=b""
        elif type(value) is int:kind=b",i\0\0";data=struct.pack(">i",value)
        elif type(value) is float and math.isfinite(value):kind=b",f\0\0";data=struct.pack(">f",value)
        else:raise ValueError("Invalid placement parameter value")
        messages.append(osc_string("/avatar/parameters/"+name)+kind+data)
    return b"#bundle\0"+struct.pack(">Q",1)+b"".join(struct.pack(">i",len(m))+m for m in messages)


class Channel:
    def __enter__(self):
        self.stack=ExitStack()
        try:
            self.stack.enter_context(writer_lock());self.sock=self.stack.enter_context(socket.socket(socket.AF_INET,socket.SOCK_DGRAM));return self
        except Exception:self.stack.close();raise
    def send(self,data,port):self.sock.sendto(data,("127.0.0.1",port))
    def __exit__(self,*args):return self.stack.__exit__(*args)


class EditorEngine:
    def __init__(self,state_path=None,channel_factory=Channel):
        self.lock=threading.RLock();self.wake=threading.Condition(self.lock);self.stop_event=threading.Event();self.channel_factory=channel_factory
        self.path=Path(state_path) if state_path else None;self.document=BoxDocument();self.instance=secrets.token_hex(4)
        self.port=9000;self.profile="anchor";self.interval=.25;self.auto=True;self.enabled=False;self.connected=False;self.generation=0
        self.placement={"world":True,"visible":True,"scale":1.0};self.pending_controls={};self.reset_pulses=0;self.resets=0
        self.desired=[];self.desired_version=0;self.sent=[];self.sent_version=-1;self.seq=secrets.randbelow(32);self.last_error="";self.sent_count=0
        self._load();self.thread=threading.Thread(target=self._worker,name="MorphForge OSC writer",daemon=True);self.thread.start()
    def _load(self):
        if not self.path or not self.path.exists():return
        try:
            data=json.loads(self.path.read_text(encoding="utf-8"));doc=data.get("document",data)
            self.document=BoxDocument(doc.get("size",16),doc.get("boxes",[]));self.profile=data.get("profile","anchor")
            if self.profile not in ("anchor","stage2"):self.profile="anchor"
            if data.get("port") in (9000,9012):self.port=data["port"]
            if .05<=data.get("interval",.25)<=2:self.interval=data["interval"]
            placement=data.get("placement",{})
            for key in ("world","visible"):
                if type(placement.get(key)) is bool:self.placement[key]=placement[key]
            scale=placement.get("scale",1.0)
            if type(scale) in (int,float) and math.isfinite(scale) and .05<=scale<=1:self.placement["scale"]=float(scale)
            self.desired=copy.deepcopy(self.document.boxes)
        except (ValueError,TypeError,KeyError) as e:self.last_error="读取上次草稿失败："+str(e)
    def _save(self):
        if not self.path:return
        value={"document":self.document.export(),"profile":self.profile,"port":self.port,"interval":self.interval,"placement":self.placement}
        self.path.parent.mkdir(parents=True,exist_ok=True);temp=self.path.with_suffix(".tmp")
        temp.write_text(json.dumps(value,ensure_ascii=False,indent=2)+"\n",encoding="utf-8");temp.replace(self.path)
    def _publish(self):
        for b in self.document.boxes:wire_box(b,self.profile)
        self.desired=copy.deepcopy(self.document.boxes);self.desired_version=self.document.version;self.wake.notify_all()
    def state(self):
        with self.lock:
            old={geometry_key(b):b for b in self.sent};new={geometry_key(b):b for b in self.desired}
            pending=len(set(old)-set(new))+sum(k not in old or full_key(old[k])!=full_key(b) for k,b in new.items())
            return {"instance":self.instance,"document":self.document.export(),"version":self.document.version,
                "voxelCount":sum(volume(b) for b in self.document.boxes),"boxCount":len(self.document.boxes),"capacity":CAPACITY,
                "canUndo":bool(self.document.undo_stack),"canRedo":bool(self.document.redo_stack),
                "connection":{"enabled":self.enabled,"sending":self.connected,"port":self.port,"profile":self.profile,"interval":self.interval,"auto":self.auto,
                    "pending":pending+self.resets+bool(self.pending_controls)+self.reset_pulses,"sentVersion":self.sent_version,"desiredVersion":self.desired_version,
                    "sentCommands":self.sent_count,"error":self.last_error},"placement":copy.deepcopy(self.placement)}
    def edit(self,value):
        with self.wake:
            # Validate legacy Y range before committing a live document.
            prior=copy.deepcopy(self.document.__dict__)
            try:
                changed=self.document.apply(value)
                if self.enabled and self.auto:self._publish()
            except Exception:self.document.__dict__=prior;raise
            if value.get("op")=="clear" and self.enabled:
                self._publish();self.resets=max(self.resets,1);self.wake.notify_all()
            self._save();return self.state()
    def configure(self,value):
        with self.wake:
            enabled=value.get("enabled",self.enabled);port=value.get("port",self.port);profile=value.get("profile",self.profile);interval=value.get("interval",self.interval)
            if type(enabled) is not bool or type(port) is not int or port not in (9000,9012):raise ValueError("仅支持本机 VRChat 9000 或 Unity 9012")
            if profile not in ("anchor","stage2"):raise ValueError("未知 Avatar 版本")
            if type(interval) not in (int,float) or not .05<=interval<=2:raise ValueError("发送间隔须在 0.05–2 秒之间")
            for b in self.document.boxes:wire_box(b,profile)
            auto=value.get("auto",self.auto)
            if type(auto) is not bool:raise ValueError("实时发送设置无效")
            restart=enabled and (not self.enabled or port!=self.port or profile!=self.profile)
            was_auto=self.auto
            self.port=port;self.profile=profile;self.interval=float(interval);self.auto=auto
            if restart:
                self.generation+=1;self.sent=[];self.sent_version=-1;self.resets=2;self.last_error=""
                self.pending_controls={};self.reset_pulses=0
                if profile=="anchor":self.pending_controls={"MF_World":self.placement["world"],"MF_Handle":self.placement["visible"],"MF_Scale":float(self.placement["scale"])}
                self._publish()
            elif enabled and auto and not was_auto:self._publish()
            self.enabled=enabled
            if not enabled:self.generation+=1;self.connected=False
            self._save();self.wake.notify_all();return self.state()
    def publish(self,resync=False):
        with self.wake:
            self._publish()
            if resync:self.resets=max(self.resets,2)
            self.wake.notify_all();return self.state()
    def place(self,value):
        with self.wake:
            if self.profile!="anchor":raise ValueError("定位球控制需要新版 Avatar")
            mapping={"world":"MF_World","visible":"MF_Handle","scale":"MF_Scale"}
            for key in value:
                if key not in (*mapping,"reset"):raise ValueError("未知定位设置")
            clean={}
            for key,parameter in mapping.items():
                if key not in value:continue
                v=value[key]
                if key=="scale":
                    if type(v) not in (int,float) or not math.isfinite(v) or not .05<=v<=1:raise ValueError("缩放范围为 0.05–1.0")
                    v=float(v)
                elif type(v) is not bool:raise ValueError("开关须为布尔值")
                clean[key]=v
            if "reset" in value and type(value["reset"]) is not bool:raise ValueError("重置值无效")
            self.placement.update(clean)
            for key,v in clean.items():self.pending_controls[mapping[key]]=v
            if value.get("reset"):self.reset_pulses=2
            self._save();self.wake.notify_all();return self.state()
    def _job(self):
        if self.resets:return "clear",{"op":"clear"}
        if self.reset_pulses:return "pulse",{"MF_Reset":self.reset_pulses==2,**({"MF_Grab":0} if self.reset_pulses==2 else {})}
        if self.pending_controls:return "controls",copy.deepcopy(self.pending_controls)
        if not self.desired and self.sent:return "clear",{"op":"clear"}
        command=next_command(self.sent,self.desired)
        if command:return "geometry",command
        self.sent_version=self.desired_version;return None
    def _worker(self):
        context=None;channel=None;active_generation=-1;next_send=0
        try:
            while not self.stop_event.is_set():
                with self.wake:
                    enabled=self.enabled;generation=self.generation
                if context and (not enabled or generation!=active_generation):
                    context.__exit__(None,None,None);context=None;channel=None
                    with self.wake:self.connected=False
                if not enabled:
                    with self.wake:self.wake.wait(.2)
                    continue
                if channel is None:
                    try:
                        context=self.channel_factory();channel=context.__enter__();active_generation=generation
                        with self.wake:self.connected=True;self.last_error=""
                    except Exception as e:
                        if context:
                            try:context.__exit__(None,None,None)
                            except Exception:pass
                        context=None;channel=None
                        with self.wake:self.enabled=False;self.connected=False;self.last_error="OSC 通道不可用："+str(e)
                        continue
                with self.wake:
                    job=self._job();port=self.port;profile=self.profile;generation=self.generation;seq=self.seq;interval=self.interval
                if not job:
                    with self.wake:self.wake.wait(.1)
                    continue
                if self.stop_event.wait(max(0,next_send-time.monotonic())):break
                with self.wake:
                    if not self.enabled or generation!=self.generation:continue
                kind,value=job
                try:
                    encoded=placement_packet(value) if kind in ("controls","pulse") else packet(dict(wire_box(value,profile),op=value["op"]) if value["op"]!="clear" else value,seq)
                    channel.send(encoded,port)
                except (OSError,ValueError,TypeError,OverflowError) as e:
                    with self.wake:self.enabled=False;self.connected=False;self.last_error="发送失败："+str(e)
                    continue
                next_send=time.monotonic()+interval
                with self.wake:
                    if generation!=self.generation:continue
                    self.sent_count+=1
                    if kind not in ("controls","pulse"):self.seq=(self.seq+1)%32
                    if kind=="clear":self.sent=[];self.resets=max(0,self.resets-1)
                    elif kind=="pulse":self.reset_pulses=max(0,self.reset_pulses-1)
                    elif kind=="controls":
                        for k,v in value.items():
                            if self.pending_controls.get(k)==v:self.pending_controls.pop(k,None)
                    elif kind=="geometry":
                        self.sent=[b for b in self.sent if geometry_key(b)!=geometry_key(value)]
                        if value["op"]=="box":self.sent.append({k:copy.deepcopy(value[k]) for k in ("min","size","styleId","tintId","motionPhase")})
        finally:
            if context:context.__exit__(None,None,None)
            with self.wake:self.connected=False
    def close(self):
        self.stop_event.set()
        with self.wake:self.wake.notify_all()
        self.thread.join(3)


class Handler(SimpleHTTPRequestHandler):
    extensions_map={**SimpleHTTPRequestHandler.extensions_map,".js":"text/javascript",".css":"text/css"}
    def __init__(self,*args,**kwargs):super().__init__(*args,directory=str(ROOT/"web"),**kwargs)
    def log_message(self,format,*args):
        if args and str(args[0]).startswith("GET"):return
        super().log_message(format,*args)
    def _host_ok(self):
        host=urlsplit("//"+self.headers.get("Host","")).hostname
        return host in self.server.allowed_hosts
    def _json(self,value,status=200):
        body=json.dumps(value,ensure_ascii=False,allow_nan=False).encode();self.send_response(status);self.send_header("Content-Type","application/json; charset=utf-8");self.send_header("Cache-Control","no-store");self.send_header("X-Content-Type-Options","nosniff");self.send_header("Content-Length",str(len(body)));self.end_headers();self.wfile.write(body)
    def do_GET(self):
        if not self._host_ok():return self._json({"error":"Host not allowed"},403)
        path=urlsplit(self.path).path
        if path=="/api/state":return self._json(self.server.engine.state())
        if path=="/api/export":return self._json(self.server.engine.state()["document"])
        if path.startswith("/api/"):return self._json({"error":"Not found"},404)
        return super().do_GET()
    def do_POST(self):
        if not self._host_ok():return self._json({"error":"Host not allowed"},403)
        origin=self.headers.get("Origin")
        if origin and urlsplit(origin).netloc!=self.headers.get("Host"):return self._json({"error":"Origin not allowed"},403)
        if self.headers.get_content_type()!="application/json":return self._json({"error":"需要 JSON 请求"},415)
        try:
            length=int(self.headers.get("Content-Length","0"))
            if not 0<length<=1024*1024:raise ValueError("请求数据过大或为空")
            value=json.loads(self.rfile.read(length))
            if not isinstance(value,dict):raise ValueError("请求须为对象")
            engine=self.server.engine;path=urlsplit(self.path).path
            if path=="/api/edit":result=engine.edit(value)
            elif path=="/api/connect":result=engine.configure(value)
            elif path=="/api/publish":result=engine.publish(bool(value.get("resync",False)))
            elif path=="/api/placement":result=engine.place(value)
            else:return self._json({"error":"Not found"},404)
            self._json(result)
        except (ValueError,TypeError,KeyError,OverflowError) as e:self._json({"error":str(e)},400)
        except OSError as e:self._json({"error":"保存失败："+str(e)},500)


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument("--listen",default="127.0.0.1");parser.add_argument("--port",type=int,default=8765)
    parser.add_argument("--state",type=Path,default=ROOT/"Docs/Editor/document.json");args=parser.parse_args()
    engine=EditorEngine(args.state)
    try:server=ThreadingHTTPServer((args.listen,args.port),Handler)
    except Exception:engine.close();raise
    server.engine=engine;server.allowed_hosts={"127.0.0.1","localhost",args.listen};server.daemon_threads=True
    directory=Path.home()/"tmp"/"morphforge";directory.mkdir(parents=True,exist_ok=True);stop=directory/"editor.stop";stop.unlink(missing_ok=True)
    def monitor():
        while not engine.stop_event.wait(.25):
            if stop.exists():server.shutdown();return
    threading.Thread(target=monitor,daemon=True).start()
    print(f"MorphForge editor ready: http://{args.listen}:{args.port} (OSC disabled until enabled in the page)",flush=True)
    try:server.serve_forever(.25)
    except KeyboardInterrupt:pass
    finally:server.server_close();engine.close()


if __name__=="__main__":main()
