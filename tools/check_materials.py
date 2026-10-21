#!/usr/bin/env python3
"""Original synthetic texture pack -> Rust -> real WebGL color/animation checks."""
import base64,io,json,struct,time,urllib.request,zipfile,zlib,argparse
from pathlib import Path
from check_editor_browser import Browser
ROOT=Path(__file__).resolve().parents[1]
def png(w,h,rows):
 def chunk(kind,data):return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data)&0xffffffff)
 data=b''.join(b'\0'+bytes(rows[y])*w for y in range(h));return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',w,h,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(data))+chunk(b'IEND',b'')
def fixture():
 buffer=io.BytesIO();colors={'east':[230,41,41,255],'west':[41,230,41,255],'up':[41,41,230,255],'down':[230,230,41,255],'south':[41,230,230,255],'north':[230,41,230,255]}
 with zipfile.ZipFile(buffer,'w',zipfile.ZIP_DEFLATED)as z:
  z.writestr('pack.mcmeta',json.dumps({'pack':{'pack_format':46,'description':'MorphForge original visual fixture'}}))
  for face,color in colors.items():z.writestr(f'assets/mf/textures/block/{face}.png',png(16,16,[color]*16))
  z.writestr('assets/mf/models/block/sixface.json',json.dumps({'parent':'minecraft:block/cube','textures':{k:'mf:block/'+k for k in colors}}))
  z.writestr('assets/mf/textures/block/animated.png',png(16,32,[[255,0,0,255]]*16+[[0,0,255,255]]*16))
  z.writestr('assets/mf/textures/block/animated.png.mcmeta',json.dumps({'animation':{'frametime':1,'frames':[1,{'index':0,'time':2}],'interpolate':True}}))
  z.writestr('assets/mf/models/block/animated.json',json.dumps({'parent':'minecraft:block/cube_all','textures':{'all':'mf:block/animated'}}))
 return buffer.getvalue()
def main():
 p=argparse.ArgumentParser();p.add_argument('--url',default='http://127.0.0.1:8768');p.add_argument('--out',type=Path,default=ROOT/'docs/materials');a=p.parse_args();out=a.out;out.mkdir(exist_ok=True,parents=True);checks=[];b=Browser()
 def api(path,body=None):
  r=urllib.request.Request(a.url+'/api/'+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'})
  with urllib.request.urlopen(r,timeout=60)as v:return json.load(v)
 def check(name,passed):
  checks.append({'name':name,'pass':bool(passed)});print(('PASS 'if passed else'FAIL ')+name,flush=True)
  if not passed:raise AssertionError(name)
 def sample(t,point,view='front'):
  return b.js('''const r=window.__morphforge.renderer;r.renderer.setAnimationLoop(null);r.helpers.visible=false;r.view(arguments[2]);for(const u of r.uniforms)u.mfTime.value=arguments[0];r.render();const p=r.projectPoint(arguments[1]),rect=r.renderer.domElement.getBoundingClientRect(),ratio=r.renderer.getPixelRatio(),gl=r.renderer.getContext(),pixel=new Uint8Array(4);gl.readPixels(Math.floor((p.x-rect.left)*ratio),Math.floor((rect.bottom-p.y)*ratio),1,1,gl.RGBA,gl.UNSIGNED_BYTE,pixel);return [...pixel];''',t,point,view)
 try:
  pack=fixture();(Path('/home/chiro/tmp/morphforge')/'material-fixture.zip').write_bytes(pack);boundary='MorphForgeFixtureBoundary';body=(f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="visual-fixture.zip"\r\nContent-Type: application/zip\r\n\r\n').encode()+pack+f'\r\n--{boundary}--\r\n'.encode();req=urllib.request.Request(a.url+'/api/packs/import',data=body,headers={'Content-Type':'multipart/form-data; boundary='+boundary})
  with urllib.request.urlopen(req,timeout=60)as r:imported=json.load(r)
  check('Resource pack accepted and materials persisted',imported['ok'])
  catalog=api('scene')['materials'];animated=next(m for m in catalog if m['source']=='assets/mf/models/block/animated.json');six=next(m for m in catalog if m['source']=='assets/mf/models/block/sixface.json');check('Explicit frame timeline has three ticks',animated['frames']==3 and abs(animated['frameTime']-.05)<1e-6)
  api('edit',{'operations':[{'op':'clear'},{'op':'box','min':[0,0,0],'size':[4,4,4],'styleId':animated['id']}]})
  b.call(b.path+'/url',{'url':a.url});b.wait('return window.__morphforge?.ready');b.js("const s=document.getElementById('renderMode');s.value='vrc';s.dispatchEvent(new Event('change'));");b.wait("return window.__morphforge.renderer.mode==='vrc'&&window.__morphforge.renderer.uniforms.length>0")
  blue=sample(0,[.5,.5,.5]);red=sample(.051,[.5,.5,.5]);mixed=sample(.101,[.5,.5,.5]);check('Animation first tick is source frame1 blue',blue[2]>blue[0]*2);check('Second tick is source frame0 red',red[0]>red[2]*2);check('Interpolation produces mixed third tick',mixed[0]>50 and mixed[2]>50)
  b.screenshot(out/'animation-third-tick.png')
  phase_result=api('edit',{'op':'box','min':[0,0,0],'size':[4,4,4],'styleId':animated['id'],'motionPhase':32});b.wait('return window.__morphforge.renderer.boxes[0].motionPhase===32');phase_pixel=sample(0,[.5,.5,.5]);check('Per-box motionPhase offsets animation in reused GPU batch',phase_pixel[0]>phase_pixel[2]*2 and b.js('return window.__morphforge.renderer.lastUpdate.incremental'))
  revision=api('state')['revision'];api('edit',{'operations':[{'op':'clear'},{'op':'box','min':[0,0,0],'size':[4,4,4],'styleId':six['id']}]});b.wait(f'return window.__morphforge.state.revision>{revision}');b.wait('return window.__morphforge.renderer.meshes.some(m=>m.userData.styleId==='+str(six['id'])+'&&m.count>0)')
  front=sample(0,[.5,.5,.5]);side=sample(0,[.5,.5,.5],'side');top=sample(0,[.5,.5,.5],'top');check('Canonical +Z face is cyan',front[1]>front[0]*1.5 and front[2]>front[0]*1.5);check('Canonical +X face is red',side[0]>side[1]*1.5);check('Top face is blue',top[2]>top[0]*1.5)
  old=api('state')['revision'];api('edit',{'op':'box','min':[5,0,0],'size':[1,1,1],'styleId':six['id']});b.wait(f'return window.__morphforge.state.revision>{old}');b.wait('return window.__morphforge.renderer.boxes.length===2');check('Small edit reuses GPU material batch',b.js('return window.__morphforge.renderer.lastUpdate.incremental'))
  logs=b.call(b.path+'/log',{'type':'browser'});check('No browser JS/WebGL errors',not[x for x in logs if x['level']=='SEVERE']);report={'success':True,'checks':checks,'pixels':{'blue':blue,'red':red,'mixed':mixed,'front':front,'side':side,'top':top,'phase32':phase_pixel},'import':imported,'scope':'Real WebGL sampled framebuffer; original generated textures'}
 except Exception as e:
  b.screenshot(out/'visual-failure.png');report={'success':False,'checks':checks,'error':str(e),'pixels':{k:locals().get(k)for k in ['blue','red','mixed','front','side','top']}};raise
 finally:(out/'webgl-validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2));b.close()
 print(json.dumps(report,ensure_ascii=False,indent=2))
if __name__=='__main__':main()
