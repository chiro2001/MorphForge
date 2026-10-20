// Real pointer regression in an isolated Tauri/WebView2 window.
// Usage: node check_pause_resume_native.mjs SESSION_JSON OUTPUT_DIR [CDP_ORIGIN] [responses|chat]
// Seed runs/conversation.json with a large synthetic history before launching.
import fs from 'node:fs';
import http from 'node:http';

const [sessionPath, out, debug = 'http://127.0.0.1:19225', apiType = 'responses'] = process.argv.slice(2);
if (!sessionPath || !out || !['responses', 'chat'].includes(apiType)) throw Error('Invalid arguments');
const session = JSON.parse(fs.readFileSync(sessionPath, 'utf8').replace(/^\uFEFF/, ''));
if (!session.projectId || !session.data.includes('pause-resume')) throw Error('Refusing a non-isolated test session');
fs.mkdirSync(out, { recursive: true });
const checks = [], requests = [], timers = new Set(), peers = new Set();
const fixture = http.createServer(async (req, res) => {
  if (req.method === 'GET' && req.url === '/models') {
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ data: [{ id: 'pause-fixture', context_window: 1048576, max_output_tokens: 393216 }] }));
    return;
  }
  if (req.method !== 'POST' || req.url !== (apiType === 'chat' ? '/chat/completions' : '/responses')) {
    res.writeHead(404); res.end(); return;
  }
  let raw = '';
  for await (const chunk of req) raw += chunk;
  requests.push(JSON.parse(raw));
  const usage = apiType === 'chat'
    ? { prompt_tokens: 160000, completion_tokens: 10, total_tokens: 160010, prompt_tokens_details: { cached_tokens: 150000 } }
    : { input_tokens: 160000, output_tokens: 10, total_tokens: 160010, input_tokens_details: { cached_tokens: 150000 } };
  res.writeHead(200, { 'Content-Type': 'text/event-stream' });
  const frame = value => res.write(`data: ${JSON.stringify(value)}\n\n`);
  frame(apiType === 'chat'
    ? { choices: [{ index: 0, delta: { reasoning_content: '本地暂停回归夹具，非真实模型。' }, finish_reason: null }] }
    : { type: 'response.reasoning_summary_text.delta', delta: '本地暂停回归夹具，非真实模型。' });
  const done = () => {
    if (apiType === 'chat') {
      frame({ choices: [{ index: 0, delta: { content: '恢复完成' }, finish_reason: 'stop' }], usage });
      res.end('data: [DONE]\n\n');
    } else {
      frame({ type: 'response.completed', response: { status: 'completed', output: [], usage } });
      res.end();
    }
  };
  if (requests.length <= 2) {
    const timer = setTimeout(done, 30000);
    timers.add(timer);
    res.on('close', () => { clearTimeout(timer); timers.delete(timer); });
  } else done();
});
fixture.on('connection', socket => { peers.add(socket); socket.on('close', () => peers.delete(socket)); });
await new Promise(resolve => fixture.listen(0, '127.0.0.1', resolve));
const targets = await (await fetch(debug + '/json/list')).json();
const page = targets.find(t => t.type === 'page' && t.url.startsWith('http://127.0.0.1:'));
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
  check('isolated project ID and patched version', initial.project.id === session.projectId && initial.software.version === '0.15.1');
  check('OSC remains disabled', !initial.connection.enabled);
  await js('window.__pauseAuditErrors=[];window.addEventListener("error",e=>window.__pauseAuditErrors.push(e.message));window.addEventListener("unhandledrejection",e=>window.__pauseAuditErrors.push(String(e.reason)));');
  await api('provider/config', { baseUrl: `http://127.0.0.1:${fixture.address().port}`, apiType, stream: true, apiKey: 'pause-local-fixture', sendReasoning: false });
  await api('harness/config', { model: 'pause-fixture', cacheGuard: 'strict', maxRunTokens: 2000000, autoCompact: false, goalMode: false });
  await click('[data-tab="ai"]');
  await js('const n=document.getElementById("prompt");n.value="验证大上下文暂停和继续";n.dispatchEvent(new Event("input",{bubbles:true}));');
  await click('#sendMessage');
  let runId;
  for (let i = 0; i < 2; i++) {
    await wait(async () => requests.length === i + 1 && await js('return !pauseAI.disabled'), 'active stream');
    const current = await api('harness/state');
    runId ||= current.run.id;
    // A real first chunk must have reached the core before the pointer pause.
    await wait(async () => (await api('events?brief=true')).events.filter(e => e.kind === 'provider.first_byte').length > i, 'streaming bytes');
    await click('#pauseAI');
    await wait(() => js('return !resumeAI.disabled && window.__morphforge.state.harness.interaction.phase==="paused"'), 'resume button enabled');
    const paused = await api('harness/state');
    check(`pause ${i + 1} preserves run and has no cache-loss alert`, paused.run.id === runId && !paused.cacheMonitor.alert);
    check(`pause ${i + 1} keeps interrupted usage unknown`, paused.cacheMonitor.unreportedInterruptions === i + 1 && paused.cacheMonitor.windowRequests === 0);
    check(`pause ${i + 1} displays an honest unknown-usage message`, await js('return cacheLiveStatus.textContent.includes("费用未知")'));
    await screenshot(`paused-${i + 1}.png`);
    await click('#resumeAI');
  }
  await wait(() => js('return window.__morphforge.state.harness.run?.status==="completed"'), 'same run completes after resume');
  const final = await api('harness/state');
  check('two pointer resumes complete the same run', final.run.id === runId && requests.length === 3);
  const field = apiType === 'chat' ? 'messages' : 'input';
  check('every actual HTTP message prefix is unchanged', requests.slice(1).every((r, i) => JSON.stringify(r[field].slice(0, requests[i][field].length)) === JSON.stringify(requests[i][field])));
  check('tools and static instructions remain stable', requests.every(r => JSON.stringify(r.tools) === JSON.stringify(requests[0].tools) && r.instructions === requests[0].instructions));
  check('only real fixture usage enters hit-rate denominator', final.cacheMonitor.windowRequests === 1 && final.cacheMonitor.lastCachedTokens === 150000);
  check('restore never authorizes a prefix reset', final.cacheMonitor.prefix.appendOnly === true && final.cacheMonitor.prefix.expectedReset === false);
  check('patched UI has no unhandled errors', await js('return window.__pauseAuditErrors.length===0'));
  check('test never enabled OSC', !(await api('state')).connection.enabled);
  await screenshot('completed.png');
  report = { success: true, apiType, checks, requests: requests.length, cacheMonitor: final.cacheMonitor, scope: 'Actual Tauri/WebView2 pointer clicks and core requests; loopback fixture usage is synthetic, no upstream cache or billing claim.', realModelRequests: 0 };
} catch (e) {
  report = { success: false, apiType, checks, error: String(e) };
  try { await screenshot('failure.png'); } catch {}
  process.exitCode = 1;
} finally {
  try { await api('harness/control', { action: 'stop' }); } catch {}
  ws.close();
  for (const timer of timers) clearTimeout(timer);
  for (const socket of peers) socket.destroy();
  fixture.close();
  fs.writeFileSync(out + '/validation.json', JSON.stringify(report, null, 2) + '\n');
}
