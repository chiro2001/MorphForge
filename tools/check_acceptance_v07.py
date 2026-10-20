#!/usr/bin/env python3
"""Actual shared-frontend acceptance; isolated data, no paid generation or OSC."""
import argparse,json,struct,time,urllib.request,urllib.error
from pathlib import Path
from check_editor_browser import Browser
from surface_pixel_probe import probe

def main():
 p=argparse.ArgumentParser();p.add_argument('--url',required=True);p.add_argument('--out',type=Path,required=True);a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True)
 b=Browser();worker=Browser();checks=[]
 def check(name,value):
  checks.append({'name':name,'passed':bool(value)});print(('PASS ' if value else 'FAIL ')+name,flush=True)
  if not value:raise AssertionError(name)
 def api(path,body=None):
  req=urllib.request.Request(a.url+'/api/'+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'})
  with urllib.request.urlopen(req,timeout=60) as r:return json.load(r)
 def tool(name,args):return api('harness/tool',{'name':name,'arguments':args})
 def current():return api('state')['revision']
 def visible_revision():b.wait('return window.__morphforge.renderer.renderRevision === '+str(current()),30)
 try:
  worker.call(worker.path+'/url',{'url':a.url+'/?capture=1'});worker.wait('return window.__morphforge?.ready',30)
  b.call(b.path+'/url',{'url':a.url});b.wait('return window.__morphforge?.ready',30)
  api('edit',{'op':'clear'});styles=api('scene')['materials'];water=next(m['id'] for m in styles if m['family']=='water')
  api('edit',{'operations':[{'op':'box','min':[-8,-4,-3],'size':[2,3,2],'styleId':1,'group':'stone'},{'op':'box','min':[0,-4,0],'size':[2,2,2],'styleId':water,'group':'water-a'},{'op':'box','min':[2,-4,0],'size':[2,2,2],'styleId':water,'group':'water-b'}]});visible_revision()
  check('default view has no debug grid and uses high quality',not b.js('return window.__morphforge.renderer.voxelGrid') and b.js('return window.__morphforge.renderer.renderer.getPixelRatio()')==2)
  state=api('state');check('negative Y is diagnosed, negative X/Z are valid coordinates',state['targetCompatibility']['outOfBoundsCount']==3 and state['targetCompatibility']['centerOffset'][1]==4)
  surfaces=api('scene')['surfaces'];check('shared transparent interface has two hidden faces',surfaces['hiddenFaces']==2)
  before=api('scene')['scene'];preview=tool('transform_scene',{'action':'center_on_ground','baseRevision':current(),'dryRun':True})
  check('transform dry run is non-mutating',api('scene')['scene']==before and preview['after']['fits'])
  b.click('centerScene');b.wait('return document.getElementById("transformDialog").open && !document.getElementById("transformApply").disabled',15);b.click('transformApply');b.wait('return !document.getElementById("transformDialog").open',15);visible_revision()
  check('UI center-on-ground fixes target bounds',api('state')['targetCompatibility']['fits'] and api('state')['stats']['bounds']['min'][1]==0)
  centered=api('scene')['scene'];b.click('undo');b.wait('return window.__morphforge.state.stats.bounds.min[1]===-4',15)
  check('whole-scene transform is undoable',api('scene')['scene']['boxes']==before['boxes'])
  api('edit',{'op':'redo'});visible_revision()
  b.click('renderOptionsOpen');b.click('voxelGrid');b.wait('return window.__morphforge.renderer.voxelGrid');b.click('showTargetBounds');b.wait('return window.__morphforge.renderer.targetBounds.visible');b.click('renderOptionsClose')
  check('view settings persist in the Rust workspace',api('view-settings')['voxelGrid'] and api('view-settings')['showTargetBounds'])
  b.call(b.path+'/refresh',{});b.wait('return window.__morphforge?.ready',30);check('view settings survive reload',b.js('return window.__morphforge.renderer.voxelGrid && window.__morphforge.renderer.targetBounds.visible'))
  b.click('pixelThemeOpen');b.wait('return document.getElementById("pixelThemeDialog").open');b.js('document.getElementById("pixelThemeChoice").value="pixel-workshop"');b.click('applyPixelTheme');b.wait('return !document.getElementById("pixelThemeDialog").open',20);visible_revision()
  materials=api('scene')['materials'];pixel=[m for m in materials if m['uid'].startswith('mf-pixel:')]
  check('offline pixel theme adds nineteen complete materials',len(pixel)==19 and all(len(m['textures'])==6 for m in pixel))
  check('applying theme preserves existing geometry by default',api('scene')['scene']['boxes']==centered['boxes'])
  stone=next(m['id'] for m in pixel if m['uid']=='mf-pixel:stone');grass=next(m['id'] for m in pixel if m['uid']=='mf-pixel:grass')
  api('edit',{'operations':[{'op':'box','min':[-3,0,-5],'size':[5,4,4],'styleId':stone,'group':'pixel-stone'},{'op':'box','min':[-3,4,-5],'size':[5,1,4],'styleId':grass,'group':'pixel-grass'}]});visible_revision()
  check('texture arrays have independent mipmaps',b.js('return window.__morphforge.renderer.textures.some(t=>t.isDataArrayTexture && t.generateMipmaps && t.anisotropy>=1)'))
  b.js('const e=document.getElementById("renderMode");e.value="vrc";e.dispatchEvent(new Event("change"))');b.wait('return window.__morphforge.renderer.mode==="vrc"',20);visible_revision()
  check('VRC preview enables virtual light in real material programs',b.js('return window.__morphforge.renderer.uniforms.length>0 && window.__morphforge.renderer.uniforms.every(u=>u.mfVirtual.value===1)'))
  capture=tool('capture_views',{'revision':current(),'views':['front','top'],'shots':[{'id':'water-detail','fit':{'group':'water-a'},'yaw':25,'pitch':25,'projection':'orthographic'}],'mode':'vrc','quality':'high','width':640,'height':480})
  check('custom shots, VRC light and supersampling share one capture API',len(capture['images'])==3 and all(i['rendering']['mode']=='vrc' and i['rendering']['supersampling']==2 and not i['rendering']['voxelGrid'] for i in capture['images']))
  for img in capture['images']:
   raw=urllib.request.urlopen(a.url+img['url']).read();check('capture output keeps requested dimensions: '+img['view'],struct.unpack('>II',raw[16:24])==(640,480));(a.out/(img['view']+'.png')).write_bytes(raw)
  check('capture restores the observer debug-grid preference',b.js('return window.__morphforge.renderer.voxelGrid'))
  b.screenshot(a.out/'workbench.png')
  catalog=urllib.request.urlopen(a.url+'/api/vrc/catalog').read();(a.out/'catalog.zip').write_bytes(catalog)
  import zipfile,io
  with zipfile.ZipFile(io.BytesIO(catalog)) as z:manifest=json.loads(z.read('catalog.json'));check('export includes original pixel license and face-mask capability','licenses/MorphForge-Pixel-MIT.txt' in z.namelist() and 'transparent-face-mask/1' in manifest['capabilities'])
  upload='''const input=document.getElementById('avatarCatalogFile');const dt=new DataTransfer();dt.items.add(new File([JSON.stringify(arguments[0])],'catalog.json',{type:'application/json'}));input.files=dt.files;input.dispatchEvent(new Event('change'));'''
  b.js(upload,manifest);b.wait('return window.__morphforge.state.materialCompatibility.status==="matched"',20);check('target catalog can be imported through the UI',api('state')['materialCompatibility']['matched'])
  api('publish',{});check('offline publication preserves source animation phase and data',api('scene')['scene']['boxes']==api('export')['project']['scene']['boxes'] and not api('state')['connection']['enabled'])
  broken=json.loads(json.dumps(manifest));next(m for m in broken['materials'] if m['id']==1)['color']=[.01,.01,.01];b.js(upload,broken);b.wait('return window.__morphforge.state.materialCompatibility.status==="mismatch"',20)
  try:api('publish',{});blocked=False
  except urllib.error.HTTPError as e:blocked='MATERIAL_CATALOG_MISMATCH' in e.read().decode()
  check('mismatched target material blocks publication before OSC',blocked)
  b.js(upload,manifest);b.wait('return window.__morphforge.state.materialCompatibility.status==="matched"',20)
  schemas=api('harness/tools')['tools'];camera=next(t for t in schemas if t['name']=='capture_views')['parameters']['properties'];check('model-facing schema exposes render quality, mode and transforms',all(k in camera for k in ['mode','quality','supersampling','voxelGrid','surfaceCulling']) and any(t['name']=='transform_scene' for t in schemas))
  mask_pixels=probe(b);check('integer face masks do not create interpolation holes',mask_pixels['changedPixels']==0)
  logs=b.call(b.path+'/log',{'type':'browser'})+worker.call(worker.path+'/log',{'type':'browser'});check('no JS or GPU shader errors',not [x for x in logs if x['level']=='SEVERE'])
  check('all acceptance actions leave OSC off',not api('state')['connection']['enabled'])
  report={'success':True,'checks':checks,'capture':capture,'surfaceMaskPixels':mask_pixels,'browserLogs':logs,'scope':'Real Chromium frontend + screenshot worker; no paid model or OSC'}
 except Exception as e:
  b.screenshot(a.out/'failure.png');report={'success':False,'checks':checks,'error':str(e),'browserLogs':b.call(b.path+'/log',{'type':'browser'})+worker.call(worker.path+'/log',{'type':'browser'})};raise
 finally:
  (a.out/'validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2));b.close();worker.close()
 print(json.dumps({'success':True,'checks':len(checks)},ensure_ascii=False))
if __name__=='__main__':main()
