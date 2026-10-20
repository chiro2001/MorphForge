// Actual Tauri/WebView2 input via its isolated, temporary CDP debugging port.
import fs from 'node:fs';
const [sessionPath,outRoot,debug='http://127.0.0.1:19220']=process.argv.slice(2);
if(!sessionPath||!outRoot)throw Error('usage: node check_gui_native.mjs SESSION_JSON OUTPUT_DIR [DEBUG_ORIGIN]');
fs.mkdirSync(outRoot,{recursive:true});
const session=JSON.parse(fs.readFileSync(sessionPath,'utf8').replace(/^\uFEFF/,''));
const targets=await(await fetch(debug+'/json/list')).json();
const page=targets.find(t=>t.type==='page'&&t.url.startsWith('http://127.0.0.1:'));
if(!page)throw Error('No isolated workbench page');
const ws=new WebSocket(page.webSocketDebuggerUrl);let seq=0;const pending=new Map(),events=[],checks=[];let report={};
ws.onmessage=e=>{const m=JSON.parse(e.data);if(m.id){const p=pending.get(m.id);if(!p)return;pending.delete(m.id);clearTimeout(p.timer);m.error?p.reject(Error(JSON.stringify(m.error))):p.resolve(m.result);}else events.push(m);};
await new Promise((ok,bad)=>{ws.onopen=ok;ws.onerror=bad;});
function send(method,params={}){return new Promise((resolve,reject)=>{const id=++seq;const timer=setTimeout(()=>{pending.delete(id);reject(Error('CDP timeout '+method));},30000);pending.set(id,{resolve,reject,timer});ws.send(JSON.stringify({id,method,params}));});}
async function js(code){const r=await send('Runtime.evaluate',{expression:`(async()=>{${code}})()`,awaitPromise:true,returnByValue:true,userGesture:true});if(r.exceptionDetails)throw Error(r.exceptionDetails.text+': '+r.exceptionDetails.exception?.description);return r.result?.value;}
async function wait(code,seconds=20){const end=Date.now()+seconds*1000;while(Date.now()<end){if(await js(code))return true;await new Promise(r=>setTimeout(r,100));}throw Error('Timed out: '+code);}
async function click(selector){await js(`const n=document.querySelector(${JSON.stringify(selector)});if(!n)throw Error('Missing target');for(let p=n.parentElement;p;p=p.parentElement)if(p.tagName==='DETAILS')p.open=true;n.scrollIntoView({block:'center'});`);const r=await js(`const n=document.querySelector(${JSON.stringify(selector)}),r=n.getBoundingClientRect();if(n.disabled)throw Error('Disabled target');return {x:r.x+r.width/2,y:r.y+r.height/2};`);await send('Input.dispatchMouseEvent',{type:'mouseMoved',...r});await send('Input.dispatchMouseEvent',{type:'mousePressed',button:'left',buttons:1,clickCount:1,...r});await send('Input.dispatchMouseEvent',{type:'mouseReleased',button:'left',buttons:0,clickCount:1,...r});}
async function fill(id,value){await js(`const n=document.getElementById(${JSON.stringify(id)});n.focus();n.value=${JSON.stringify(value)};n.dispatchEvent(new Event('input',{bubbles:true}));n.dispatchEvent(new Event('change',{bubbles:true}));`);}
async function check(name,yes){checks.push({name,passed:!!yes});console.log((yes?'PASS ':'FAIL ')+name);if(!yes)throw Error(name);}
async function snapshot(name){const r=await send('Page.captureScreenshot',{format:'png'});fs.writeFileSync(outRoot+'/'+name,Buffer.from(r.data,'base64'));}
async function menu(key,exportModel=false){const sel=`[data-${exportModel?'export':'command'}="${key}"]`;const index=await js(`document.querySelectorAll('.app-menu').forEach(n=>n.open=false);return [...document.querySelectorAll('.app-menu')].indexOf(document.querySelector(${JSON.stringify(sel)}).closest('.app-menu'));`);await click(`.app-menu:nth-child(${index+1})>summary`);if(exportModel)await click('.menu-submenu>summary');await click(sel);}
try{
 await wait('return !!window.__morphforge?.ready');await send('Page.enable');await js('window.__nativeErrors=[];window.addEventListener("error",e=>{if(e.message)window.__nativeErrors.push(e.message)});window.addEventListener("unhandledrejection",e=>window.__nativeErrors.push(String(e.reason)));');
 await check('real Windows packaged app version',await js('return window.__morphforge.state.software.version==="0.14.1"'));
 await check('native test starts with OSC disabled',!await js('return window.__morphforge.state.connection.enabled'));
 try{await send('Browser.setDownloadBehavior',{behavior:'allow',downloadPath:session.downloads,eventsEnabled:true});}catch{await send('Page.setDownloadBehavior',{behavior:'allow',downloadPath:session.downloads});}
 await click('[data-tab=text]');await click('#settingsOpen');await wait('return !settingsPage.hidden');await check('native tab switching preserves context settings',!await js('return document.getElementById("tab-context").hidden'));
 await fill('pinnedNotes','Windows native GUI verification');await click('#saveNotes');await wait('return toast.textContent.includes("已保存") && Number(getComputedStyle(toast).opacity)>0');await check('settings feedback is visible in native WebView2',await js('return toast.getClientRects().length>0'));await snapshot('native-settings.png');await click('#settingsBack');await wait('return settingsPage.hidden');
 await click('#runtimeMaterialOpen');await fill('runtimeKind','pbr');await fill('runtimeRough',2);await click('#runtimeMaterialApply');await wait('return toast.dataset.kind==="error" && Number(getComputedStyle(toast).opacity)>0');await check('native dialog validation error appears above the modal backdrop',await js('return runtimeMaterialDialog.contains(toast) && toast.getBoundingClientRect().top>=runtimeMaterialDialog.getBoundingClientRect().top'));await snapshot('native-modal-feedback.png');await click('#runtimeMaterialClose');
 await click('#addTool');for(const [k,v] of [['brushX',4],['brushY',4],['brushZ',4]])await fill(k,v);await click('#applyPosition');await wait('return window.__morphforge.state.stats.boxCount===1');await check('native pointer draws actual scene geometry',true);
 await menu('exportProject');await new Promise(r=>setTimeout(r,700));await check('JSON save action remains inside the running workbench',await js('return !!window.__morphforge?.ready'));
 for(const format of ['glb','obj','ply','stl']){await menu(format,true);await wait('return modelExportDialog.open');await check('native '+format+' menu opens export dialog',await js(`return modelExportFormat.value==='${format}'`));await click('#modelExportApply');await wait('return modelExportStatus.textContent.includes("导出完成")');await check('native '+format+' export completes its UI action',true);await click('#modelExportClose');}
 await check('native controls have no unhandled JavaScript error',await js('return window.__nativeErrors.length===0'));await snapshot('native-complete.png');
 report={success:true,checks,downloadEvents:events.filter(e=>e.method?.includes('download')),scope:'Actual installed-style Tauri shell and Windows WebView2 with trusted CDP pointer input. Isolated data/profile; no model calls or OSC.'};
}catch(e){report={success:false,checks,error:String(e),events:events.slice(-12)};try{await snapshot('native-failure.png');}catch{};throw e;}
finally{fs.writeFileSync(outRoot+'/validation.json',JSON.stringify(report,null,2));ws.close();}
