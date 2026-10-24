// Real Windows WebView2 controls against an isolated installed NSIS payload.
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
const [sessionFile,out,mode='full']=process.argv.slice(2);
const session=JSON.parse(fs.readFileSync(sessionFile,'utf8').replace(/^\uFEFF/,''));
if(!session.data.includes('localization-20261025'))throw Error('Isolated native data required');
fs.mkdirSync(out,{recursive:true});const checks=[];
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
async function wait(fn){const end=Date.now()+45000;while(Date.now()<end){const value=await fn();if(value)return value;await sleep(100);}throw Error('Condition timed out');}
function check(name,value){checks.push({name,passed:!!value});console.log((value?'PASS ':'FAIL ')+name);if(!value)throw Error(name);}
async function connect(target){
 const socket=new WebSocket(target.webSocketDebuggerUrl),pending=new Map();let seq=0;
 socket.onmessage=e=>{const m=JSON.parse(e.data),p=pending.get(m.id);if(!p)return;pending.delete(m.id);clearTimeout(p.timer);m.error?p.reject(Error(JSON.stringify(m.error))):p.resolve(m.result);};
 await new Promise((resolve,reject)=>{socket.onopen=resolve;socket.onerror=reject;});
 const call=(method,params={})=>new Promise((resolve,reject)=>{const id=++seq,timer=setTimeout(()=>{pending.delete(id);reject(Error(method+' timed out'));},30000);pending.set(id,{resolve,reject,timer});socket.send(JSON.stringify({id,method,params}));});
 const js=async code=>{const r=await call('Runtime.evaluate',{expression:`(async()=>{${code}})()`,awaitPromise:true,returnByValue:true});if(r.exceptionDetails)throw Error(r.exceptionDetails.exception?.description||r.exceptionDetails.text);return r.result.value;};
 return {socket,call,js};
}
let main,mini,base;
try{
 const targets=async()=>await(await fetch(`http://127.0.0.1:${session.debugPort}/json/list`)).json();
 const target=await wait(async()=>{try{return(await targets()).find(t=>t.type==='page'&&/^http:\/\/127\.0\.0\.1:\d+\//.test(t.url)&&!t.url.includes('/mini.html'));}catch{return null;}});
 main=await connect(target);base=new URL(target.url).origin;
 const api=async(route,body)=>{const r=await fetch(base+'/api/'+route,body===undefined?{}:{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});const v=await r.json();if(!r.ok)throw Error(v.error);return v;};
 await wait(()=>main.js('return !!window.__morphforge?.ready'));
 const before=await api('state'),scene=JSON.stringify(await api('scene'));
 check('actual Windows 0.27.0 core and OSC off',before.software.version==='0.27.0'&&!before.connection.enabled);
 if(mode==='restart'){
  check('manual language survives native application restart',await main.js('return document.documentElement.lang===\"en\"&&uiLanguage.value===\"en\"'));
 }else{
  const sha=data=>createHash('sha256').update(data).digest('hex');
  const root=path.resolve('D:/Projects/VRChatProjects2/MorphForge');
  for(const file of ['studio.html','studio.js','bootstrap.js','i18n.js','locales/messages.js'])check('packaged '+file+' matches final source',sha(Buffer.from(await(await fetch(base+'/'+file)).arrayBuffer()))===sha(fs.readFileSync(path.join(root,'web',file))));
  await main.js("window.__localeNativeSentinel='same-native-window';window.__localeErrors=[];window.addEventListener('error',e=>window.__localeErrors.push(e.message));window.addEventListener('unhandledrejection',e=>window.__localeErrors.push(String(e.reason)));prompt.value='保存 / 日本語 / English';settingsOpen.click();");
  await api('desktop/window',{action:'open',mode:'progress'});
  const miniTarget=await wait(async()=>(await targets()).find(t=>t.type==='page'&&t.url.startsWith(base+'/mini.html')));
  mini=await connect(miniTarget);await wait(()=>mini.js("return document.getElementById('miniRun').textContent.length>0"));
  for(const [language,label]of [['ja','設定'],['en','Settings'],['zh-CN','设置']]){
   await main.js(`uiLanguage.value=${JSON.stringify(language)};uiLanguage.dispatchEvent(new Event('change',{bubbles:true}));`);
   await wait(()=>main.js(`return document.documentElement.lang===${JSON.stringify(language)}&&!uiLanguage.disabled;`));
   check(language+' native interface switches immediately',await main.js(`return settingsOpen.textContent===${JSON.stringify(label)}&&window.__localeNativeSentinel==='same-native-window';`));
   check(language+' retains unsaved user input',await main.js("return prompt.value==='保存 / 日本語 / English';"));
   await wait(()=>mini.js(`return document.documentElement.lang===${JSON.stringify(language)};`));
   check(language+' native mini language synchronized',true);
   const image=await main.call('Page.captureScreenshot',{format:'png'});fs.writeFileSync(path.join(out,language+'.png'),Buffer.from(image.data,'base64'));
  }
  await main.js("uiLanguage.value='en';uiLanguage.dispatchEvent(new Event('change',{bubbles:true}));");await wait(()=>main.js("return document.documentElement.lang==='en'&&!uiLanguage.disabled"));
  check('manual preference persisted to isolated data',JSON.parse(fs.readFileSync(path.join(session.data,'ui-settings.json'),'utf8')).language==='en');
  await api('desktop/window',{action:'close'});
  check('switching leaves geometry and version unchanged',JSON.stringify(await api('scene'))===scene&&(await api('state')).revision===before.revision);
  check('no JavaScript runtime errors',!(await main.js('return window.__localeErrors')).length);
 }
 const state=await api('state');check('native verification starts no model, microphone or sender',!state.connection.enabled&&!state.speechStatus.active&&state.harness.interaction.phase==='idle');
 const report={success:true,checks,base,version:state.software.version,scope:'Actual Windows WebView2 and NSIS payload with isolated data. No microphone, paid calls or OSC.'};fs.writeFileSync(path.join(out,'validation.json'),JSON.stringify(report,null,2)+'\n');
}catch(error){fs.writeFileSync(path.join(out,'validation.json'),JSON.stringify({success:false,checks,error:String(error)},null,2)+'\n');throw error;}
finally{mini?.socket.close();main?.socket.close();}
