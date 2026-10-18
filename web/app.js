import * as THREE from 'three';
import { OrbitControls } from '/vendor/OrbitControls.js';

const $=id=>document.getElementById(id), viewport=$('viewport');
let state=null,tool='box',lastModelKey='',mouseDown=null,hover=null,toastTimer,editing=Promise.resolve(),polling=false;
const renderer=new THREE.WebGLRenderer({antialias:true,alpha:false});renderer.setPixelRatio(Math.min(devicePixelRatio,2));renderer.outputColorSpace=THREE.SRGBColorSpace;viewport.prepend(renderer.domElement);
const scene=new THREE.Scene();scene.background=new THREE.Color('#10171e');
const camera=new THREE.PerspectiveCamera(42,1,.005,2000);camera.position.set(5,4.3,6);
const controls=new OrbitControls(camera,renderer.domElement);controls.enableDamping=true;controls.dampingFactor=.1;controls.mouseButtons={LEFT:null,MIDDLE:THREE.MOUSE.PAN,RIGHT:THREE.MOUSE.ROTATE};controls.target.set(0,.8,0);
const content=new THREE.Group();scene.add(content);
const boxesGroup=new THREE.Group();content.add(boxesGroup);const unit=new THREE.BoxGeometry(1,1,1);
const materials=new Map(),colorBases={1:[.62,.67,.73],2:[.64,.31,.12],3:[.12,.55,.92],4:[.24,.78,.38]};
const raycaster=new THREE.Raycaster(),mouse=new THREE.Vector2(),plane=new THREE.Plane(new THREE.Vector3(0,1,0),0);
let grid=null,boundsHelper=null;
const ghost=new THREE.Mesh(unit,new THREE.MeshBasicMaterial({color:'#e9bf67',opacity:.25,transparent:true,depthWrite:false}));ghost.visible=false;content.add(ghost);
const ghostEdges=new THREE.LineSegments(new THREE.EdgesGeometry(unit),new THREE.LineBasicMaterial({color:'#efcc85',transparent:true,opacity:.9}));ghost.add(ghostEdges);
const gold=new THREE.Mesh(new THREE.SphereGeometry(.045,24,16),new THREE.MeshStandardMaterial({color:'#f8c75b',emissive:'#d48b19',emissiveIntensity:1,transparent:true,opacity:.55,roughness:.25,metalness:.4}));scene.add(gold);
const ring=new THREE.Mesh(new THREE.TorusGeometry(.065,.0015,6,48),new THREE.MeshBasicMaterial({color:'#f5c76e',transparent:true,opacity:.7}));ring.rotation.x=Math.PI/2;scene.add(ring);
scene.add(new THREE.AmbientLight(0xffffff,1.5));const light=new THREE.DirectionalLight(0xffe6bc,2);light.position.set(3,6,5);scene.add(light);
const axes=new THREE.AxesHelper(.25);scene.add(axes);

