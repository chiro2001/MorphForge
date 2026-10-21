#!/usr/bin/env python3
"""Pixel-level production WebGL material orientation/repeat and face-culling checks."""
import argparse,io,json,urllib.request,zipfile
from pathlib import Path
from check_editor_browser import Browser
from check_materials import fixture,png

def main():
 p=argparse.ArgumentParser();p.add_argument('--url',required=True);p.add_argument('--out',type=Path,required=True);p.add_argument('--webdriver',default='http://127.0.0.1:9518');a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True);b=Browser(a.webdriver);report={}
 def api(path,body=None):
  with urllib.request.urlopen(urllib.request.Request(a.url+'/api/'+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'}),timeout=40) as r:return json.load(r)
 try:
  buffer=io.BytesIO(fixture())
  with zipfile.ZipFile(buffer,'a',zipfile.ZIP_DEFLATED) as z:
   z.writestr('assets/mf/textures/block/stripes.png',png(16,16,[[255,255,255,255]]*8+[[0,0,0,255]]*8))
   z.writestr('assets/mf/models/block/stripes.json',json.dumps({'parent':'minecraft:block/cube_all','textures':{'all':'mf:block/stripes'}}))
  boundary='MF6MaterialAcceptance';body=(f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="mf6-original-fixture.zip"\r\n\r\n').encode()+buffer.getvalue()+f'\r\n--{boundary}--\r\n'.encode()
  with urllib.request.urlopen(urllib.request.Request(a.url+'/api/packs/import',data=body,headers={'Content-Type':'multipart/form-data; boundary='+boundary}),timeout=40) as response:imported=json.load(response)
  assert imported['ok'];styles=api('scene')['materials'];six=next(m for m in styles if m['source']=='assets/mf/models/block/sixface.json');stripes=next(m for m in styles if m['source']=='assets/mf/models/block/stripes.json')
  b.call(b.path+'/url',{'url':a.url});b.wait('return window.__morphforge?.ready',30)
  result=b.js('''return (async()=>{
   const THREE=await import('three'),r=window.__morphforge.renderer,checks=[];
   r.renderer.setAnimationLoop(null);r.controls.enabled=false;
   const convert=n=>new THREE.Vector3(n.x,n.y,-n.z),normals=[new THREE.Vector3(1,0,0),new THREE.Vector3(-1,0,0),new THREE.Vector3(0,1,0),new THREE.Vector3(0,-1,0),new THREE.Vector3(0,0,1),new THREE.Vector3(0,0,-1)];
   const colors=[[1,0,0],[0,1,0],[0,0,1],[1,1,0],[0,1,1],[1,0,1]],axes=[new THREE.Quaternion(),new THREE.Quaternion().setFromAxisAngle(normals[4],Math.PI/2),new THREE.Quaternion().setFromAxisAngle(normals[0],Math.PI/2),new THREE.Quaternion().setFromAxisAngle(normals[0],-Math.PI/2),new THREE.Quaternion().setFromAxisAngle(normals[4],Math.PI),new THREE.Quaternion().setFromAxisAngle(normals[4],-Math.PI/2)];
   const scene={format:'MorphForgeScene/2',revision:1,cellSize:.25,size:256,boxes:[{id:1,min:[-4,-4,-4],size:[8,8,8],styleId:arguments[0].id,tintId:0,motionPhase:0,group:'gpu'}],triangles:[]};
   function view(n){r.helpers.visible=false;r.camera.position.copy(convert(n.clone().multiplyScalar(5)));r.camera.up.copy(convert(Math.abs(n.y)>.5?normals[4]:normals[2]));r.camera.lookAt(0,0,0);r.camera.updateMatrixWorld();r.render();}
   function pixel(point){const gl=r.renderer.getContext(),size=r.renderer.getDrawingBufferSize(new THREE.Vector2()),p=convert(point).project(r.camera),out=new Uint8Array(4);gl.readPixels(Math.floor((p.x+1)*.5*size.x),Math.floor((p.y+1)*.5*size.y),1,1,gl.RGBA,gl.UNSIGNED_BYTE,out);return [...out];}
   for(let code=0;code<24;code++){
    await r.load(scene,[{...arguments[0],textureRotation:code,textureScale:4}],{mode:'vrc',scale:1});
    const rotation=new THREE.Quaternion().setFromAxisAngle(normals[2],-Math.PI/2*(code%4)).multiply(axes[Math.floor(code/4)]).invert();
    for(let face=0;face<6;face++){
     view(normals[face]);const p=pixel(new THREE.Vector3()),normal=normals[face].clone().applyQuaternion(rotation),source=Math.abs(normal.x)>.5?(normal.x>0?0:1):Math.abs(normal.y)>.5?(normal.y>0?2:3):(normal.z>0?4:5);
     const lo=Math.min(...p.slice(0,3)),hi=Math.max(...p.slice(0,3)),mask=p.slice(0,3).map(n=>Number(n>(lo+hi)/2));
     checks.push({kind:'face',code,face,source,pixel:p,passed:hi-lo>40&&mask.every((n,i)=>n===colors[source][i])});
    }
   }
   scene.boxes[0].styleId=arguments[1].id;
   for(const scale of [0,1,4,8]){
    await r.load(scene,[{...arguments[1],textureScale:scale}],{mode:'vrc',scale:1});view(normals[4]);let transitions=0,previous=null;
    for(let i=0;i<200;i++){const p=pixel(new THREE.Vector3(0,-.99+1.98*i/199,1)),next=p[0]>80;if(previous!==null&&next!==previous)transitions++;previous=next;}
    const expected=scale===1?15:scale===4?3:1;checks.push({kind:'repeat',scale,transitions,expected,passed:transitions===expected});
   }
   scene.boxes=[];scene.triangles=[{id:2,vertices:[[-4,-4,0],[4,-4,0],[0,4,0]],color:[255,180,40],doubleSided:false,group:'triangle'}];
   for(const doubleSided of [false,true]){
    scene.triangles[0].doubleSided=doubleSided;await r.load(scene,[],{mode:'vrc',scale:1});
    for(const face of [4,5]){view(normals[face]);const p=pixel(new THREE.Vector3()),visible=p[0]>80;checks.push({kind:'triangle',doubleSided,face,pixel:p,passed:visible===(doubleSided||face===4)});}
   }
   return {success:checks.every(c=>c.passed),checks,scope:'Actual production WebGL shader, original six-face textures, 24 proper quaternion frames, tiling and single/double-sided RGB faces; no model calls or OSC.'};
  })();''',six,stripes)
  logs=b.call(b.path+'/log',{'type':'browser'});errors=[v for v in logs if v['level']=='SEVERE' and 'favicon.ico' not in v['message']];result['browserErrors']=errors;result['success']=result['success'] and not errors;report=result
  print('WebGL checks:',len(result['checks']),'failures:',[c for c in result['checks'] if not c['passed']],flush=True);assert result['success']
 finally:
  (a.out/'validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2));b.close()
if __name__=='__main__':main()
