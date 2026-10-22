#!/usr/bin/env python3
"""Regression evidence for preview concurrency, capsule picking and depth precision."""
import argparse,base64,json,socket,subprocess,time,urllib.request
from pathlib import Path
from check_editor_browser import Browser
p=argparse.ArgumentParser();p.add_argument('--url',required=True);p.add_argument('--out',type=Path,required=True);p.add_argument('--geometry',type=Path,required=True);a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True)
s=socket.socket();s.bind(('127.0.0.1',0));port=s.getsockname()[1];s.close();driver=subprocess.Popen(['chromedriver',f'--port={port}','--allowed-ips=127.0.0.1'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);b=None;checks=[]
def check(name,value):
 checks.append({'name':name,'passed':bool(value)});print(('PASS ' if value else 'FAIL ')+name,flush=True)
 if not value:raise AssertionError(name)
try:
 for _ in range(50):
  try:b=Browser(f'http://127.0.0.1:{port}');break
  except(OSError,RuntimeError):time.sleep(.1)
 b.call(b.path+'/url',{'url':a.url});b.wait('return window.__morphforge?.ready',30)
 check('OSC remains disabled',not b.js('return window.__morphforge.state.connection.enabled'))
 # Slow the shared font load to guarantee overlapping preview requests.
 race=b.js('''return(async()=>{const r=window.__morphforge.renderer,t=r.textPanels;
 const old=[...t.items.values()][0];if(!old)throw Error('Run text panel UI acceptance first');const panel=structuredClone(old.userData.textPanel);panel.id='race-proof';
 const response=await fetch('/api/scene'),data=await response.json(),layout=data.textLayouts[0].layout;
 const original=t.texture.bind(t);t.texture=async font=>{await new Promise(ok=>setTimeout(ok,80));return original(font)};
 try{await Promise.all([t.preview({...panel,name:'obsolete'},layout),t.preview({...panel,name:'current'},layout)]);
 const roots=t.root.children.filter(n=>n.userData.textPanel?.id===panel.id);return {count:roots.length,name:roots[0]?.name};}finally{t.texture=original;}})()''')
 check('overlapping text previews retain exactly the newest mesh',race=={'count':1,'name':'current'})
 picked=b.js('''return(async()=>{const r=window.__morphforge.renderer,{Vector3}=await import('three');
 await r.load({cellSize:.25,size:256,revision:99999,boxes:[],drawings:[{id:999,kind:'segment',start:[-60,50,0],end:[60,50,0],diameter64:128,color:[255,80,30],group:'pick'}]},[],{textLayouts:[]});
 r.camera.position.set(0,12.5,50);r.camera.lookAt(0,12.5,0);r.camera.updateMatrixWorld();r.render();
 return [-55,0,55].map(x=>{const p=r.projectPoint([x*.25,12.5,0]);return r.pick({clientX:p.x,clientY:p.y},{erase:true})?.drawing?.id});})()''')
 check('capsule CPU picking agrees at both far endpoints and center',picked==[999,999,999])
 private=json.loads(a.geometry.read_text());
 result=b.js('''return(async()=>{const data=arguments[0],r=window.__morphforge.renderer,{Vector3}=await import('three');
 r.renderer.setAnimationLoop(null);await r.load(data.scene,data.materials,{textLayouts:[]});r.setViewOptions({surfaceCulling:true,showTargetBounds:false});
 const shots=[];for(const [name,distance]of [['near',115],['far',400]])shots.push({id:name,position:[distance*.35,100,distance],target:[0,100,0],projection:'perspective',fov:45});
 const images=await r.capture(shots.map(s=>s.id),768,640,shots,{supersampling:1});
 r.camera.position.set(20,25,60);r.camera.lookAt(0,25,0);r.camera.updateMatrixWorld();r.render();const range={near:r.camera.near,far:r.camera.far};
 const captures=[];for(let i=0;i<4;i++){r.render();captures.push(r.renderer.domElement.toDataURL());}
 return{images,range,stable:captures.every(v=>v===captures[0]),boxCount:r.boxes.length};})()''',private)
 check('private 176-box scene renders with a stable stationary view',result['boxCount']==176 and result['stable'])
 check('preview uses tighter depth range',result['range']['near']>.001 and result['range']['far']/result['range']['near']<1000)
 for im in result['images']:(a.out/(im['view']+'.png')).write_bytes(base64.b64decode(im['data'].split(',',1)[1]))
 # Numeric and images are public evidence; original geometry and user text stay private.
 (a.out/'depth.json').write_text(json.dumps({k:v for k,v in result.items()if k!='images'},indent=2))
 logs=b.call(b.path+'/log',{'type':'browser'});check('no JavaScript or WebGL errors',not [v for v in logs if v['level']=='SEVERE'and'favicon.ico'not in v['message']])
finally:
 (a.out/'validation.json').write_text(json.dumps({'success':bool(checks)and all(c['passed']for c in checks),'checks':checks},ensure_ascii=False,indent=2))
 if b:b.close()
 driver.terminate()
