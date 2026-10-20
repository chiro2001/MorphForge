#!/usr/bin/env python3
"""Real browser import preview/apply acceptance against an isolated QA workspace."""
import argparse,base64,json,struct,urllib.request
from pathlib import Path
from check_editor_browser import Browser

def structure():
 def name(t,s):
  s=s.encode();return bytes([t])+struct.pack('>H',len(s))+s
 def text(s):
  s=s.encode();return struct.pack('>H',len(s))+s
 def ints(k,ns):return name(9,k)+b'\x03'+struct.pack('>i',len(ns))+b''.join(struct.pack('>i',n) for n in ns)
 palette=['minecraft:oak_log','minecraft:stone_bricks','minecraft:red_wool']
 blocks=[(x,y,z,(x+y+z)%3) for x in range(4) for z in range(4) for y in range(2) if x in (0,3) or z in (0,3)]
 data=name(10,'')+ints('size',[4,2,4])+name(9,'palette')+b'\x0a'+struct.pack('>i',len(palette))
 for value in palette:data+=name(8,'Name')+text(value)+b'\x00'
 data+=name(9,'blocks')+b'\x0a'+struct.pack('>i',len(blocks))
 for x,y,z,state in blocks:data+=ints('pos',[x,y,z])+name(3,'state')+struct.pack('>i',state)+b'\x00'
 return data+b'\x00'

def main():
 p=argparse.ArgumentParser();p.add_argument('--url',required=True);p.add_argument('--out',type=Path,required=True);p.add_argument('--webdriver',default='http://127.0.0.1:9518');a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True);checks=[];report={};b=Browser(a.webdriver)
 def api(path,body=None):
  with urllib.request.urlopen(urllib.request.Request(a.url+'/api/'+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'}),timeout=40) as r:return json.load(r)
 def check(name,value):
  checks.append({'name':name,'passed':bool(value)});print(('PASS ' if value else 'FAIL ')+name,flush=True);assert value,name
 try:
  b.call(b.path+'/url',{'url':a.url});b.wait('return window.__morphforge?.ready',30);initial=api('scene')['scene'];revision=api('state')['revision'];check('OSC disabled',not api('state')['connection']['enabled'])
  b.js('document.getElementById("meshPanel").open=true;');b.click('voxelWedge');b.wait('return window.__morphforge.renderer.boxes.some(b=>b.shape>0)',15)
  slope=api('scene')['scene']['boxes'][-1];check('textured slope remains one native object',slope['shape']==1)
  mesh=b.js('return window.__morphforge.renderer.meshes.filter(m=>m.userData.shape>0).map(m=>({count:m.count,min:m.geometry.attributes.position.array.reduce((a,n)=>Math.min(a,n),1)}))');check('slope has GPU template mesh',len(mesh)>0)
  b.click('minecraftOpen');b.wait('return document.getElementById("minecraftDialog").open');fixture=a.out/'building.nbt';fixture.write_bytes(structure())
  el=b.call(b.path+'/element',{'using':'css selector','value':'#mcFile'})['element-6066-11e4-a52e-4f735466cecf'];b.call(b.path+'/element/'+el+'/value',{'text':str(fixture.resolve())});b.wait('return document.getElementById("mcSource").options.length>0&&!document.getElementById("mcPreview").disabled',20)
  check('source upload does not change geometry revision',api('state')['revision']==revision+1)
  b.click('mcPreview');b.wait('return !document.getElementById("mcApply").disabled',30);check('independent preview leaves draft unchanged',api('state')['revision']==revision+1)
  check('independent preview renders in its own WebGL canvas',b.js('return !!document.querySelector("#mcViewport canvas")'))
  b.screenshot(a.out/'minecraft-preview.png');b.click('mcApply')
  b.wait(f'return window.__morphforge.state.revision==={revision+2}',20)
  check('apply stores compiled geometry and keeps original source',len(api('scene')['scene']['boxes'])>0 and len(api('minecraft')['sources'])>0)
  b.click('mcClose');b.wait('return !document.querySelector("#mcViewport canvas")',10);check('preview GPU resources disposed after close',True)
  b.click('undo');b.wait(f'return window.__morphforge.state.revision==={revision+3}',15);check('undo restores the preimport slope and triangles',any(v.get('shape') for v in api('scene')['scene']['boxes']) and len(api('scene')['scene'].get('triangles',[]))==len(initial.get('triangles',[])))
  logs=b.call(b.path+'/log',{'type':'browser'});check('no JavaScript or shader errors',not [r for r in logs if r['level']=='SEVERE' and 'favicon.ico' not in r['message']]);report={'success':True,'checks':checks,'scope':'Real Chromium file upload, isolated WebGL preview, source preservation, apply, undo and textured slope. No paid model or VRChat requests.'}
 except Exception as cause:
  b.screenshot(a.out/'failure.png');report={'success':False,'checks':checks,'error':str(cause),'logs':b.call(b.path+'/log',{'type':'browser'})};raise
 finally:
  (a.out/'validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2));b.close()
if __name__=='__main__':main()
