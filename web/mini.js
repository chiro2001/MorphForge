import {t as uiText, bindText} from "./i18n.js";
import {StreamText} from '/stream-text.js';
const $=id=>document.getElementById(id);const query=new URLSearchParams(location.search);let mode=query.get('mode')||'speech',state=null,desktop=null,busy=false;
async function api(path,body){const r=await fetch('/api/'+path,body===undefined?{}:{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});const v=await r.json();if(!r.ok)throw Error(v.error||uiText("请求失败"));return v;}
const fail=e=>{bindText($('miniErrorText'), () => (e.message));$('miniError').hidden=false;};
$('miniErrorClose').onclick=()=>{$('miniError').hidden=true;};
const transcript=new StreamText($('miniTranscript'));
function showMode(){mode=['speech','progress','custom'].includes(mode)?mode:'speech';$('miniMode').value=mode;$('miniSpeech').hidden=mode==='progress';$('miniProgress').hidden=mode==='speech';}
$('miniMode').onchange=async()=>{mode=$('miniMode').value;showMode();ensureSpeech();try{await api('desktop/window',{action:'open',mode});}catch(e){fail(e);}};
$('miniOpacity').oninput=()=>{document.body.style.opacity=$('miniOpacity').value;};
$('miniOpacity').onchange=()=>api('desktop/window',{action:'appearance',opacity:Number($('miniOpacity').value)}).catch(fail);
$('miniCompact').onchange=()=>{document.body.classList.toggle('compact',$('miniCompact').checked);localStorage.setItem('miniCompact',String($('miniCompact').checked));};
$('miniCompact').checked=localStorage.getItem('miniCompact')==='true';document.body.classList.toggle('compact',$('miniCompact').checked);
$('miniRecording').onclick=()=>api('speech/control',{action:'stop'}).catch(fail);
$('miniPin').onchange=()=>api('desktop/window',{action:'pin',pinned:$('miniPin').checked}).catch(fail);
$('miniMain').onclick=()=>api('desktop/window',{action:'show_main'}).catch(fail);
$('miniOnly').onclick=()=>api('desktop/window',{action:'hide_main'}).catch(fail);
$('miniClose').onclick=async()=>{try{await api('desktop/window',{action:'close'});if(!desktop?.native)window.close();}catch(e){fail(e);}};
for(const [id,action]of [['miniPause','pause'],['miniResume','resume'],['miniStop','stop']])$(id).onclick=()=>api('harness/control',{action}).catch(fail);
async function refresh(){if(busy)return;busy=true;try{
 state=await api('state');desktop=await api('desktop/window');const prefs=desktop.preferences;const opacity=prefs.miniOpacity??1;if(document.activeElement!==$('miniOpacity')){$('miniOpacity').value=opacity;document.body.style.opacity=opacity;}bindText($('miniWindowStatus'), () => (desktop.nativeStatus?.error||''));$('miniPin').checked=!!prefs.miniPinned;$('miniPin').disabled=!desktop.native;$('miniOnly').disabled=!desktop.native;
 $('miniRecording').hidden=!state.speechStatus?.active;$('miniRecording').disabled=state.speechStatus?.phase==='finishing';
 bindText($('miniProject'), () => (state.project.name));const h=state.harness,phase=h.interaction.phase;bindText($('miniRun'), () => (uiText("{0} · {1}步", ({running:uiText("建模中"),compacting:uiText("整理上下文"),paused:uiText("已暂停"),pausing:uiText("正在暂停"),interrupting:uiText("正在打断"),idle:uiText("待命"),stopped:uiText("已停止"),completed:uiText("完成"),failed:uiText("出错")})[phase]||phase, h.run?.steps||0)));$('miniPause').disabled=!['running','compacting'].includes(phase);$('miniResume').disabled=phase!=='paused';$('miniStop').disabled=!['running','compacting','pausing','paused','interrupting'].includes(phase);
 bindText($('miniResources'), () => (uiText("{0} Box · {1} 面 · {2} 格", state.stats.boxCount, state.stats.triangleCount||0, state.stats.voxelCount)));
 const c=state.connection,p=c.progress;const known=Number.isFinite(p?.percent);if(known)$('miniSendProgress').value=p.percent;else $('miniSendProgress').removeAttribute('value');bindText($('miniSendStatus'), () => (c.enabled?uiText("{0} · 待发{1}批{2} · 本机发送", known?p.percent.toFixed(1)+'%':c.pending?uiText("发送中"):uiText("等待发送"), c.pending, c.pending&&Number.isFinite(c.etaSeconds)?uiText(" · 约")+c.etaSeconds.toFixed(1)+uiText("秒"):''):uiText("OSC关闭")));
 if(mode!=='speech'){const events=await api('timeline?session=current&limit=100');const texts=events.events.filter(e=>['provider.text_delta','provider.reasoning_delta'].includes(e.kind));transcript.setText(texts.length?texts.map(e=>e.body?.text||e.data?.text||'').join(''):(h.run?.message||''));}
 if(window.speechMiniRefresh)await window.speechMiniRefresh(api,state);
 }catch(e){fail(e);}finally{busy=false;}}
showMode();refresh();setInterval(refresh,500);
// Shared speech controller is loaded separately so progress-only windows do not
// acquire microphone devices or load local recognition models.
let speechReady=false;async function ensureSpeech(){if(mode==='progress'||speechReady)return;speechReady=true;try{const m=await import('/speech-ui.js');m.setupMiniSpeech({$,api,fail,getState:()=>state});}catch(e){speechReady=false;fail(e);}}ensureSpeech();
