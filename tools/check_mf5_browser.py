#!/usr/bin/env python3
"""MF5 real browser acceptance in an isolated workspace; no OSC or paid API."""
import argparse,base64,copy,json,struct,urllib.request
from pathlib import Path
from check_editor_browser import Browser

def main():
 p=argparse.ArgumentParser();p.add_argument('--url',required=True);p.add_argument('--out',type=Path,required=True);a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True)
 b=Browser();checks=[]
 def api(path,body=None):
  with urllib.request.urlopen(urllib.request.Request(a.url+'/api/'+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'}),timeout=90) as r:return json.load(r)
 def check(name,value):
  checks.append({'name':name,'passed':bool(value)});print(('PASS ' if value else 'FAIL ')+name,flush=True)
  if not value:raise AssertionError(name)
 def visible():b.wait('return window.__morphforge?.renderer.renderRevision === window.__morphforge.state.revision',40)
 def tool(name,args):return api('harness/tool',{'name':name,'arguments':args})
 def asyncjs(script,*args):return b.call(b.path+'/execute/async',{'script':script,'args':list(args)})
 try:
  initial=api('export')['project'];initial['scene']['boxes']=[];initial['materials']=[m for m in initial['materials'] if m['id']<=24];api('import',{'project':initial})
  b.call(b.path+'/url',{'url':a.url});b.wait('return window.__morphforge?.ready',30)
  check('isolated frontend starts with OSC off',not api('state')['connection']['enabled'])
  b.js('document.querySelector("button[data-tab=sync]").click()');asyncjs('const done=arguments[arguments.length-1];document.getElementById("avatarCatalogBundled").onclick().then(()=>done(true)).catch(e=>done({error:String(e)}));');b.wait('return window.__morphforge.state.connection.profile === "mf5"',20)
  check('MF5 directory selected without sending',api('state')['materialCompatibility']['matched'] and not api('state')['connection']['enabled'])
  b.click('repairPublish');b.wait('return window.__morphforge.state.connection.repairing',15);check('offline repair button queues replay without enabling OSC',api('state')['connection']['repairing'] and not api('state')['connection']['enabled'])
  check('geometry uses 240 synchronized bits',api('state')['connection']['syncedBits']==240)
  for percent,physical in [(1,.0005),(10,.005),(100,.05),(1000,.5)]:
   b.js('const e=document.getElementById("scalePercent");e.value=arguments[0];e.dispatchEvent(new Event("change"));',percent)
   b.wait('return Math.abs(window.__morphforge.state.connection.placement.scale-arguments[0])<1e-7'.replace('arguments[0]',str(physical)),15)
   check('scale UI '+str(percent)+'% maps to physical '+str(physical),abs(api('state')['connection']['placement']['scale']-physical)<1e-7)
  b.js('Array.from(document.querySelectorAll("[data-scale]")).find(e=>e.dataset.scale==="100").click()');b.wait('return Math.abs(window.__morphforge.state.connection.placement.scale-.05)<1e-7')
  b.call(b.path+'/refresh',{});b.wait('return window.__morphforge?.ready',30)
  check('scale persists and displays new 100%',b.js('return Number(document.getElementById("scalePercent").value)')==100)
  b.click('runtimeMaterialOpen');b.wait('return document.getElementById("runtimeMaterialDialog").open')
  b.js('document.getElementById("runtimeName").value="现场桃红";document.getElementById("runtimeColor").value="#f179ad";document.getElementById("runtimeKind").value="color";')
  before=api('state')['revision'];b.click('runtimeMaterialApply');b.wait('return window.__morphforge.state.revision>'+str(before),20);visible()
  mats=api('scene')['materials'];pink=next(m for m in mats if m['name']=='现场桃红');check('real dialog creates RGB24 runtime material',pink['vrcEffective']['color']==pink['color'])
  b.js('document.getElementById("runtimeMaterialDialog").close()')
  b.click('pixelThemeOpen');b.wait('return document.getElementById("pixelThemeDialog").open');b.js('document.getElementById("pixelThemeChoice").value="block-workshop"');b.click('applyPixelTheme');b.wait('return !document.getElementById("pixelThemeDialog").open',20);visible()
  scene=api('scene');pixel=[m for m in scene['materials'] if m['uid'].startswith('mf-pixel:')];check('48 original block textures available',len(pixel)==48 and all(len(m['textures'])==6 for m in pixel))
  rev=api('state')['revision'];r=tool('define_materials',{'baseRevision':rev,'materials':[{'name':kind,'kind':kind,'color':'#89ABC1','noiseScale':1,'seed':2} for kind in ['water','cloud','wood','stone']]})
  boxes=[{'id':i+1,'min':[i*4-10,0,0],'size':[3,3,3],'styleId':m['id']} for i,m in enumerate([pink]+r['materials'])]
  manifest=api('export')['project'];manifest['scene']['boxes']=boxes;api('import',{'project':manifest});b.call(b.path+'/refresh',{});b.wait('return window.__morphforge?.ready',30);visible()
  b.js('const e=document.getElementById("renderMode");e.value="vrc";e.dispatchEvent(new Event("change"))');b.wait('return window.__morphforge.renderer.mode==="vrc"',20);visible()
  check('runtime materials all match built avatar without palette rebuild',api('state')['materialCompatibility']['matched'])
  b.screenshot(a.out/'runtime-materials.png')
  check('procedural patterns reach WebGL uniforms',b.js('return window.__morphforge.renderer.uniforms.filter(u=>u.mfProcKind?.value>0).length')==4)
  # Shrunk observer and both screenshot camera projections must still fit geometry.
  api('placement',{'scale':.0005});b.wait('return Math.abs(window.__morphforge.renderer.scale-.0005)<1e-7',15)
  result=asyncjs('''const done=arguments[arguments.length-1],r=window.__morphforge.renderer;r.view("hero");r.capture(["micro-perspective","micro-orthographic"],320,240,[{id:"micro-perspective",yaw:35,pitch:25},{id:"micro-orthographic",yaw:35,pitch:25,projection:"orthographic"}],{quality:"high"}).then(done).catch(e=>done({error:String(e)}));''')
  check('micro capture perspective and orthographic complete',isinstance(result,list) and len(result)==2)
  for image in result:
   raw=base64.b64decode(image['data'].split(',')[1]);(a.out/(image['view']+'.png')).write_bytes(raw)
   visible_pixels=asyncjs('const done=arguments[arguments.length-1],im=new Image();im.onload=()=>{const c=document.createElement("canvas");c.width=im.width;c.height=im.height;const ctx=c.getContext("2d");ctx.drawImage(im,0,0);const p=ctx.getImageData(0,0,c.width,c.height).data;let n=0;for(let i=0;i<p.length;i+=4)if(Math.abs(p[i]-p[0])+Math.abs(p[i+1]-p[1])+Math.abs(p[i+2]-p[2])>20)n++;done(n);};im.onerror=()=>done(0);im.src=arguments[0];',image['data'])
   check(image['view']+' is visible at minimum scale',visible_pixels>1000 and struct.unpack('>II',raw[16:24])==(320,240))
  # 5000 distinct colors must use one instance batch and retain the per-voxel colors.
  perf=asyncjs('''const done=arguments[arguments.length-1],r=window.__morphforge.renderer;const styles=Array.from({length:5000},(_,i)=>({id:1000+i,uid:"qa:"+i,name:"color"+i,color:[(i%251)/250,(i%127)/126,(i%61)/60],metallic:0,roughness:.9,opacity:1,emission:0,renderMode:"opaque",textures:[]}));const boxes=styles.map((m,i)=>({id:"qa-"+i,min:[i%100-50,Math.floor(i/100),0],size:[1,1,1],styleId:m.id}));r.load({size:256,revision:999999,boxes},styles,{mode:"vrc",scale:.0005}).then(()=>{r.renderer.setAnimationLoop(null);r.view("hero");r.helpers.visible=false;r.renderer.render(r.scene,r.camera);const mesh=r.meshes[0],a=Array.from(mesh.instanceColor.array.slice(0,3)),z=Array.from(mesh.instanceColor.array.slice(4999*3,5000*3));return {batches:r.meshes.length,instances:r.meshes.reduce((s,m)=>s+m.count,0),calls:r.renderer.info.render.calls,first:a,last:z};}).then(done).catch(e=>done({error:String(e)}));''')
  check('5000 colors share one batch',perf.get('batches')==1 and perf['instances']==5000 and perf['calls']<=3)
  check('per-box colors survive batching',perf['first']!=perf['last'])
  change=asyncjs('''const done=arguments[arguments.length-1],r=window.__morphforge.renderer,styles=r.styles,boxes=r.boxes.map(b=>({...b}));boxes[0].styleId=styles[4999].id;r.load({size:256,revision:1000000,boxes},styles,{mode:"vrc",scale:.0005}).then(()=>done({incremental:r.lastUpdate.incremental,color:Array.from(r.meshes[0].instanceColor.array.slice(0,3)),last:Array.from(r.meshes[0].instanceColor.array.slice(4999*3,5000*3))})).catch(e=>done({error:String(e)}));''')
  check('incremental repaint updates same-batch style changes',change.get('incremental') and change['color']==change['last'])
  logs=b.call(b.path+'/log',{'type':'browser'});errors=[x for x in logs if x['level']=='SEVERE'];check('no JS or GPU shader errors',not errors)
  check('entire test leaves OSC disabled',not api('state')['connection']['enabled'])
  report={'success':True,'checks':checks,'performance':perf,'browserLogs':logs,'scope':'Real Chromium UI and renderer; isolated workspace; no paid model or real VRChat.'}
 except Exception as e:
  b.screenshot(a.out/'failure.png');report={'success':False,'checks':checks,'error':str(e),'browserLogs':b.call(b.path+'/log',{'type':'browser'})};raise
 finally:
  (a.out/'validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2));b.close()
if __name__=='__main__':main()
