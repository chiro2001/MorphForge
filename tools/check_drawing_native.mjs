// Actual isolated Tauri/WebView2 acceptance. Native acknowledgement contains
// OS window visibility/topmost queried by the shell, not echoed preferences.
import fs from 'node:fs';
const [sessionPath,outRoot]=process.argv.slice(2);if(!sessionPath||!outRoot)throw Error('usage SESSION OUTPUT');fs.mkdirSync(outRoot,{recursive:true});const session=JSON.parse(fs.readFileSync(sessionPath,'utf8').replace(/^\uFEFF/,''));const checks=[];let report={},main,mini;const debug='http://127.0.0.1:'+session.debugPort;
async function api(path,body){const r=await fetch(session.base+'/api/'+path,body===undefined?{}:{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});const v=await r.json();if(!r.ok)throw Error(v.error);return v;}
async function wait(fn,seconds=25){const end=Date.now()+seconds*1000;while(Date.now()<end){try{const v=await fn();if(v)return v;}catch(e){if(!/context|closed|target/i.test(e.message))throw e;}await new Promise(r=>setTimeout(r,100));}throw Error('Timed out');}
function check(name,passed){checks.push({name,passed:!!passed});console.log((passed?'PASS ':'FAIL ')+name);if(!passed)throw Error(name);}
async function page(match){const targets=await(await fetch(debug+'/json/list')).json();const target=targets.find(t=>t.type==='page'&&t.url.startsWith(session.base)&&match(t.url));if(!target)return null;const ws=new WebSocket(target.webSocketDebuggerUrl);let seq=0;const pending=new Map(),errors=[];ws.onmessage=e=>{const m=JSON.parse(e.data);if(m.id){const p=pending.get(m.id);if(!p)return;pending.delete(m.id);clearTimeout(p.timer);m.error?p.reject(Error(JSON.stringify(m.error))):p.resolve(m.result);}else if(m.method==='Runtime.exceptionThrown')errors.push(m.params);};await new Promise((ok,bad)=>{ws.onopen=ok;ws.onerror=bad;});function send(method,params={}){return new Promise((resolve,reject)=>{const id=++seq,timer=setTimeout(()=>reject(Error('CDP timeout')),25000);pending.set(id,{resolve,reject,timer});ws.send(JSON.stringify({id,method,params}));});}await send('Runtime.enable');const js=async expression=>{const r=await send('Runtime.evaluate',{expression:`(async()=>{${expression}})()`,awaitPromise:true,returnByValue:true,userGesture:true});if(r.exceptionDetails)throw Error(r.exceptionDetails.exception?.description||r.exceptionDetails.text);return r.result.value;};const click=async selector=>{const r=await js(`const n=document.querySelector(${JSON.stringify(selector)});if(!n)throw Error('Missing button');n.scrollIntoView({block:'center'});if(n.disabled)throw Error('Disabled button');const r=n.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2};`);await send('Input.dispatchMouseEvent',{type:'mousePressed',x:r.x,y:r.y,button:'left',clickCount:1});await send('Input.dispatchMouseEvent',{type:'mouseReleased',x:r.x,y:r.y,button:'left',clickCount:1});};return{ws,js,click,errors,send};}
try {
 check('isolated native desktop 0.23.0 and OSC disabled',(await api('state')).software.version==='0.23.0'&&!(await api('state')).connection.enabled);
 main=await wait(()=>page(url=>!url.includes('/mini.html')));await wait(()=>main.js('return !!window.__morphforge?.ready'));
 check('native host serves the expected private project',(await api('state')).project.id===session.projectId);
 const hashes=[];
 for(const file of ['drawing-ui.js','drawing.css','studio.js']) {
  const crypto=await import('node:crypto');const expected=crypto.createHash('sha256').update(fs.readFileSync('D:/Projects/VRChatProjects2/MorphForge/web/'+file)).digest('hex');
  const actual=crypto.createHash('sha256').update(Buffer.from(await(await fetch(session.base+'/'+file)).arrayBuffer())).digest('hex');hashes.push({file,sha256:actual});check('embedded '+file+' matches frozen source',actual===expected);
 }
 const before=(await api('state')).revision;
 await main.click('#drawingOpen');await wait(()=>main.js('return drawingDialog.open'));
 await main.click('#drawExample');await wait(()=>main.js('return !drawApply.disabled'));
 check('combination preview has a native WebView canvas and six primitives',await main.js('return !!drawingViewport.querySelector("canvas")&&drawMetrics.textContent.includes("6个图元")'));
 check('native preview does not mutate the project',(await api('state')).revision===before);
 fs.writeFileSync(outRoot+'/native-preview.png',Buffer.from((await main.send('Page.captureScreenshot',{format:'png'})).data,'base64'));
 await main.click('#drawApply');await wait(()=>main.js('return !drawingDialog.open'));
 const scene=(await api('scene')).scene;check('native add button persists the actual colored mesh',(scene.drawings||[]).length>40&&new Set([...(scene.triangles||[]),...(scene.drawings||[])].map(t=>t.color.join(','))).size>=5);
 check('all generated vertices fit MF6 integer coordinates',(scene.triangles||[]).every(t=>t.vertices.every(p=>p.every(Number.isInteger)&&p[0]>=-128&&p[0]<=127&&p[1]>=0&&p[1]<=255&&p[2]>=-128&&p[2]<=127))&&(scene.drawings||[]).every(p=>p.start.concat(p.end).every(Number.isInteger)&&p.diameter64>0));
 check('native main camera frames the applied mesh',await main.js('const {Vector3}=await import("three");const r=window.__morphforge.renderer,b=r.bounds();return [0,1,2,3,4,5,6,7].every(i=>{const p=new Vector3(i&1?b.max.x:b.min.x,i&2?b.max.y:b.min.y,i&4?b.max.z:b.min.z).project(r.camera);return Math.abs(p.x)<=1.03&&Math.abs(p.y)<=1.03&&p.z>=-1&&p.z<=1;});'));
 fs.writeFileSync(outRoot+'/native-scene.png',Buffer.from((await main.send('Page.captureScreenshot',{format:'png'})).data,'base64'));
 await main.click('#drawingOpen');await main.js('drawKind.value="segment";drawKind.dispatchEvent(new Event("input",{bubbles:true}));drawWidth.value="8";drawWidth.dispatchEvent(new Event("input",{bubbles:true}));');
 await main.click('#drawPreview');await wait(()=>main.js('return !drawApply.disabled'));
 check('native controls accept adjustable line thickness',await main.js('return !drawWidth.closest("[data-draw-field]").hidden&&drawWidth.value==="8"'));
 await main.js('drawingDialog.close()');await wait(()=>main.js('return !drawingViewport.querySelector("canvas")'));check('closing native preview frees its extra canvas',true);
 const state=await api('state');check('no model, microphone or OSC task was started',!state.connection.enabled&&state.harness.interaction.phase==='idle'&&!(await api('speech')).state.active);
 check('no unhandled native JavaScript exception',main.errors.length===0);
 report={success:true,checks,embeddedFiles:hashes,scope:'Actual isolated Tauri/WebView2 CDP mouse input and packaged Rust geometry; no microphone/model/VRChat OSC.'};
} catch(e) {report={success:false,checks,error:e.message};throw e;}
finally {fs.writeFileSync(outRoot+'/validation.json',JSON.stringify(report,null,2));main?.ws.close();}
