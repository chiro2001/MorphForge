// Context meter and MF6 repair in an isolated Tauri/WebView2 window; no OSC or paid API calls.
// Usage: node check_publish_context_native.mjs SESSION_JSON OUTPUT_DIR [CDP_ORIGIN] [responses|chat]
import fs from 'node:fs';
import http from 'node:http';

const [sessionPath, out, debug = 'http://127.0.0.1:19228', apiType = 'responses'] = process.argv.slice(2);
if (!sessionPath || !out || !['responses', 'chat'].includes(apiType)) throw Error('Invalid arguments');
const session = JSON.parse(fs.readFileSync(sessionPath, 'utf8').replace(/^\uFEFF/, ''));
if (!session.projectId || !session.data.includes('publish-context-v0153')) throw Error('Refusing a non-isolated test session');
fs.mkdirSync(out, { recursive: true });
const checks = [], requests = [], peers = new Set();
const fixture = http.createServer(async (req, res) => {
  if (req.method === 'GET' && req.url === '/models') {
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ data: [{ id: 'context-fixture', context_window: 500000, max_output_tokens: 8192 }] }));
    return;
  }
  if (req.method !== 'POST' || req.url !== (apiType === 'chat' ? '/chat/completions' : '/responses')) {
    res.writeHead(404); res.end(); return;
  }
  let raw = '';
  for await (const chunk of req) raw += chunk;
  requests.push(JSON.parse(raw));
  const input = requests.length === 1 ? 125000 : requests.length === 2 ? 250000 : 300000;
  const usage = apiType === 'chat'
    ? { prompt_tokens: input, completion_tokens: 10, total_tokens: input+10, prompt_tokens_details: { cached_tokens: input-1000 } }
    : { input_tokens: input, output_tokens: 10, total_tokens: input+10, input_tokens_details: { cached_tokens: input-1000 } };
  res.writeHead(200, { 'Content-Type': 'text/event-stream' });
  const frame = value => res.write(`data: ${JSON.stringify(value)}\n\n`);
  const content = requests.length === 3 ? '本地夹具摘要：继续检查上下文表盘。' : '本地夹具回复：保留作品继续。';
  if (apiType === 'chat') {
    frame({ choices: [{ index: 0, delta: { content }, finish_reason: 'stop' }], usage });
    res.end('data: [DONE]\n\n');
  } else {
    frame({ type: 'response.completed', response: { status: 'completed', output: [{type:'message',role:'assistant',content:[{type:'output_text',text:content}]}], usage } });
    res.end();
  }
});
fixture.on('connection', socket => { peers.add(socket); socket.on('close', () => peers.delete(socket)); });
await new Promise(resolve => fixture.listen(0, '127.0.0.1', resolve));
let page;
const startupDeadline=Date.now()+45000;
while(Date.now()<startupDeadline){
  try { const targets=await(await fetch(debug+'/json/list')).json();page=targets.find(t=>t.type==='page'&&t.url.startsWith('http://127.0.0.1:'));if(page)break; } catch {}
  await new Promise(r=>setTimeout(r,200));
}
if (!page) throw Error('No isolated workbench');
const origin = new URL(page.url).origin;
const ws = new WebSocket(page.webSocketDebuggerUrl), pending = new Map();
let seq = 0, report;
ws.onmessage = e => {
  const msg = JSON.parse(e.data), request = pending.get(msg.id);
  if (!request) return;
  pending.delete(msg.id); clearTimeout(request.timer);
  msg.error ? request.reject(Error(JSON.stringify(msg.error))) : request.resolve(msg.result);
};
await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
function cdp(method, params = {}) {
  return new Promise((resolve, reject) => {
    const id = ++seq, timer = setTimeout(() => { pending.delete(id); reject(Error('CDP timeout: ' + method)); }, 30000);
    pending.set(id, { resolve, reject, timer }); ws.send(JSON.stringify({ id, method, params }));
  });
}
async function js(code) {
  const r = await cdp('Runtime.evaluate', { expression: `(async()=>{${code}})()`, awaitPromise: true, returnByValue: true, userGesture: true });
  if (r.exceptionDetails) throw Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
  return r.result?.value;
}
async function api(path, body) {
  const r = await fetch(origin + '/api/' + path, body === undefined ? {} : { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
  const value = await r.json(); if (!r.ok) throw Error(JSON.stringify(value)); return value;
}
async function wait(predicate, label) {
  const deadline = Date.now() + 20000;
  while (Date.now() < deadline) { if (await predicate()) return; await new Promise(resolve => setTimeout(resolve, 100)); }
  throw Error('Timed out: ' + label);
}
async function click(selector) {
  const r = await js(`const n=document.querySelector(${JSON.stringify(selector)});if(!n||n.disabled)throw Error('Button unavailable');n.scrollIntoView({block:'center'});const r=n.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2};`);
  await cdp('Input.dispatchMouseEvent', { type: 'mousePressed', button: 'left', buttons: 1, clickCount: 1, ...r });
  await cdp('Input.dispatchMouseEvent', { type: 'mouseReleased', button: 'left', buttons: 0, clickCount: 1, ...r });
}
function check(name, ok) {
  checks.push({ name, passed: !!ok }); console.log((ok ? 'PASS ' : 'FAIL ') + name);
  if (!ok) throw Error(name);
}
async function screenshot(name) {
  const image = await cdp('Page.captureScreenshot', { format: 'png' });
  fs.writeFileSync(out + '/' + name, Buffer.from(image.data, 'base64'));
}
try {
  await wait(() => js('return !!window.__morphforge?.ready'), 'workbench ready');
  const initial = await api('state');
  check('isolated project ID and patched version', initial.project.id === session.projectId && initial.software.version === '0.15.3');
  check('OSC remains disabled', !initial.connection.enabled);
  await js('window.__contextAuditErrors=[];window.addEventListener("error",e=>window.__contextAuditErrors.push(e.message));window.addEventListener("unhandledrejection",e=>window.__contextAuditErrors.push(String(e.reason)));');
  await api('provider/config', { baseUrl: `http://127.0.0.1:${fixture.address().port}`, apiType, stream: true, apiKey: 'context-local-fixture', sendReasoning: false });
  await api('harness/config', { model: 'context-fixture', cacheGuard: 'strict', maxRunTokens: 2000000, contextWindowTokens:1048576, autoCompact: false, goalMode: false });
  await click('[data-tab="ai"]');
  const meters=[];
  for(let i=0;i<2;i++){
    await js(`const n=document.getElementById('prompt');n.value=${JSON.stringify(i?'继续检查上下文，不累加历史请求输入':'检查当前上下文占用')};n.dispatchEvent(new Event('input',{bubbles:true}));`);
    await click('#sendMessage');
    await wait(async()=>requests.length===i+1&&(await api('harness/state')).run?.status==='completed','fixture completion');
    const h=await api('harness/state'),m=h.context.usage;
    meters.push(m);
    check(`request ${i+1} calibrates from upstream input`,m.upstreamInputTokens===(i+1)*125000&&m.usedTokens>=(i+1)*125000&&m.usedTokens<(i+1)*125000+1000);
    check(`request ${i+1} uses upstream window`,m.windowTokens===500000);
    await wait(()=>js(`return usageContext.textContent.includes('${i?'250.':'125.'}')&&usageContext.textContent.includes('${i?'50.0':'25.0'}%')`),'visible calibrated meter');
    check(`request ${i+1} displays k and percent beside composer`,await js('const n=usageContext,r=n.getBoundingClientRect();return !n.hidden&&r.width>0&&r.height>0&&n.textContent.includes("k / 500.0k")&&n.title.includes("不是累计计费");'));
  }
  const beforeCompact=await api('harness/state');
  check('current context differs from accumulated billing',beforeCompact.usage.total.inputTokens===375000&&beforeCompact.context.usage.usedTokens<251000);
  const field=apiType==='chat'?'messages':'input';
  check('actual HTTP request input remains append only',JSON.stringify(requests[1][field].slice(0,requests[0][field].length))===JSON.stringify(requests[0][field]));
  check('static instructions and tools remain unchanged',requests[0].instructions===requests[1].instructions&&JSON.stringify(requests[0].tools)===JSON.stringify(requests[1].tools));
  check('context accounting metadata never enters wire requests',requests.every(r=>!JSON.stringify(r).includes('contextUsage')&&!JSON.stringify(r).includes('upstreamInputTokens')));
  check('strict cache checks remain enabled without alert',beforeCompact.settings.cacheGuard==='strict'&&!beforeCompact.cacheMonitor.alert);
  await screenshot('context-composer.png');
  await api('harness/control',{action:'compact'});
  await wait(async()=>(await api('harness/state')).context.compactCount===1,'compact complete');
  const compact=await api('harness/state');
  check('compact discards prior input calibration',compact.context.usage.source==='local_estimate'&&compact.context.usage.upstreamInputTokens===null&&compact.context.usage.usedTokens<125000);
  await wait(()=>js('return !usageContext.textContent.includes("250.")'),'compact meter refresh');
  await api('vrc/profile',{action:'bundled_mf6'});
  await api('connect',{profile:'mf6',enabled:false});
  await click('[data-tab="sync"]');
  await wait(()=>js('return !repairPublish.disabled'),'MF6 repair available');
  const beforeRepair=(await api('state')).connection;
  await click('#repairPublish');
  await wait(async()=>(await api('state')).connection.repairing===true,'MF6 repair accepted by actual button');
  const connection=(await api('state')).connection;
  check('MF6 repair is queued without clear or OSC',!connection.enabled&&connection.pending===beforeRepair.pending&&connection.sentCommands===beforeRepair.sentCommands&&connection.remoteAcknowledged===false&&connection.baselineKind==='local_udp_sent');
  check('no unhandled UI errors',await js('return window.__contextAuditErrors.length===0'));
  await screenshot('mf6-repair.png');
  report={success:true,apiType,checks,meters,connection,requestCount:requests.length,realModelRequests:0,scope:'Actual Windows Tauri/WebView2 controls and loopback HTTP fixture. Synthetic usage, no upstream billing/cache claims, OSC disabled.'};
} catch(e) {
  report={success:false,apiType,checks,error:String(e)};
  try{await screenshot('failure.png');}catch{}
  process.exitCode=1;
} finally {
  try{await api('harness/control',{action:'stop'});}catch{}
  ws.close();
  for(const socket of peers)socket.destroy();
  fixture.close();
  fs.writeFileSync(out+'/validation.json',JSON.stringify(report,null,2)+'\n');
}