function toast(message){$('toast').textContent=message;$('toast').classList.add('visible');clearTimeout(toastTimer);toastTimer=setTimeout(()=>$('toast').classList.remove('visible'),3800);}
async function api(path,body){const response=await fetch('/api/'+path,body===undefined?{}:{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});const result=await response.json();if(!response.ok)throw new Error(result.error||'请求失败');return result;}
function effectiveScale(){return state?.connection.profile==='stage2'?1:(state?.placement.scale||1);}
function material(style,tint,w,h,axis){
  const key=[style,tint,w,h,axis].join('/');if(materials.has(key))return materials.get(key);
  const fract=v=>v-Math.floor(v);let base=colorBases[style]||[.173,.397,.619].map(v=>.25+.65*fract(style*v));
  if(tint>0)base=base.map((v,i)=>v*(.45+.55*fract(tint*[.31,.53,.79][i])));
  const shade=.52+.48*[.4,.8,.6][axis]/Math.sqrt(1.16);
  const canvas=document.createElement('canvas');canvas.width=canvas.height=128;const ctx=canvas.getContext('2d');
  ctx.fillStyle='#ffffff';ctx.fillRect(0,0,128,128);ctx.fillStyle='rgb(214,214,214)';ctx.fillRect(0,0,64,64);ctx.fillRect(64,64,64,64);ctx.fillStyle='rgb(77,77,77)';ctx.fillRect(0,0,3,128);ctx.fillRect(125,0,3,128);ctx.fillRect(0,0,128,3);ctx.fillRect(0,125,128,3);
  const texture=new THREE.CanvasTexture(canvas);texture.colorSpace=THREE.LinearSRGBColorSpace;texture.wrapS=texture.wrapT=THREE.RepeatWrapping;texture.repeat.set(w,h);texture.anisotropy=4;
  const m=new THREE.MeshBasicMaterial({map:texture,color:new THREE.Color().setRGB(...base.map(v=>v*shade),THREE.LinearSRGBColorSpace)});materials.set(key,m);return m;
}
function rebuild(){
  boxesGroup.clear();for(const m of materials.values()){m.map.dispose();m.dispose();}materials.clear();
  for(const b of state.document.boxes){
    const [x,y,z]=b.min,[sx,sy,sz]=b.size;
    const face=[material(b.styleId,b.tintId,sz,sy,0),material(b.styleId,b.tintId,sz,sy,0),material(b.styleId,b.tintId,sx,sz,1),material(b.styleId,b.tintId,sx,sz,1),material(b.styleId,b.tintId,sx,sy,2),material(b.styleId,b.tintId,sx,sy,2)];
    const mesh=new THREE.Mesh(unit,face);mesh.scale.set(sx*.25,sy*.25,sz*.25);mesh.position.set((x+sx/2-128)*.25,(y+sy/2)*.25,(z+sz/2-128)*.25);mesh.userData.box=b;boxesGroup.add(mesh);
  }
  content.scale.setScalar(effectiveScale());const n=state.document.size;
  if(grid){content.remove(grid);grid.geometry.dispose();grid.material.dispose();}
  grid=new THREE.GridHelper(n*.25,Math.min(n,64),0x606c76,0x34424f);grid.material.transparent=true;grid.material.opacity=.65;grid.position.y=Number($('layer').value)*.25;content.add(grid);
  if(boundsHelper){content.remove(boundsHelper);boundsHelper.geometry.dispose();boundsHelper.material.dispose();}
  boundsHelper=new THREE.Box3Helper(new THREE.Box3(new THREE.Vector3(-n*.125,0,-n*.125),new THREE.Vector3(n*.125,n*.25,n*.125)),0x35424e);boundsHelper.material.transparent=true;boundsHelper.material.opacity=.3;content.add(boundsHelper);
  content.updateMatrixWorld(true);$('emptyHint').hidden=state.boxCount!==0;gold.visible=ring.visible=state.placement.visible&&state.connection.profile==='anchor';
}
function applyState(value){
  state=value;const c=state.connection,p=state.placement;
  $('voxelCount').replaceChildren(document.createTextNode(state.voxelCount.toLocaleString()),Object.assign(document.createElement('small'),{textContent:' 格'}));
  $('boxCount').replaceChildren(document.createTextNode(String(state.boxCount)),Object.assign(document.createElement('small'),{textContent:' / '+state.capacity}));
  $('undo').disabled=!state.canUndo;$('redo').disabled=!state.canRedo;
  $('gridSize').value=String(state.document.size);$('layer').max=state.document.size-1;if(Number($('layer').value)>=state.document.size)$('layer').value=state.document.size-1;
  $('target').value=String(c.port);$('profile').value=c.profile;$('interval').value=String(c.interval);$('auto').checked=c.auto;
  if(document.activeElement!==$('scale'))$('scale').value=p.scale;$('scaleValue').value=Number($('scale').value).toFixed(2)+'×';$('world').checked=p.world;$('visible').checked=p.visible;
  $('placementControls').classList.toggle('legacy-disabled',c.profile==='stage2');$('legacyNote').hidden=c.profile!=='stage2';
  $('connect').textContent=c.enabled?'停止发送':'开启实时显示';$('connect').classList.toggle('connected',c.enabled);
  $('connectionBadge').textContent=c.enabled?(c.port===9000?'发送至 VRChat':'发送至 Unity'):'本地预览';$('connectionBadge').classList.toggle('live',c.sending);
  $('sendDot').className=c.error?'error':c.sending?'live':'';
  $('sendStatus').textContent=c.error||(!c.enabled?'尚未开启发送':!c.sending?'正在获取发送通道…':state.version!==c.desiredVersion?'草稿未提交，点击发送':c.pending?`待发送 ${c.pending} 项`:`当前草稿已发送 · v${c.sentVersion}`);
  const key=[state.instance,state.version,effectiveScale(),c.profile,p.visible].join('/');if(key!==lastModelKey){lastModelKey=key;rebuild();}
  window.__morphforge.state=state;
}
function edit(value){editing=editing.then(async()=>{try{applyState(await api('edit',value));}catch(e){toast(e.message);}});return editing;}
function size(){return ['brushX','brushY','brushZ'].map(id=>Math.max(1,Math.min(256,Math.floor(Number($(id).value)||1))));}
function style(){return Math.max(0,Math.min(4095,Math.floor(Number($('styleId').value)||0)));}
function operation(minimum,erase=false){return {op:erase?'erase':tool,min:minimum,size:size(),styleId:style(),tintId:Math.max(0,Math.min(63,Math.floor(Number($('tintId').value)||0))) };}
function setTool(value){tool=value;$('addTool').classList.toggle('selected',tool==='box');$('eraseTool').classList.toggle('selected',tool==='erase');}
function cursor(event){
  if(!state)return null;const rect=renderer.domElement.getBoundingClientRect();mouse.set((event.clientX-rect.left)/rect.width*2-1,-(event.clientY-rect.top)/rect.height*2+1);raycaster.setFromCamera(mouse,camera);
  const erase=tool==='erase'||event.shiftKey;const hits=raycaster.intersectObjects(boxesGroup.children,false);let point;
  if(hits.length){point=content.worldToLocal(hits[0].point.clone());point.addScaledVector(hits[0].face.normal,erase?-.0025:.0025);}
  else{
    if(erase)return null;plane.constant=-Number($('layer').value)*.25*effectiveScale();const world=raycaster.ray.intersectPlane(plane,new THREE.Vector3());if(!world)return null;point=content.worldToLocal(world);
  }
  const pos=[Math.floor(point.x/.25),Math.floor(point.y/.25),Math.floor(point.z/.25)];const n=state.document.size;
  if(pos[0]<-n/2||pos[0]>=n/2||pos[2]<-n/2||pos[2]>=n/2||pos[1]<0||pos[1]>=n)return null;
  return {pos,erase};
}
renderer.domElement.addEventListener('pointermove',e=>{
  hover=cursor(e);ghost.visible=!!hover;if(!hover)return;
  const s=size();ghost.scale.set(...s.map(v=>v*.25));ghost.position.set(...hover.pos.map((v,i)=>(v+s[i]/2)*.25));ghost.material.color.set(hover.erase?'#e18f88':'#e9bf67');ghostEdges.material.color.copy(ghost.material.color);
  $('cursorPosition').textContent=`X ${hover.pos[0]} · Y ${hover.pos[1]} · Z ${hover.pos[2]}`;
});
renderer.domElement.addEventListener('pointerleave',()=>{ghost.visible=false;hover=null;});
renderer.domElement.addEventListener('pointerdown',e=>{if(e.button===0)mouseDown=[e.clientX,e.clientY];});
renderer.domElement.addEventListener('pointerup',e=>{if(e.button!==0||!mouseDown)return;const moved=Math.hypot(e.clientX-mouseDown[0],e.clientY-mouseDown[1]);mouseDown=null;if(moved>5)return;const hit=cursor(e);if(hit)edit(operation(hit.pos,hit.erase));});
renderer.domElement.addEventListener('contextmenu',e=>e.preventDefault());
$('addTool').onclick=()=>setTool('box');$('eraseTool').onclick=()=>setTool('erase');
document.querySelectorAll('[data-style]').forEach(b=>b.onclick=()=>{$('styleId').value=b.dataset.style;document.querySelectorAll('[data-style]').forEach(x=>x.classList.toggle('selected',x===b));});
$('applyPosition').onclick=()=>edit(operation(['coordX','coordY','coordZ'].map(id=>Math.floor(Number($(id).value)||0))));
$('layer').oninput=()=>{$('layerValue').value=$('layer').value;if(grid)grid.position.y=Number($('layer').value)*.25;};
$('gridSize').onchange=()=>edit({op:'size',size:Number($('gridSize').value)}).then(fit);
$('stairsDemo').onclick=()=>edit({op:'demo',name:'stairs'}).then(fit);$('houseDemo').onclick=()=>edit({op:'demo',name:'house'}).then(fit);
$('undo').onclick=()=>edit({op:'undo'});$('redo').onclick=()=>edit({op:'redo'});$('clearScene').onclick=()=>edit({op:'clear'});
window.addEventListener('keydown',e=>{if(['INPUT','SELECT','TEXTAREA'].includes(document.activeElement.tagName))return;if(e.ctrlKey||e.metaKey){if(e.key.toLowerCase()==='z'){e.preventDefault();edit({op:e.shiftKey?'redo':'undo'});}if(e.key.toLowerCase()==='y'){e.preventDefault();edit({op:'redo'});}}});
$('importButton').onclick=()=>$('importFile').click();$('importFile').onchange=async e=>{try{const f=e.target.files[0];if(!f)return;await edit({op:'import',document:JSON.parse(await f.text())});fit();}catch(error){toast(error.message);}e.target.value='';};
$('exportButton').onclick=()=>{if(!state)return;const blob=new Blob([JSON.stringify(state.document,null,2)],{type:'application/json'});const url=URL.createObjectURL(blob);const a=document.createElement('a');a.href=url;a.download='morphforge-scene.json';a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);};
function config(enabled=state?.connection.enabled){return {enabled,port:Number($('target').value),profile:$('profile').value,interval:Number($('interval').value),auto:$('auto').checked};}
async function configure(enabled){try{applyState(await api('connect',config(enabled)));}catch(e){toast(e.message);}}
$('connect').onclick=()=>configure(!state.connection.enabled);for(const id of ['target','profile','interval','auto'])$(id).onchange=()=>configure(state.connection.enabled);
$('publish').onclick=async()=>{try{applyState(await api('publish',{}));if(!state.connection.enabled)toast('草稿已准备；开启实时显示后发送');}catch(e){toast(e.message);}};
$('resync').onclick=async()=>{try{applyState(await api('publish',{resync:true}));if(!state.connection.enabled)toast('开启实时显示后，将重建当前模型');}catch(e){toast(e.message);}};
async function placement(value){try{applyState(await api('placement',value));}catch(e){toast(e.message);}}
$('world').onchange=()=>placement({world:$('world').checked});$('visible').onchange=()=>placement({visible:$('visible').checked});
$('scale').oninput=()=>{$('scaleValue').value=Number($('scale').value).toFixed(2)+'×';content.scale.setScalar(Number($('scale').value));};$('scale').onchange=()=>placement({scale:Number($('scale').value)});
$('resetAnchor').onclick=()=>placement({reset:true});
function fit(){
  if(!state)return;const bounds=new THREE.Box3().setFromObject(boxesGroup),factor=effectiveScale();let center,span;
  if(boxesGroup.children.length){center=bounds.getCenter(new THREE.Vector3());span=Math.max(...bounds.getSize(new THREE.Vector3()).toArray(),.5);}
  else{span=state.document.size*.25*factor;center=new THREE.Vector3(0,span*.2,0);}
  controls.target.copy(center);camera.position.copy(center).add(new THREE.Vector3(1,.8,1.2).normalize().multiplyScalar(span*2.2+1));camera.near=Math.max(.001,span/10000);camera.updateProjectionMatrix();controls.update();
}
$('fitView').onclick=fit;$('resetView').onclick=()=>{camera.position.set(5,4.3,6);controls.target.set(0,.8,0);controls.update();};
function resize(){const rect=viewport.getBoundingClientRect();renderer.setSize(rect.width,rect.height);camera.aspect=rect.width/rect.height;camera.updateProjectionMatrix();}
new ResizeObserver(resize).observe(viewport);resize();
renderer.setAnimationLoop(()=>{controls.update();renderer.render(scene,camera);});
window.__morphforge={state:null,ready:false,projectPoint(p){const v=new THREE.Vector3(...p).multiplyScalar(effectiveScale()).project(camera);const r=renderer.domElement.getBoundingClientRect();return {x:r.left+(v.x+1)*r.width/2,y:r.top+(1-v.y)*r.height/2};},renderer};
async function poll(){if(polling)return;polling=true;try{applyState(await api('state'));window.__morphforge.ready=true;}catch(e){$('sendStatus').textContent='本机服务暂不可用';}finally{polling=false;}}
await poll();fit();setInterval(poll,500);
