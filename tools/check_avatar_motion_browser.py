#!/usr/bin/env python3
"""Real Chromium clicks for explicit Avatar animation controls; never enable OSC."""
import argparse,json,socket,subprocess,time,urllib.request
from pathlib import Path
from check_editor_browser import Browser

def main():
 p=argparse.ArgumentParser();p.add_argument('--url',required=True);p.add_argument('--out',type=Path,required=True);a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True)
 def api(path,body=None):
  req=urllib.request.Request(a.url+'/api/'+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'})
  with urllib.request.urlopen(req,timeout=30) as r:return json.load(r)
 checks=[];b=None
 def check(name,value):
  checks.append({'name':name,'passed':bool(value)});print(('PASS ' if value else 'FAIL ')+name,flush=True)
  if not value:raise AssertionError(name)
 s=socket.socket();s.bind(('127.0.0.1',0));port=s.getsockname()[1];s.close();driver=subprocess.Popen(['chromedriver',f'--port={port}','--allowed-ips=127.0.0.1'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
 try:
  check('isolated sender remains disabled',not api('state')['connection']['enabled'])
  api('harness/tool',{'name':'run_recipe','arguments':{'mode':'replace','code':"mf.box([-4,24,-4],[8,8,8],{style:1,group:'rotor'});"}})
  clip={'name':'测试旋转','duration':4,'looping':True,'tracks':[{'group':'rotor','pivot':[0,28,0],'interpolation':'linear','keys':[{'time':t,'translation':[0,0,0],'rotation':[0,y,0],'scale':[1,1,1]} for t,y in [(0,0),(2,180),(4,360)]]}]}
  rev=api('creative')['revision'];api('creative/animation',{'action':'save','id':'browser-spin','baseCreativeRevision':rev,'clip':clip})
  for _ in range(50):
   try:b=Browser(f'http://127.0.0.1:{port}');break
   except (OSError,RuntimeError):time.sleep(.1)
  b.call(b.path+'/url',{'url':a.url});b.wait('return window.__morphforge?.ready',30);b.click('animationOpen');b.wait('return !motionPanel.hidden')
  b.js('motionClip.value="browser-spin";motionClip.dispatchEvent(new Event("change"));');time.sleep(.2)
  b.js('motionAvatarCheck.closest("details").open=true;');b.click('motionAvatarCheck');b.wait('return motionStatus.textContent.includes("可发布")')
  check('check button validates saved clip and explains frame cost','定义4帧' in b.js('return motionStatus.textContent'))
  check('dry run does not bind or send motion',api('state')['connection']['motion']['clipId']=='')
  b.click('motionAvatarPublish');b.wait('return !motionAvatarPublish.disabled');time.sleep(.2)
  check('unsupported target reports clear error',b.js('return document.body.innerText.includes("AVATAR_MOTION_UNSUPPORTED")'))
  for button in ['motionAvatarResume','motionAvatarPause','motionAvatarStop','motionAvatarSeek']:
   b.click(button);b.wait('return !document.getElementById("'+button+'").disabled');time.sleep(.1)
   check(button+' handles missing publication visibly',b.js('return document.body.innerText.includes("尚未发布Avatar动画")'))
  b.click('motionAvatarDetach');b.wait('return motionStatus.textContent.includes("已解除Avatar动画绑定")');check('detach button explicitly queues a static target',api('state')['connection']['motion']['attached']is False)
  b.click('motionPlay');check('editor playback still works',b.js('return motionPlay.textContent.includes("播放中")'));b.click('motionPause');check('editor pause still works',b.js('return motionPlay.textContent==="播放"'))
  check('no OSC enabled by any control',not api('state')['connection']['enabled']);b.screenshot(a.out/'avatar-motion-controls.png')
  logs=b.call(b.path+'/log',{'type':'browser'});(a.out/'browser-logs.json').write_text(json.dumps(logs,ensure_ascii=False,indent=2));errors=[x for x in logs if x['level']=='SEVERE' and 'favicon.ico' not in x['message'] and '400 (Bad Request)' not in x['message'] and not (x['source']=='console-api' and any(v in x['message'] for v in ['AVATAR_MOTION_UNSUPPORTED','尚未发布Avatar动画']))]
  check('no unhandled JavaScript or WebGL errors',not errors)
 finally:
  (a.out/'validation.json').write_text(json.dumps({'success':bool(checks) and all(c['passed'] for c in checks),'checks':checks},ensure_ascii=False,indent=2))
  if b:b.close()
  driver.terminate()
if __name__=='__main__':main()
