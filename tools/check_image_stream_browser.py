#!/usr/bin/env python3
"""Real UI acceptance for image pixels/voxel planes and offline loop settings."""
import argparse,json,struct,zlib,urllib.request,time
from pathlib import Path
from check_editor_browser import Browser

def png_fixture():
 def chunk(t,b):return struct.pack('>I',len(b))+t+b+struct.pack('>I',zlib.crc32(t+b)&0xffffffff)
 raw=bytes([0,255,0,0,255,0,255,0,255,0,0,255,0,0,0,0,255,255,255,0,0,255,1,2,3,255])
 return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',3,2,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(raw))+chunk(b'IEND',b'')
def main():
 p=argparse.ArgumentParser();p.add_argument('--url',required=True);p.add_argument('--out',required=True,type=Path);a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True);checks=[]
 def api(path,body=None):
  with urllib.request.urlopen(urllib.request.Request(a.url+'/api/'+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'}),timeout=45) as r:return json.load(r)
 def check(name,value):
  checks.append({'name':name,'passed':bool(value)});print(('PASS ' if value else 'FAIL ')+name,flush=True)
  assert value,name
 for i in range(300):
  try:api('state');break
  except OSError:time.sleep(.1)
 boundary='MorphForgeImageFixture';body=('--'+boundary+'\r\nContent-Disposition: form-data; name="file"; filename="fixture.png"\r\nContent-Type: image/png\r\n\r\n').encode()+png_fixture()+('\r\n--'+boundary+'--\r\n').encode()
 item=json.load(urllib.request.urlopen(urllib.request.Request(a.url+'/api/references/upload',data=body,headers={'Content-Type':'multipart/form-data; boundary='+boundary})))
 api('vrc/profile',{'action':'bundled_mf5'});api('connect',{'profile':'mf5','enabled':False})
 b=Browser()
 try:
  b.call(b.path+'/url',{'url':a.url});b.wait('return window.__morphforge?.ready',30)
  b.click('imageToolsOpen');b.wait('return document.getElementById("imageToolsDialog").open');b.js('document.getElementById("imageToolId").value=arguments[0];document.getElementById("imageToolId").dispatchEvent(new Event("change"));document.getElementById("imageToolWidth").value=3;document.getElementById("imageToolHeight").value=2;document.getElementById("imageToolAspect").checked=false;',item['id'])
  b.click('imageToolInspect');b.wait('return document.getElementById("imageToolReport").textContent.includes("rgba")',20);v=json.loads(b.js('return document.getElementById("imageToolReport").textContent'));check('real UI reads exact source RGBA pixel',v['points'][0]['rgba']==[255,0,0,255])
  before=api('state')['revision'];b.click('imageToolPreview');b.wait('return !document.getElementById("imageToolApply").disabled',20)
  check('preview leaves scene unchanged',api('state')['revision']==before and not api('scene')['scene']['boxes'])
  preview=json.loads(b.js('return document.getElementById("imageToolReport").textContent'));check('preview diagnoses geometry and target bounds',preview['generatedBoxes']==5 and preview['target']['fits'])
  b.screenshot(a.out/'image-preview.png');b.click('imageToolApply');b.wait('return !document.getElementById("imageToolsDialog").open',20);b.wait('return window.__morphforge.state.revision>'+str(before),20)
  scene=api('scene')['scene'];check('pixel plane applies five colored voxels',len(scene['boxes'])==5 and all(x['size']==[1,1,1] for x in scene['boxes']))
  check('top image row maps above bottom row without left-right mirroring',next(x for x in scene['boxes'] if x['min']==[-1,1,0])['styleId']!=next(x for x in scene['boxes'] if x['min']==[-1,0,0])['styleId'])
  check('transparent pixel stays empty',not any(x['min']==[1,1,0] for x in scene['boxes']))
  check('generated RGB maps directly to bundled avatar',api('state')['materialCompatibility']['matched'])
  b.js('window.__morphforge.renderer.view("front")');b.screenshot(a.out/'pixel-plane.png');b.click('undo');b.wait('return window.__morphforge.state.stats.boxCount===0 || window.__morphforge.state.boxCount===0 || window.__morphforge.renderer.boxes.length===0',20);check('image geometry and palette changes undo together',not api('scene')['scene']['boxes'] and len(api('scene')['materials'])==24)
  b.click('imageToolsOpen');b.wait('return document.getElementById("imageToolsDialog").open');b.js('document.getElementById("imageToolId").value=arguments[0];document.getElementById("imageToolId").dispatchEvent(new Event("change"));document.getElementById("imageToolWidth").value=6;document.getElementById("imageToolAspect").checked=true;',item['id']);b.click('imageToolResize');b.wait('return document.getElementById("imageToolId").value!==arguments[0]'.replace('arguments[0]',json.dumps(item['id'])),20)
  derived=json.loads(b.js('return document.getElementById("imageToolReport").textContent'));check('resize creates a separate aspect-preserving reference',derived['width']==6 and derived['height']==4 and derived['parentImageId']==item['id'])
  images=api('references')['images'];check('original and derivative both remain in local library',any(i['id']==item['id'] for i in images) and any(i['id']==derived['id'] for i in images));b.click('imageToolsClose')
  b.js('document.querySelector("button[data-tab=sync]").click()');b.click('loopReplay');b.wait('return window.__morphforge.state.connection.loopReplay===true',15);check('loop setting does not enable OSC',not api('state')['connection']['enabled'])
  b.js('document.getElementById("sendOrder").value="bottom_up";document.getElementById("sendOrder").dispatchEvent(new Event("change"))');b.wait('return window.__morphforge.state.connection.sendOrder==="bottom_up"',15)
  b.call(b.path+'/refresh',{});b.wait('return window.__morphforge?.ready',30);check('loop and order persist through reload',b.js('return window.__morphforge.state.connection.loopReplay && window.__morphforge.state.connection.sendOrder==="bottom_up"'))
  logs=b.call(b.path+'/log',{'type':'browser'});check('no browser or shader errors',not [x for x in logs if x['level']=='SEVERE']);report={'success':True,'checks':checks,'scope':'Real browser controls against isolated backend; no real OSC or paid API'}
 except Exception as e:
  b.screenshot(a.out/'failure.png');report={'success':False,'checks':checks,'error':str(e),'browserLogs':b.call(b.path+'/log',{'type':'browser'})};raise
 finally:
  (a.out/'validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2));b.close()
if __name__=='__main__':main()
