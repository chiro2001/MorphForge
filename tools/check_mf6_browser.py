#!/usr/bin/env python3
"""Real mesh editor acceptance. Use only an isolated test workspace."""
import argparse,base64,json,time,urllib.request
from pathlib import Path
from check_editor_browser import Browser
def main():
 p=argparse.ArgumentParser();p.add_argument('--url',required=True);p.add_argument('--out',required=True,type=Path);p.add_argument('--webdriver',default='http://127.0.0.1:9518');a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True);checks=[];report={};b=Browser(a.webdriver)
 def api(path,body=None):
  with urllib.request.urlopen(urllib.request.Request(a.url+'/api/'+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'}),timeout=40) as r:return json.load(r)
 def check(name,condition):
  checks.append({'name':name,'passed':bool(condition)});print(('PASS ' if condition else 'FAIL ')+name,flush=True)
  assert condition,name
 try:
  b.call(b.path+'/url',{'url':a.url});b.wait('return window.__morphforge?.ready',30)
  check('test starts with OSC disabled',not api('state')['connection']['enabled'])
  b.js('document.getElementById("meshPanel").open=true;document.getElementById("meshDoubleSided").checked=true;');b.click('meshAdd');b.wait('return window.__morphforge.state.stats.triangleCount===1',20)
  face=api('scene')['scene']['triangles'][0];face_id=face['id'];check('real UI creates a per-face RGB triangle',face['vertices']==[[-8,0,0],[8,0,0],[0,16,0]] and face['color']==[231,163,75])
  check('triangle has a real GPU mesh',b.js('return window.__morphforge.renderer.triangleMeshes[0].geometry.attributes.position.count')==3)
  b.js('window.__morphforge.renderer.view("front")');time.sleep(.2);b.click('selectTool');b.point([0,4/3,0]);b.wait('return window.__morphforge.renderer.selection.size===1',15);check('real pointer selects triangle without a Box',True)
  b.click('meshLoadSelection');b.js('document.getElementById("meshC").value="0, 20, 0";');b.click('meshUpdate');b.wait('return window.__morphforge.renderer.triangles[0]?.vertices[2][1]===20',15)
  check('vertex update preserves object ID and count',api('scene')['scene']['triangles'][0]['id']==face_id and len(api('scene')['scene']['triangles'])==1)
  b.js('document.getElementById("coordX").value=3;document.getElementById("coordY").value=2;document.getElementById("coordZ").value=-1;');b.click('moveSelection');b.wait('return window.__morphforge.renderer.triangles[0]?.vertices[0][0]===-5',15)
  check('selected face translates with its stable ID',api('scene')['scene']['triangles'][0]['vertices'][0]==[-5,2,-1]);b.click('undo');b.wait('return window.__morphforge.renderer.triangles[0]?.vertices[0][0]===-8',15);check('undo restores triangle vertices',True)
  b.click('meshWedge');b.wait('return window.__morphforge.state.stats.triangleCount===9',20);check('slope tool creates closed eight-face prism',len(api('scene')['scene']['triangles'])==9)
  b.js('document.getElementById("modelingStyle").value="lowpoly";document.getElementById("modelingStyle").dispatchEvent(new Event("change"));');b.wait('return window.__morphforge.state.project.modelingStyle==="lowpoly"',15);check('modeling style is persisted for the harness',api('state')['project']['modelingStyle']=='lowpoly')
  b.js('window.__morphforge.renderer.view("hero")');b.screenshot(a.out/'mesh-editor.png')
  images=b.js('return window.__morphforge.renderer.capture(["front","hero","top"],384,320)')
  for image in images:(a.out/('mesh-'+image['view']+'.png')).write_bytes(base64.b64decode(image['data'].split(',',1)[1]))
  check('free-view screenshot pipeline includes polygon geometry',len(images)==3 and all(i['rendering']['supersampling']>=1 for i in images))
  value=api('export');check('export preserves colored mesh schema',value['project']['scene']['format']=='MorphForgeScene/2' and len(value['project']['scene']['triangles'])==9)
  logs=b.call(b.path+'/log',{'type':'browser'});check('no JavaScript or shader errors',not [r for r in logs if r['level']=='SEVERE' and 'favicon.ico' not in r['message']]);check('no OSC or model requests were started',not api('state')['connection']['enabled'])
  report={'success':True,'checks':checks,'scope':'Real Chromium UI, pointer selection, native WebGL meshes, screenshot camera pipeline and isolated HTTP backend. No paid model calls or real VRChat.'}
 except Exception as cause:
  b.screenshot(a.out/'failure.png');report={'success':False,'checks':checks,'error':str(cause),'logs':b.call(b.path+'/log',{'type':'browser'})};raise
 finally:
  (a.out/'validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2));b.close()
if __name__=='__main__':main()
