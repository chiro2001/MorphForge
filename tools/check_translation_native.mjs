// Actual isolated Tauri/WebView2 acceptance. Native acknowledgement contains
// OS window visibility/topmost queried by the shell, not echoed preferences.
import fs from 'node:fs';
const [sessionPath,outRoot]=process.argv.slice(2);if(!sessionPath||!outRoot)throw Error('usage SESSION OUTPUT');fs.mkdirSync(outRoot,{recursive:true});const session=JSON.parse(fs.readFileSync(sessionPath,'utf8').replace(/^\uFEFF/,''));const checks=[];let report={},main,mini;const debug='http://127.0.0.1:'+session.debugPort;
async function api(path,body){const r=await fetch(session.base+'/api/'+path,body===undefined?{}:{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});const v=await r.json();if(!r.ok)throw Error(v.error);return v;}
async function wait(fn,seconds=25){const end=Date.now()+seconds*1000;while(Date.now()<end){try{const v=await fn();if(v)return v;}catch(e){if(!/context|closed|target/i.test(e.message))throw e;}await new Promise(r=>setTimeout(r,100));}throw Error('Timed out');}
function check(name,passed){checks.push({name,passed:!!passed});console.log((passed?'PASS ':'FAIL ')+name);if(!passed)throw Error(name);}
async function page(match){const targets=await(await fetch(debug+'/json/list')).json();const target=targets.find(t=>t.type==='page'&&t.url.startsWith(session.base)&&match(t.url));if(!target)return null;const ws=new WebSocket(target.webSocketDebuggerUrl);let seq=0;const pending=new Map(),errors=[];ws.onmessage=e=>{const m=JSON.parse(e.data);if(m.id){const p=pending.get(m.id);if(!p)return;pending.delete(m.id);clearTimeout(p.timer);m.error?p.reject(Error(JSON.stringify(m.error))):p.resolve(m.result);}else if(m.method==='Runtime.exceptionThrown')errors.push(m.params);};await new Promise((ok,bad)=>{ws.onopen=ok;ws.onerror=bad;});function send(method,params={}){return new Promise((resolve,reject)=>{const id=++seq,timer=setTimeout(()=>reject(Error('CDP timeout')),25000);pending.set(id,{resolve,reject,timer});ws.send(JSON.stringify({id,method,params}));});}await send('Runtime.enable');const js=async expression=>{const r=await send('Runtime.evaluate',{expression:`(async()=>{${expression}})()`,awaitPromise:true,returnByValue:true,userGesture:true});if(r.exceptionDetails)throw Error(r.exceptionDetails.exception?.description||r.exceptionDetails.text);return r.result.value;};const click=async selector=>{const r=await js(`const n=document.querySelector(${JSON.stringify(selector)});if(!n)throw Error('Missing button');n.scrollIntoView({block:'center'});if(n.disabled)throw Error('Disabled button');const r=n.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2};`);await send('Input.dispatchMouseEvent',{type:'mousePressed',x:r.x,y:r.y,button:'left',clickCount:1});await send('Input.dispatchMouseEvent',{type:'mouseReleased',x:r.x,y:r.y,button:'left',clickCount:1});};return{ws,js,click,errors,send};}
const {createServer}=await import('node:http');let calls=0;
const mock=createServer(async(req,res)=>{if(req.method==='GET'){res.setHeader('content-type','application/json');res.end(JSON.stringify({data:[{id:'deepseek-flash'}]}));return;}let raw='';for await(const chunk of req)raw+=chunk;const v=JSON.parse(raw);calls++;const source=v.input[0].content[0].text;
 res.setHeader('content-type','application/json');if(source==='error fixture'){res.statusCode=429;res.end(JSON.stringify({error:{message:'local-fixture-secret'}}));return;}
 let text=v.instructions.includes('Simplified Chinese')?'你好，世界。':v.instructions.includes('Japanese')?'こんにちは、世界。':'Hello, world.';
 if(source==='原文一行\n原文二行\n原文三行')text='訳文一行\n訳文二行\n訳文三行';
 res.end(JSON.stringify({status:'completed',output:[{type:'message',role:'assistant',content:[{type:'output_text',text}]}],usage:{input_tokens:20,output_tokens:8}}));});
