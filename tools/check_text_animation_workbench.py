#!/usr/bin/env python3
"""Isolated workbench regression: camera, compact streams and editing controls."""
import argparse,json,subprocess,socket,time,urllib.request
from pathlib import Path
from check_editor_browser import Browser

def main():
 p=argparse.ArgumentParser();p.add_argument('--url',required=True);p.add_argument('--out',type=Path,required=True);a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True)
 sock=socket.socket();sock.bind(('127.0.0.1',0));port=sock.getsockname()[1];sock.close()
 driver=subprocess.Popen(['chromedriver',f'--port={port}','--allowed-ips=127.0.0.1'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);b=None;checks=[]
 def check(name,value):
  checks.append({'name':name,'passed':bool(value)});print(('PASS 'if value else 'FAIL ')+name,flush=True)
  if not value:raise AssertionError(name)
 def api(path,body=None):
  req=urllib.request.Request(a.url+'/api/'+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'})
  with urllib.request.urlopen(req,timeout=30)as f:return json.load(f)
 try:
  for i in range(50):
   try:b=Browser(f'http://127.0.0.1:{port}');break
   except(OSError,RuntimeError):time.sleep(.1)
  b.call(b.path+'/url',{'url':a.url});b.wait('return window.__morphforge?.ready',30)
  check('isolated workspace, OSC disabled',not api('state')['connection']['enabled'])
  check('manual tools and drawing start closed',b.js('return document.body.classList.contains("hide-tools")&&document.getElementById("manualEditToggle").getAttribute("aria-pressed")==="false"'))
  b.click('manualEditToggle');check('manual drawing toggle works',b.js('return document.getElementById("manualEditToggle").getAttribute("aria-pressed")==="true"'));b.click('manualEditToggle')
  api('edit',{'op':'box','min':[-8,0,-4],'size':[16,20,8],'styleId':1});b.wait('return window.__morphforge.renderer.boxes.length===1')
  result=b.js('''return (async()=>{const r=window.__morphforge.renderer;r.renderer.setAnimationLoop(null);const render=r.render.bind(r);r.render=()=>{};const rows=[];for(const view of ['hero','front','top','side']){r.view(view);const first=r.cameraBookmark();for(let i=0;i<20;i++)r.view(view);const last=r.cameraBookmark();rows.push({view,error:Math.max(...['position','target'].flatMap(k=>first[k].map((x,i)=>Math.abs(x-last[k][i]))))});}r.controls._panOffset.set(10,4,3);r.controls._scale=3;r.view('hero');const a=r.cameraBookmark();r.view('hero');const b=r.cameraBookmark();rows.push({view:'pending pan/zoom reset',error:Math.max(...a.position.map((x,i)=>Math.abs(x-b.position[i])))});r.render=render;const before=r.cameraBookmark();await r.capture(['front','top'],256,256);const after=r.cameraBookmark();rows.push({view:'capture restore',error:Math.max(...['position','target'].flatMap(k=>before[k].map((x,i)=>Math.abs(x-after[k][i]))))});r.animationLoop(100);return rows})()''')
  for row in result:check('camera '+row['view'],row['error']<1e-6)
  check('one input options fold contains type, references and queue',b.js('return ["modelingStyle","referenceComposer","pendingMessages"].every(id=>document.getElementById("composerOptions").contains(document.getElementById(id)))'))
  b.click('settingsOpen');b.wait('return !document.getElementById("settingsPage").hidden')
  check('send mode is in settings, strict prefix default enabled',b.js('return document.getElementById("settingsPage").contains(document.getElementById("messageMode"))&&document.getElementById("strictAppendOnly").checked'))
  b.click('strictAppendOnly');b.wait('return window.__morphforge.state.harness.settings.strictAppendOnly===false');check('strict prefix setting persisted',api('state')['harness']['settings']['strictAppendOnly']is False)
  b.click('strictAppendOnly');b.wait('return window.__morphforge.state.harness.settings.strictAppendOnly===true');b.click('settingsBack')
  b.js('''return (async()=>{const {StreamText}=await import('/stream-text.js');const host=document.createElement('div');host.id='streamProbe';host.style.cssText='position:fixed;left:10px;top:80px;width:220px;z-index:999;background:#222';document.body.append(host);window.streamProbe=new StreamText(host);streamProbe.setText('中 English 日本語 '.repeat(100));})()''');time.sleep(1)
  check('single-line stream follows right edge',b.js('return streamProbe.content.scrollWidth>streamProbe.content.clientWidth&&streamProbe.atEnd()'))
  b.js('streamProbe.content.dispatchEvent(new WheelEvent("wheel",{deltaY:-20}));streamProbe.content.scrollLeft=0;streamProbe.setText(streamProbe.text+" next ".repeat(100))');time.sleep(.4)
  check('manual scroll detaches follow',b.js('return !streamProbe.following&&streamProbe.content.scrollLeft===0'))
  b.js('streamProbe.latest.click();streamProbe.button.click();streamProbe.setText("line\\n".repeat(100))');time.sleep(1)
  check('expanded stream has internal max-height and follows bottom',b.js('return streamProbe.expanded&&streamProbe.content.clientHeight<=260&&streamProbe.content.scrollHeight>streamProbe.content.clientHeight&&streamProbe.atEnd()'))
  b.js('document.getElementById("streamProbe").remove()')
  check('status and materials overlays exist',b.js('return !!document.getElementById("viewportStatus")&&document.getElementById("usedMaterialOverlay").textContent.includes("1")'))
  b.screenshot(a.out/'workbench.png')
  errors=[r for r in b.call(b.path+'/log',{'type':'browser'})if r['level']=='SEVERE'and'favicon.ico'not in r['message']]
  check('no JavaScript or WebGL errors',not errors)
 finally:
  (a.out/'validation.json').write_text(json.dumps({'success':bool(checks)and all(x['passed']for x in checks),'checks':checks},ensure_ascii=False,indent=2))
  if b:b.close()
  driver.terminate()
if __name__=='__main__':main()
