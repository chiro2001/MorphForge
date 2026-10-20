// Isolated Windows Tauri controls and actual WebGL coplanar pixels.
import fs from 'node:fs';
const [sessionPath,out,debug='http://127.0.0.1:19227']=process.argv.slice(2);
const session=JSON.parse(fs.readFileSync(sessionPath,'utf8').replace(/^\uFEFF/,''));
if(!session.data.includes('overlap-v0152'))throw Error('Refusing non-isolated data');
fs.mkdirSync(out,{recursive:true});
let page;
const startupDeadline=Date.now()+45000;
while(Date.now()<startupDeadline){
 try{const targets=await(await fetch(debug+'/json/list')).json();page=targets.find(t=>t.type==='page'&&t.url.startsWith('http://127.0.0.1:'));if(page)break;}catch{}
 await new Promise(r=>setTimeout(r,200));
}
if(!page)throw Error('No isolated page');
const origin=new URL(page.url).origin,ws=new WebSocket(page.webSocketDebuggerUrl),pending=new Map(),checks=[];
let seq=0,report;
ws.onmessage=e=>{const m=JSON.parse(e.data),p=pending.get(m.id);if(!p)return;pending.delete(m.id);clearTimeout(p.timer);m.error?p.reject(Error(JSON.stringify(m.error))):p.resolve(m.result);};
await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject;});
function cdp(method,params={}){return new Promise((resolve,reject)=>{const id=++seq,timer=setTimeout(()=>{pending.delete(id);reject(Error('CDP timeout'));},45000);pending.set(id,{resolve,reject,timer});ws.send(JSON.stringify({id,method,params}));});}
async function js(code){const r=await cdp('Runtime.evaluate',{expression:`(async()=>{${code}})()`,awaitPromise:true,returnByValue:true,userGesture:true});if(r.exceptionDetails)throw Error(r.exceptionDetails.exception?.description||r.exceptionDetails.text);return r.result?.value;}
async function api(path,body){const r=await fetch(origin+'/api/'+path,body===undefined?{}:{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});const value=await r.json();if(!r.ok)throw Error(JSON.stringify(value));return value;}
async function wait(predicate,label){const end=Date.now()+20000;while(Date.now()<end){if(await predicate())return;await new Promise(r=>setTimeout(r,100));}throw Error('Timeout '+label);}
function check(name,ok){checks.push({name,passed:!!ok});console.log((ok?'PASS ':'FAIL ')+name);if(!ok)throw Error(name);}
async function snapshot(name){const r=await cdp('Page.captureScreenshot',{format:'png'});fs.writeFileSync(out+'/'+name,Buffer.from(r.data,'base64'));}
try{
 await wait(()=>js('return !!window.__morphforge?.ready'),'ready');
 const initial=await api('state');check('isolated patched project with OSC disabled',initial.project.id===session.projectId&&initial.software.version==='0.15.2'&&!initial.connection.enabled);
 await js('window.__syncErrors=[];window.addEventListener("error",e=>window.__syncErrors.push(e.message));window.addEventListener("unhandledrejection",e=>window.__syncErrors.push(String(e.reason)));');
 const levels=[.1,.2,.25,.28,.3,.4,.5];
 check('seven requested send intervals are selectable',await js('return JSON.stringify([...document.getElementById("interval").options].map(o=>Number(o.value)))')===JSON.stringify(levels));
 for(const interval of levels){
  await js(`const n=document.getElementById('interval');n.value='${interval}';n.dispatchEvent(new Event('change',{bubbles:true}));`);
  await wait(async()=>Math.abs((await api('state')).connection.interval-interval)<1e-8,'interval persisted');
  await wait(()=>js(`return Number(document.getElementById('interval').value)===${interval}`),'interval UI refresh');
  check(`${interval.toFixed(2)}s persists through the live UI`,await js(`return Number(document.getElementById('interval').value)===${interval}`));
 }
 await js('for(const value of ["0.28","0.4","0.3"]){const n=document.getElementById("interval");n.value=value;n.dispatchEvent(new Event("change",{bubbles:true}));}');
 await wait(()=>js('return !document.getElementById("connect").disabled'),'queued settings settled');
 await new Promise(r=>setTimeout(r,1200));
 check('rapid preset edits survive subsequent status polling',(await api('state')).connection.interval===.3&&await js('return document.getElementById("interval").value==="0.3"'));
 await api('connect',{interval:.33});
 await wait(()=>js('return document.getElementById("interval").value==="0.33"'),'custom interval visible');
 await js('const n=document.getElementById("targetCapacity");n.value="16383";n.dispatchEvent(new Event("change",{bubbles:true}));');
 await wait(async()=>(await api('state')).connection.capacity===16383,'unrelated edit saved');
 check('custom interval survives an unrelated settings edit',(await api('state')).connection.interval===.33);
 await js('const n=document.getElementById("interval");n.value="0.3";n.dispatchEvent(new Event("change",{bubbles:true}));document.querySelector("[data-tab=sync]").click();');
 await wait(async()=>(await api('state')).connection.interval===.3,'preset restored');
 check('local ACK is explicitly distinguished from remote delivery',await js('return document.getElementById("intervalHint").textContent.includes("本机ACK也不能确认远端接收")'));
 await snapshot('intervals.png');
 const pixels=await js(`
 const r=window.__morphforge.renderer;
 const scene={revision:999,size:256,cellSize:.25,boxes:[{id:1,min:[-4,0,-2],size:[8,8,2],styleId:1,group:'box',tintId:0,motionPhase:0}],triangles:[{id:2,vertices:[[-4,0,0],[4,0,0],[0,8,0]],color:[230,0,0],doubleSided:false,group:'detail'}]};
 const styles=[{id:1,color:[0,1,0],roughness:.8,metallic:0,opacity:1,emission:0,renderMode:'opaque'}];
 const result=[];const difference=(a,b,i)=>Math.max(Math.abs(a[i]-b[i]),Math.abs(a[i+1]-b[i+1]),Math.abs(a[i+2]-b[i+2]));
 const canvas=document.createElement('canvas');canvas.width=256;canvas.height=256;const context=canvas.getContext('2d',{willReadFrequently:true});
 const originalSize=r.renderer.getSize({set(x,y){this.x=x;this.y=y;return this;}}),ratio=r.renderer.getPixelRatio(),helper=r.helpers.visible;
 try{
  r.renderer.setPixelRatio(1);r.renderer.setSize(256,256,false);r.helpers.visible=false;
  for(const mode of ['art','vrc']){
   await r.load(scene,styles,{mode,scale:1});
   // load() rebuilds the grid and restores the viewport's size/pixel ratio.
   // Reapply capture dimensions after load so the fixture samples the image
   // rather than cropping the upper-left of the full desktop viewport.
   r.renderer.setPixelRatio(1);r.renderer.setSize(256,256,false);r.helpers.visible=false;
   const triangle=r.triangleMeshes[0],box=r.meshes[0];
   for(const bias of [false,true])for(const yaw of [-65,-40,-15,0,15,40,65])for(const first of [true,false]){
    triangle.material.polygonOffset=bias;triangle.material.needsUpdate=true;triangle.renderOrder=first?0:2;box.renderOrder=1;
    const angle=yaw*Math.PI/180;r.viewShot({id:'probe',projection:'perspective',position:[Math.sin(angle)*16,4.5,Math.cos(angle)*16],target:[0,4,0],fov:42},256,256);
    const render=()=>{r.renderer.render(r.scene,r.camera);context.drawImage(r.renderer.domElement,0,0);return context.getImageData(0,0,256,256).data;};
    box.visible=false;triangle.visible=false;const empty=render();triangle.visible=true;const tri=render();box.visible=true;triangle.visible=false;const vox=render();triangle.visible=true;const actual=render();let total=0,wrong=0;
    // Art mode adds environment/specular light to pure colors. Compare with
    // actual reference pixels rather than assuming a particular RGB hue.
    for(let y=2;y<254;y++)for(let x=2;x<254;x++){const i=y*256+x;let inside=true;for(const d of [0,1,-1,256,-256,257,-257,255,-255]){const p=(i+d)*4;if(difference(tri,empty,p)<10||difference(vox,empty,p)<10||difference(tri,vox,p)<30){inside=false;break;}}if(inside){total++;if(difference(actual,tri,i*4)>2)wrong++;}}
    result.push({mode,bias,yaw,first,testedPixels:total,wrongPixels:wrong,passed:total>20&&wrong===0});
   }
  }
 }finally{for(const m of r.triangleMeshes)m.material.polygonOffset=true;r.helpers.visible=helper;r.renderer.setPixelRatio(ratio);r.renderer.setSize(originalSize.x,originalSize.y,false);}
 return result;
 `);
 fs.writeFileSync(out+'/pixel-checks.json',JSON.stringify(pixels,null,2)+'\n');
 check('WebGL negative control actually exhibits coplanar conflicts',pixels.some(p=>!p.bias&&!p.passed&&p.testedPixels>20));
 check('biased triangles win in both preview modes, 14 views/orders each',pixels.filter(p=>p.bias).every(p=>p.passed));
 check('visual verification never changes saved geometry',(await api('state')).revision===0);
 check('no unhandled JavaScript errors',await js('return window.__syncErrors.length===0'));
 check('native test never takes the OSC lease',!(await api('state')).connection.enabled);
 await snapshot('coplanar.png');
 report={success:true,checks,pixels,scope:'Actual Windows Tauri controls and GPU preview pixels; OSC disabled and saved geometry untouched. Does not claim the uploaded Avatar was updated.'};
}catch(e){report={success:false,checks,error:String(e)};try{await snapshot('failure.png');}catch{};process.exitCode=1;}
finally{fs.writeFileSync(out+'/validation.json',JSON.stringify(report,null,2)+'\n');ws.close();}
