// Windows Tauri button checks for stream v16. Isolated data; never enables OSC.
import fs from 'node:fs';
import http from 'node:http';
const [sessionPath,out,debug='http://127.0.0.1:19231',packFile,catalogFile]=process.argv.slice(2);
const session=JSON.parse(fs.readFileSync(sessionPath,'utf8').replace(/^\uFEFF/,''));
if(!session.data.includes('stream-v16'))throw Error('Refusing nonisolated session');
fs.mkdirSync(out,{recursive:true});
let catalogHash,queryReads=0;
const fixture=http.createServer((req,res)=>{
 res.setHeader('Content-Type','application/json');
 if(req.url==='/?HOST_INFO'){res.end(JSON.stringify({OSC_TRANSPORT:'UDP',EXTENSIONS:{VALUE:true}}));return;}
 if(req.url==='/avatar/parameters'&&catalogHash){
  queryReads++;const CONTENTS={};for(const [n,v]of Object.entries({MF_InfoVersion:7,MF_InfoFeatures:7,MF_InfoBuild0:41,MF_InfoBuild1:7,MF_InfoBuild2:1}))CONTENTS[n]={VALUE:[v]};
  for(let i=0;i<32;i++)CONTENTS['MF_Catalog'+String(i).padStart(2,'0')]={VALUE:[parseInt(catalogHash.slice(i*2,i*2+2),16)]};
  res.end(JSON.stringify({CONTENTS}));return;
 }res.statusCode=404;res.end('{}');
});
await new Promise(r=>fixture.listen(0,'127.0.0.1',r));
let page;const startup=Date.now()+45000;
while(Date.now()<startup){try{const pages=await(await fetch(debug+'/json/list')).json();page=pages.find(p=>p.type==='page'&&p.url.startsWith('http://127.0.0.1:'));if(page)break;}catch{}await new Promise(r=>setTimeout(r,200));}
if(!page)throw Error('No isolated workbench');
const origin=new URL(page.url).origin,ws=new WebSocket(page.webSocketDebuggerUrl),pending=new Map(),checks=[];
let seq=0,report;
ws.onmessage=e=>{const m=JSON.parse(e.data),p=pending.get(m.id);if(!p)return;pending.delete(m.id);clearTimeout(p.timer);m.error?p.reject(Error(JSON.stringify(m.error))):p.resolve(m.result);};
await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject;});
function cdp(method,params={}){return new Promise((resolve,reject)=>{const id=++seq,timer=setTimeout(()=>{pending.delete(id);reject(Error('CDP timeout '+method));},30000);pending.set(id,{resolve,reject,timer});ws.send(JSON.stringify({id,method,params}));});}
async function js(code){const r=await cdp('Runtime.evaluate',{expression:`(async()=>{${code}})()`,awaitPromise:true,returnByValue:true,userGesture:true});if(r.exceptionDetails)throw Error(r.exceptionDetails.exception?.description||r.exceptionDetails.text);return r.result?.value;}
async function api(path,body){const r=await fetch(origin+'/api/'+path,body===undefined?{}:{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});const v=await r.json();if(!r.ok)throw Error(JSON.stringify(v));return v;}
async function wait(fn,label){const end=Date.now()+25000;while(Date.now()<end){if(await fn())return;await new Promise(r=>setTimeout(r,100));}throw Error('Timeout '+label);}
async function click(selector){const r=await js(`const n=document.querySelector(${JSON.stringify(selector)});if(!n||n.disabled)throw Error('Button unavailable '+${JSON.stringify(selector)});for(let p=n.parentElement;p;p=p.parentElement)if(p.tagName==='DETAILS')p.open=true;n.scrollIntoView({block:'center'});const r=n.getBoundingClientRect();return{x:r.x+r.width/2,y:r.y+r.height/2};`);await cdp('Input.dispatchMouseEvent',{type:'mousePressed',button:'left',buttons:1,clickCount:1,...r});await cdp('Input.dispatchMouseEvent',{type:'mouseReleased',button:'left',buttons:0,clickCount:1,...r});}
function check(name,ok){checks.push({name,passed:!!ok});console.log((ok?'PASS ':'FAIL ')+name);if(!ok)throw Error(name);}
async function shot(name){const r=await cdp('Page.captureScreenshot',{format:'png'});fs.writeFileSync(out+'/'+name,Buffer.from(r.data,'base64'));}
try{
 await wait(()=>js('return !!window.__morphforge?.ready'),'ready');
 const initial=await api('state');check('isolated project and v16 payload',initial.project.id===session.projectId&&initial.software.version==='0.16.0'&&!initial.connection.enabled);
 await js('window.__streamErrors=[];addEventListener("error",e=>window.__streamErrors.push(e.message));addEventListener("unhandledrejection",e=>window.__streamErrors.push(String(e.reason)));');
 await click('[data-tab="sync"]');
 check('retired protocols and catalog shortcuts removed',await js('return profile.options.length===1&&profile.value==="mf6"&&!document.getElementById("avatarCatalogBundled")&&!document.getElementById("avatarCatalogMf4")'));
 await click('#autoDetectAvatar');await wait(async()=>(await api('state')).connection.autoDetect===false,'auto discovery toggle');
 for(const strategy of ['incremental','rebuild','auto']){
  await js(`publishStrategy.value=${JSON.stringify(strategy)};publishStrategy.dispatchEvent(new Event('change',{bubbles:true}));`);
  await wait(async()=>(await api('state')).connection.publishStrategy===strategy,'strategy saved');
  check('strategy persists: '+strategy,(await api('state')).connection.publishStrategy===strategy);
 }
 await click('#avatarCatalogMf6');await wait(async()=>!!(await api('state')).connection.catalogIdentity,'base catalog selected');
 catalogHash=(await api('state')).connection.catalogIdentity;
 await js(`oscQueryPort.value='${fixture.address().port}';`);await click('#discoverAvatar');
 await wait(async()=>(await api('state')).connection.discovery.catalogKnown===true,'capability applied');
 const discovery=(await api('state')).connection.discovery;
 check('real OSCQuery HTTP discovers exact known catalog',queryReads===2&&discovery.info.catalogHash===catalogHash&&discovery.info.region8===true);
 check('discovery does not become execution or remote ACK',discovery.info.executionAck===false&&discovery.remoteAcknowledged===false&&!(await api('state')).connection.enabled);
 check('readable capability result is visible',await js('return discoveryStatus.textContent.includes("连续区域编码可用")&&discoveryStatus.textContent.includes("本机有匹配目录")'));
 await click('#publish');await wait(async()=>!!(await api('state')).connection.publication,'publication plan');
 check('publish button displays both simulated costs',await js('return publicationPlan.textContent.includes("增量")&&publicationPlan.textContent.includes("重建")&&publicationPlan.textContent.includes("帧")'));
 const before=(await api('state')).connection;await click('#repairPublish');await wait(async()=>(await api('state')).connection.repairing===true,'repair accepted');
 check('explicit repair never queues an automatic rebuild',(await api('state')).connection.pending===before.pending);
 if(packFile){
  const document=await cdp('DOM.getDocument');const node=await cdp('DOM.querySelector',{nodeId:document.root.nodeId,selector:'#packFile'});await cdp('DOM.setFileInputFiles',{nodeId:node.nodeId,files:[packFile]});
  await wait(()=>js('return document.getElementById("packReportDialog").open'),'resource fixture import');await click('#packReportClose');
 }
 if(catalogFile){
  const document=await cdp('DOM.getDocument');const node=await cdp('DOM.querySelector',{nodeId:document.root.nodeId,selector:'#avatarCatalogFile'});await cdp('DOM.setFileInputFiles',{nodeId:node.nodeId,files:[catalogFile]});
  await wait(async()=>(await api('state')).connection.catalogIdentity!==catalogHash,'fixture catalog imported');
  const before=(await api('export')).project;await click('#avatarMaterials');
  await wait(async()=>(await api('export')).project.materials.length===before.materials.length+1,'catalog material added');
  const after=(await api('export')).project;
  check('avatar material button merges definitions without changing geometry',JSON.stringify(before.scene.boxes)===JSON.stringify(after.scene.boxes)&&after.materials.some(m=>m.uid==='fixture:avatar-gold'));
 }
 await click('[data-tab="sync"]');
 await js('catalogPixels.value="16";catalogScope.value="all";');await click('#catalogBudget');
 await wait(()=>js('return catalogBudgetStatus.textContent.includes("GPU纹理预计")'),'budget rendered');
 const budget=await api('vrc/catalog/budget?cellPixels=16');
 check('material budget uses deduped slices and one-pixel compatibility atlas',budget.cellPixels===16&&budget.bytes.atlas===4&&(!packFile||budget.uniqueSlices===1));
 check('budget explains no new OSC bits or particle systems',await js('return catalogBudgetStatus.textContent.includes("不增加同步bit/动画器层/粒子系统")'));
 // Additional workbench acceptance uses real native input and local HTTP only.
 await click('[data-tab="ai"]');
 await js('modelingStyle.value="lowpoly";modelingStyle.dispatchEvent(new Event("change",{bubbles:true}));');
 await wait(async()=>(await api('state')).project.modelingStyle==='lowpoly','creation style saved');
 await js('modelingExtentPreset.value="64";modelingExtentPreset.dispatchEvent(new Event("change",{bubbles:true}));');
 await wait(async()=>JSON.stringify((await api('state')).project.modelingExtent)==='[64,64,64]','creation size saved');
 check('input-adjacent type and size selectors persist in native app',(await api('harness/tool',{name:'get_context',arguments:{}})).modelingExtent[0]===64&&await js('return modelingStyle.closest("section").id==="tab-ai"'));
 await js('modelingExtentPreset.value="auto";modelingExtentPreset.dispatchEvent(new Event("change",{bubbles:true}));');
 await wait(async()=>(await api('state')).project.modelingExtent===null,'automatic size');
 await js(`window.__originalFetch=window.fetch;window.__messageRequests=0;window.__preferenceGate=null;
 window.fetch=async function(input,options){
   const url=String(input);
   if(url==='/api/project')await new Promise(r=>window.__preferenceGate=r);
   if(url==='/api/harness/message'){window.__messageRequests++;return new Response(JSON.stringify({error:'Native fixture: no model request'}),{status:400,headers:{'Content-Type':'application/json'}});}
   return window.__originalFetch(input,options);
 };
 modelingExtentPreset.value='32';modelingExtentPreset.dispatchEvent(new Event('change',{bubbles:true}));`);
 await wait(()=>js('return !!window.__preferenceGate'),'preference save delayed');
 await js(`window.__preparedMessages=Promise.all([window.__morphforge.harnessUi.send({prompt:'Local duplicate-send fixture',mode:'steer',intent:'start'}),window.__morphforge.harnessUi.send({prompt:'Local duplicate-send fixture',mode:'steer',intent:'start'})]);window.__preferenceGate();`);
 await js('await window.__preparedMessages;window.fetch=window.__originalFetch;');
 check('double submit while preferences save produces only one message request',await js('return window.__messageRequests===1'));
 const revision=(await api('state')).revision;
 await api('edit',{op:'box',min:[-4,0,-4],size:[8,8,8],styleId:1,baseRevision:revision});
 await wait(()=>js('return window.__morphforge.renderer.boxes.length===1'),'render test cube');
 await js('window.__morphforge.renderer.view();window.__morphforge.renderer.controls.enableDamping=false;');
 const cameraBefore=await js('return window.__morphforge.renderer.cameraBookmark()');
 await js('renderMode.value="vrc";renderMode.dispatchEvent(new Event("change",{bubbles:true}));');
 await wait(()=>js('return window.__morphforge.renderer.mode==="vrc"'),'VRC preview');
 const cameraAfter=await js('return window.__morphforge.renderer.cameraBookmark()');
 check('native render switch preserves orbit and normalized distance',['position','target'].every(k=>cameraBefore[k].every((v,i)=>Math.abs(v-cameraAfter[k][i])<1e-5)));
 await js('renderMode.value="art";renderMode.dispatchEvent(new Event("change",{bubbles:true}));');
 await wait(()=>js('return window.__morphforge.renderer.mode==="art"'),'art preview');
 await click('[data-tab="sync"]');await click('#publish');
 await wait(async()=>(await api('state')).connection.progress.totalFrames!==null,'progress plan');
 check('native publication progress shows frames and local-only meaning',await js('return syncProgress.max===100&&syncProgressLabel.textContent.includes("帧")&&syncProgressNote.textContent.includes("不确认")'));
 const artwork=(await api('artworks',{action:'save',title:'Native delete fixture',tags:[]})).artwork;
 const projectBefore=(await api('export')).project;
 await click('#libraryOpen');
 await wait(()=>js(`return [...document.querySelectorAll('[data-artwork-id]')].some(n=>n.dataset.artworkId===${JSON.stringify(artwork.id)})`),'library fixture');
 await js(`const c=[...document.querySelectorAll('[data-artwork-id]')].find(n=>n.dataset.artworkId===${JSON.stringify(artwork.id)});[...c.querySelectorAll('button')].find(n=>n.textContent==='删除').id='nativeDeleteArtwork';`);
 await click('#nativeDeleteArtwork');
 await wait(async()=>!(await api('artworks')).artworks.some(a=>a.id===artwork.id),'library deletion');
 check('native delete button preserves open project',JSON.stringify((await api('export')).project)===JSON.stringify(projectBefore));
 check('native built-in artworks have no delete button',await js('return [...document.querySelectorAll("[data-origin=builtin] button")].every(n=>n.textContent!=="删除")'));
 await click('#libraryClose');
 check('Minecraft importer defaults to automatic grid precision',await js('return mcScale.value==="0"'));
 await shot('materials-and-publish.png');
 check('no unhandled UI errors',await js('return window.__streamErrors.length===0'));
 check('test never acquires OSC channel',!(await api('state')).connection.enabled);
 report={success:true,checks,discovery,budget,realModelRequests:0,scope:'Actual isolated Windows Tauri pointer controls and loopback OSCQuery fixture; OSC disabled. No real Avatar discovery claim.'};
}catch(e){report={success:false,checks,error:String(e)};try{await shot('failure.png');}catch{}process.exitCode=1;}
finally{ws.close();fixture.closeAllConnections();fixture.close();fs.writeFileSync(out+'/validation.json',JSON.stringify(report,null,2)+'\n');}