await new Promise(ok=>mock.listen(0,'127.0.0.1',ok));
try{
 check('isolated native desktop 0.25.0 and OSC off',(await api('state')).software.version==='0.25.0'&&!(await api('state')).connection.enabled&&(await api('state')).project.id===session.projectId);
 await api('provider/config',{apiType:'responses',baseUrl:'http://127.0.0.1:'+mock.address().port,apiKey:'local-native-fixture'});
 const created=await api('text-panels',{action:'create',id:'translation-caption',preset:'caption',baseRevision:(await api('state')).revision});check('native default caption reserves four fixed-size rows',created.panel.maxLines===4&&created.panel.size.join(',')==='128,40'&&created.panel.fontSize===8);
 main=await wait(()=>page(url=>!url.includes('/mini.html')));await wait(()=>main.js('return !!window.__morphforge?.ready'));
 for(const file of ['speech-ui.js','mini.html','text-panels.js']){const crypto=await import('node:crypto');const expected=crypto.createHash('sha256').update(fs.readFileSync('D:/Projects/VRChatProjects2/MorphForge/web/'+file)).digest('hex');const actual=crypto.createHash('sha256').update(Buffer.from(await(await fetch(session.base+'/'+file)).arrayBuffer())).digest('hex');check('embedded '+file+' matches frozen source',expected===actual);}
 await api('desktop/window',{action:'open',mode:'speech'});mini=await wait(()=>page(url=>url.includes('/mini.html')));
 await wait(()=>mini.js('return !!document.getElementById("speechTranslate")&&[...speechTarget.options].some(o=>o.value==="panel:translation-caption")'));
 await mini.js('speechTarget.value="panel:translation-caption";speechTarget.dispatchEvent(new Event("change"));');await wait(()=>mini.js('return !speechTranslate.disabled'));
 check('native mini exposes Chinese Japanese English',await mini.js('return [...speechTranslationLanguage.options].map(o=>o.value).join(",")==="zh,ja,en"'));
 await mini.click('#speechTranslate');await wait(async()=>!!(await api('speech')).settings.translationEnabled);
 for(const [language,text]of [['ja','こんにちは、世界。'],['zh','你好，世界。'],['en','Hello, world.']]){
  await mini.js(`speechTranslationLanguage.value=${JSON.stringify(language)};speechTranslationLanguage.dispatchEvent(new Event('change'));`);
  await wait(async()=>(await api('speech')).settings.translationLanguage===language);
  await mini.js('speechText.value="native original";speechText.dispatchEvent(new Event("input"));');await mini.click('#speechSend');
  await wait(async()=>(await api('scene')).scene.textPanels[0].text==='native original\n'+text);check('native '+language+' preserves source and translation',(await api('speech')).state.translation.language===language);
  check('native source remains editable for '+language,await mini.js('return speechText.value==="native original"'));
 }
 await wait(()=>mini.js('return speechTranslation.textContent.includes("Hello, world.")'));
 fs.writeFileSync(outRoot+'/native-translation.png',Buffer.from((await mini.send('Page.captureScreenshot',{format:'png'})).data,'base64'));
 await mini.js('speechTranslationLanguage.value="ja";speechTranslationLanguage.dispatchEvent(new Event("change"));');await wait(async()=>(await api('speech')).settings.translationLanguage==='ja');
 await mini.js('speechText.value="原文一行\\n原文二行\\n原文三行";speechText.dispatchEvent(new Event("input"));');await mini.click('#speechSend');
 await wait(async()=>(await api('scene')).scene.textPanels[0].text==='原文二行\n原文三行\n訳文二行\n訳文三行');const retained=await api('scene');
 check('native four-row retention keeps both source and translation at size8',retained.textLayouts[0].layout.visibleLines===4&&retained.textLayouts[0].layout.fontSize===8);
 await wait(()=>main.js('return window.__morphforge.renderer.textPanels.panels[0]?.text==="原文二行\\n原文三行\\n訳文二行\\n訳文三行"'));
 const images=await main.js('const r=window.__morphforge.renderer;return await r.capture(["bilingual"],768,360,[{id:"bilingual",projection:"orthographic",position:[0,44,128],target:[0,44,0],orthoHeight:64}],{supersampling:1});');
 fs.writeFileSync(outRoot+'/native-bilingual-four-rows.png',Buffer.from(images[0].data.split(',')[1],'base64'));
 await mini.js('speechText.value="error fixture";speechText.dispatchEvent(new Event("input"));');await mini.click('#speechSend');await wait(async()=>(await api('speech')).state.translation.phase==='error');
 check('native failure falls back and shows error without secret',(await api('scene')).scene.textPanels[0].text==='error fixture'&&!JSON.stringify(await api('speech')).includes('local-fixture-secret'));
 await wait(()=>mini.js('return speechError.textContent.includes("429")'));
 await mini.click('#speechTranslate');await wait(async()=>!(await api('speech')).settings.translationEnabled);const before=calls;
 await mini.js('speechText.value="original after disabling";speechText.dispatchEvent(new Event("input"));');await mini.click('#speechSend');await wait(async()=>(await api('scene')).scene.textPanels[0].text==='original after disabling');
 check('disabled native translation avoids model call',calls===before);
 check('no microphone modeling or OSC task started',!(await api('speech')).state.active&&(await api('state')).harness.interaction.phase==='idle'&&!(await api('state')).connection.enabled);
 check('no unhandled native JavaScript exception',main.errors.length===0&&mini.errors.length===0);
 await mini.click('#miniClose');
 report={success:true,checks,apiRequests:calls,scope:'Actual isolated Tauri/WebView2 mini, mouse input, packaged Rust provider and subtitle scene transactions with local fake LLM. No microphone, paid API or OSC.'};
}catch(e){report={success:false,checks,error:e.message};throw e;}
finally{fs.writeFileSync(outRoot+'/validation.json',JSON.stringify(report,null,2));main?.ws.close();mini?.ws.close();mock.closeAllConnections();mock.close();}
