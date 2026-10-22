#!/usr/bin/env python3
"""Text-only animation: real viewer pose/restore and animated GLB group export."""
import argparse,json,socket,subprocess,time,urllib.request,struct
from pathlib import Path
from check_editor_browser import Browser

def main():
 p=argparse.ArgumentParser();p.add_argument('--url',required=True);p.add_argument('--out',type=Path,required=True);a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True);checks=[];b=None
 def api(path,body=None,raw=False):
  req=urllib.request.Request(a.url+'/api/'+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'})
  with urllib.request.urlopen(req,timeout=30)as r:return r.read()if raw else json.load(r)
 def check(name,value):
  checks.append({'name':name,'passed':bool(value)});print(('PASS 'if value else 'FAIL ')+name,flush=True)
  if not value:raise AssertionError(name)
 sock=socket.socket();sock.bind(('127.0.0.1',0));port=sock.getsockname()[1];sock.close();driver=subprocess.Popen(['chromedriver',f'--port={port}','--allowed-ips=127.0.0.1'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
 try:
  check('isolated project with OSC disabled',not api('state')['connection']['enabled']);api('edit',{'op':'clear'});rev=api('state')['revision']
  api('text-panels',{'action':'create','id':'moving-text','baseRevision':rev,'panel':{'text':'风车 Wind','position':[2,34,3],'rotation':[10,20,30],'size':[32,12],'fontSize':4,'group':'caption'}})
  clip={'name':'文本框运动','duration':4,'looping':False,'tracks':[{'group':'caption','pivot':[0,28,0],'interpolation':'linear','keys':[{'time':0,'translation':[0,0,0],'rotation':[0,0,0],'scale':[1,1,1]},{'time':4,'translation':[8,2,0],'rotation':[0,90,0],'scale':[1,1,1]}]}]}
  rev=api('creative')['revision'];api('creative/animation',{'action':'save','id':'text-motion','baseCreativeRevision':rev,'clip':clip});before=api('scene')['scene']
  check('text-only groups are authorable animation targets',api('state')['stats']['groups'].get('caption')==1)
  for _ in range(50):
   try:b=Browser(f'http://127.0.0.1:{port}');break
   except (OSError,RuntimeError):time.sleep(.1)
  b.call(b.path+'/url',{'url':a.url});b.wait('return window.__morphforge?.ready&&window.__morphforge.renderer.textPanels.items.size===1',30)
  result=b.js('''const r=window.__morphforge.renderer,clip=arguments[0],root=r.textPanels.items.get('moving-text');root.updateMatrix();const original=root.matrix.elements.slice();r.setAnimationPose(clip,4);const moved=root.matrix.elements.slice();const bounds=r.cameraObjects();for(let i=0;i<25;i++)r.setAnimationPose(clip,4);const repeated=root.matrix.elements.slice();r.clearAnimationPose();root.updateMatrix();const restored=root.matrix.elements.slice();return {original,moved,repeated,restored,bounds,auto:root.matrixAutoUpdate};''',clip)
  check('viewer moves and rotates the actual text root',all(abs(x-y)<1e-6 for x,y in zip(result['moved'][12:15],[2.75,9,0.5])))
  check('repeated posing never accumulates transforms',max(abs(x-y)for x,y in zip(result['moved'],result['repeated']))<1e-7)
  check('leaving animation restores text matrix and automatic updates',result['auto'] and max(abs(x-y)for x,y in zip(result['original'],result['restored']))<1e-7)
  check('animated camera bounds include text',len(result['bounds'])==1 and result['bounds'][0]['group']=='caption')
  check('preview never mutates persisted text',api('scene')['scene']==before)
  binary=api('creative/animation/text-motion/export',raw=True);length=struct.unpack_from('<I',binary,12)[0];doc=json.loads(binary[20:20+length]);nodes=doc['nodes'];motion=next((n for n in nodes if n.get('name')=='motion:caption'),None)
  check('animated GLB exports text geometry under its motion group',motion is not None and bool(motion.get('children'))and bool(doc.get('animations')))
  b.js('window.__morphforge.renderer.setAnimationPose(arguments[0],4);window.__morphforge.renderer.view("hero");',clip);time.sleep(.25);b.screenshot(a.out/'text-motion.png');logs=b.call(b.path+'/log',{'type':'browser'});check('no JavaScript or GPU errors',not[x for x in logs if x['level']=='SEVERE'and'favicon.ico'not in x['message']])
 finally:
  (a.out/'validation.json').write_text(json.dumps({'success':bool(checks)and all(c['passed']for c in checks),'checks':checks},ensure_ascii=False,indent=2))
  if b:b.close()
  driver.terminate()
if __name__=='__main__':main()
