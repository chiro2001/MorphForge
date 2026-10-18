#!/usr/bin/env python3
"""Real shared frontend smoke + 16k instancing/capture, isolated data only."""
import argparse,json,time,urllib.request
from pathlib import Path
from check_editor_browser import Browser
ROOT=Path(__file__).resolve().parents[1]
def main():
 p=argparse.ArgumentParser();p.add_argument('--url',default='http://127.0.0.1:8767');p.add_argument('--label',default='linux');a=p.parse_args();out=ROOT/'docs/workbench';out.mkdir(parents=True,exist_ok=True);b=Browser();checks=[];measure={}
 def check(name,passed):
  checks.append({'name':name,'pass':bool(passed)});print(('PASS 'if passed else'FAIL ')+name,flush=True)
  if not passed:raise AssertionError(name)
 def count(n):return b.wait(f'return window.__morphforge?.state?.stats?.boxCount==={n}',30)
 def api(path,body=None):
  r=urllib.request.Request(a.url+'/api/'+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'})
  with urllib.request.urlopen(r,timeout=60)as v:return json.load(v)
 try:
  b.call(b.path+'/url',{'url':a.url});b.wait('return window.__morphforge?.ready');check('OSC disabled on launch',not b.js('return window.__morphforge.state.connection.enabled'))
  api('project',{'cellSize':.25});b.click('clearScene');count(0);b.point([.125,0,.125]);count(1);check('Real pointer creates canonical positive cell',api('scene')['scene']['boxes'][0]['min']==[0,0,0]);
  b.point([.125,.25,.125]);b.wait('return window.__morphforge.state.stats.voxelCount===2');check('Adjacent voxels merge',api('scene')['scene']['boxes'][0]['size']==[1,2,1]);
  b.point([.125,.5,.125],shift=True);b.wait('return window.__morphforge.state.stats.voxelCount===1');check('Shift erases top cell',True)
  b.click('undo');b.wait('return window.__morphforge.state.stats.voxelCount===2');b.click('redo');b.wait('return window.__morphforge.state.stats.voxelCount===1');check('Undo and redo visible',True)
  b.click('directionsDemo');count(4);b.wait('return window.__morphforge.renderer.boxes.length===4');check('One renderer canonical/Three Z conversion',b.js('const r=window.__morphforge.renderer;return r.meshes.every(m=>m.userData.boxes.every((box,i)=>{const a=m.instanceMatrix.array;return Math.abs(a[i*16+14]+(box.min[2]+box.size[2]/2)*r.cellSize)<.00001}))'))
  b.screenshot(out/(a.label+'-directions.png'))
  for button,n in [('stress5k',5000),('stress16k',16384)]:
   start=time.monotonic();b.click(button);count(n);b.wait(f'return window.__morphforge.renderer.boxes.length==={n}',30);measure[button]={'loadSeconds':time.monotonic()-start,'instances':b.js('return window.__morphforge.renderer.meshes.reduce((s,m)=>s+m.count,0)'),'drawCalls':b.js('return window.__morphforge.renderer.renderer.info.render.calls')};check(button+' exact instances',measure[button]['instances']==n);check(button+' batches below32',measure[button]['drawCalls']<32)
  b.screenshot(out/(a.label+'-16k.png'));start=time.monotonic();capture=api('harness/tool',{'name':'capture_views','arguments':{'revision':api('state')['revision'],'views':['hero','front','top']}});measure['captureSeconds']=time.monotonic()-start;check('Independent capture binds exact revision',len(capture['images'])==3 and capture['revision']==api('state')['revision'])
  before=api('state')['revision'];api('project',{'cellSize':.125});check('Scene settings get a new immutable revision',api('state')['revision']==before+1 and api('scene?revision='+str(before))['scene']['cellSize']==.25)
  result=api('harness/tool',{'name':'run_recipe','arguments':{'code':"mf.box([0,0,0],[3,2,4],{style:'copper'});",'mode':'apply','dryRun':True}});check('Recipe sandbox dry run retains revision',api('state')['revision']==before+1)
  logs=b.call(b.path+'/log',{'type':'browser'});errors=[x for x in logs if x['level']=='SEVERE'];check('No browser JS/WebGL errors',not errors)
  report={'success':True,'checks':checks,'measurements':measure,'capture':capture,'browserLogs':logs,'scope':'Real Chromium W3C input; no OSC enabled'}
 except Exception as e:
  b.screenshot(out/(a.label+'-failure.png'));report={'success':False,'checks':checks,'measurements':measure,'error':str(e)};raise
 finally:
  (out/(a.label+'-validation.json')).write_text(json.dumps(report,ensure_ascii=False,indent=2));b.close()
 print(json.dumps(report,ensure_ascii=False,indent=2))
if __name__=='__main__':main()
