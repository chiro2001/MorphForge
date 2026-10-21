#!/usr/bin/env python3
"""Real UI checks on an isolated v16 workspace. Never enables OSC or calls a model."""
import argparse, base64, json, time, urllib.request
from pathlib import Path
from check_editor_browser import Browser
from check_materials import png

def main():
 p=argparse.ArgumentParser();p.add_argument('--url',required=True);p.add_argument('--out',type=Path,required=True);p.add_argument('--webdriver',default='http://127.0.0.1:19528');a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True)
 b=Browser(a.webdriver);checks=[];report={}
 def api(path,body=None):
  with urllib.request.urlopen(urllib.request.Request(a.url+'/api/'+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'}),timeout=60)as r:return json.load(r)
 def check(name,value):
  checks.append({'name':name,'passed':bool(value)});print(('PASS 'if value else 'FAIL ')+name,flush=True);assert value,name
 def select(identifier,value):
  b.js('const n=document.getElementById(arguments[0]);n.value=arguments[1];n.dispatchEvent(new Event("change",{bubbles:true}));',identifier,value)
 try:
  b.call(b.path+'/url',{'url':a.url});b.wait('return window.__morphforge?.ready',30)
  check('isolated v16 backend with OSC disabled',api('state')['software']['version']=='0.16.0' and not api('state')['connection']['enabled'])
  select('modelingStyle','lowpoly');b.wait('return window.__morphforge.state.project.modelingStyle==="lowpoly"')
  select('modelingExtentPreset','64');b.wait('return window.__morphforge.state.project.modelingExtent?.[0]===64')
  check('creation selectors sit beside the input and persist',api('state')['project']['modelingExtent']==[64,64,64] and b.js('return document.getElementById("modelingStyle").closest("section").id')=='tab-ai')
  select('modelingExtentPreset','custom');b.js('document.getElementById("modelingExtentX").value=40;document.getElementById("modelingExtentY").value=80;document.getElementById("modelingExtentZ").value=24;document.getElementById("modelingExtentZ").dispatchEvent(new Event("change"));')
  b.wait('return JSON.stringify(window.__morphforge.state.project.modelingExtent)==="[40,80,24]"')
  check('noncubic target size reaches dynamic harness context',api('harness/tool',{'name':'get_context','arguments':{}})['modelingExtent']==[40,80,24])
  select('modelingExtentPreset','auto');select('modelingStyle','auto');b.wait('return window.__morphforge.state.project.modelingExtent===null&&window.__morphforge.state.project.modelingStyle==="auto"')
  check('automatic type and size remain available',True)
  api('project',{'cellSize':.4});api('edit',{'op':'box','min':[-8,0,-4],'size':[16,20,8],'styleId':1})
  b.wait('return window.__morphforge.renderer.boxes.length===1',20)
  b.js('window.__morphforge.renderer.view("hero");window.__morphforge.renderer.controls.enableDamping=false;')
  for scale in [.0005,.05,.5]:
   api('placement',{'scale':scale});b.wait(f'return Math.abs(window.__morphforge.state.connection.placement.scale-{scale})<1e-7')
   before=b.js('return window.__morphforge.renderer.cameraBookmark()')
   screen=b.js('return window.__morphforge.renderer.projectPoint([0,4,0])')
   select('renderMode','vrc');b.wait('return window.__morphforge.renderer.mode==="vrc"',20)
   after=b.js('return window.__morphforge.renderer.cameraBookmark()');screen2=b.js('return window.__morphforge.renderer.projectPoint([0,2.5,0])')
   # cameraBookmark is expressed in grid cells, projectPoint in world units.
   check(f'art to VRC preserves normalized camera at scale {scale}',max(abs(x-y)for k in ['position','target']for x,y in zip(before[k],after[k]))<1e-5)
   select('renderMode','art');b.wait('return window.__morphforge.renderer.mode==="art"',20)
   back=b.js('return window.__morphforge.renderer.cameraBookmark()')
   check(f'roundtrip keeps orbit and zoom at scale {scale}',max(abs(x-y)for k in ['position','target']for x,y in zip(before[k],back[k]))<1e-5)
  b.js('document.querySelector("[data-tab=sync]").click()');b.click('avatarCatalogMf6');b.wait('return !!window.__morphforge.state.connection.catalogIdentity')
  b.click('publish');b.wait('return window.__morphforge.state.connection.progress.totalFrames!==null')
  check('publication progress is shown without claiming remote receipt',b.js('return !!document.querySelector("progress#syncProgress")&&document.getElementById("syncProgressNote").textContent.includes("不确认")&&document.getElementById("syncProgressLabel").textContent.includes("帧")'))
  saved=api('artworks',{'action':'save','title':'可删除 GUI 作品','tags':[]})['artwork'];before=api('export')['project']
  b.click('libraryOpen');b.wait('return !!document.querySelector("[data-artwork-id=\\"'+saved['id']+'\\"]")')
  b.js('const card=[...document.querySelectorAll("[data-artwork-id]")].find(n=>n.dataset.artworkId===arguments[0]);window.__deleteTarget=[...card.querySelectorAll("button")].find(n=>n.textContent==="删除");window.__deleteTarget.id="deleteFixtureArtwork";',saved['id']);b.click('deleteFixtureArtwork')
  b.wait('return !document.querySelector("[data-artwork-id=\\"'+saved['id']+'\\"]")')
  check('delete button removes only the library record',api('export')['project']==before and not any(x['id']==saved['id']for x in api('artworks')['artworks']))
  check('built-in examples never show a delete button',b.js('return [...document.querySelectorAll("[data-origin=builtin] button")].every(n=>n.textContent!=="删除")'))
  b.click('libraryClose')
  transparent='data:image/png;base64,'+base64.b64encode(png(16,16,[[255,255,255,0]]*16)).decode()
  green='data:image/png;base64,'+base64.b64encode(png(16,16,[[20,240,30,255]]*16)).decode()
  result=b.js('''return (async()=>{
   const r=window.__morphforge.renderer,THREE=await import('three');r.renderer.setAnimationLoop(null);r.controls.enabled=false;r.helpers.visible=false;
   const s={id:900,uid:'fixture:leaves',name:'leaf',family:'foliage',color:[1,1,1],roughness:.8,opacity:1,emission:0,renderMode:'cutout',textures:[arguments[0],arguments[0],arguments[0],arguments[0],arguments[0],arguments[1]],frames:1};
   const scene={revision:200,cellSize:.25,size:256,boxes:[{id:1,min:[-4,-4,-4],size:[8,8,8],styleId:900,group:'leaves'}],triangles:[]};
   await r.load(scene,[s],{mode:'vrc',scale:1});r.camera.position.set(0,0,-5);r.controls.target.set(0,0,0);r.camera.up.set(0,1,0);r.camera.lookAt(0,0,0);r.camera.updateMatrixWorld();r.render();
   const gl=r.renderer.getContext(),size=r.renderer.getDrawingBufferSize(new THREE.Vector2()),pixel=new Uint8Array(4);gl.readPixels(Math.floor(size.x/2),Math.floor(size.y/2),1,1,gl.RGBA,gl.UNSIGNED_BYTE,pixel);
   return {pixel:[...pixel],doubleSided:r.meshes[0].material.side===THREE.DoubleSide};
  })();''',transparent,green)
  check('leaf holes reveal the far back face in the production WebGL shader',result['doubleSided'] and result['pixel'][1]>result['pixel'][0]*2 and result['pixel'][1]>70)
  b.screenshot(a.out/'leaves-backface.png')
  logs=b.call(b.path+'/log',{'type':'browser'});errors=[x for x in logs if x['level']=='SEVERE'and'favicon.ico'not in x['message']]
  check('no JavaScript or GPU shader errors',not errors)
  check('no OSC or model requests started',not api('state')['connection']['enabled'])
  report={'success':True,'checks':checks,'leafPixels':result,'scope':'Actual Chromium pointer/controls, persisted HTTP preferences, normalized camera across three physical scales and nondefault art cell size, gallery delete, production WebGL cutout. No OSC/model request.'}
 except Exception as e:
  b.screenshot(a.out/'failure.png');report={'success':False,'checks':checks,'error':str(e),'logs':b.call(b.path+'/log',{'type':'browser'})};raise
 finally:
  (a.out/'validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2));b.close()
if __name__=='__main__':main()
