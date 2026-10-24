// Actual Chromium/WebView2 clicks against an isolated core. OSC stays disabled.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';

const [sessionFile, out] = process.argv.slice(2);
const session = JSON.parse(fs.readFileSync(sessionFile, 'utf8').replace(/^\uFEFF/, ''));
if (!session.data?.includes('model-motion-v026') || !session.projectId) throw Error('Isolated session required');
fs.mkdirSync(out, { recursive: true });
const checks = [], requests = new Map();
let socket, seq = 0, base, report;
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
async function wait(fn) {
  const end = Date.now() + 25000;
  while (Date.now() < end) { const v = await fn(); if (v) return v; await sleep(100); }
  throw Error('Condition timed out');
}
function check(name, passed) {
  checks.push({ name, passed: !!passed }); console.log((passed ? 'PASS ' : 'FAIL ') + name);
  if (!passed) throw Error(name);
}
function cdp(method, params = {}) {
  return new Promise((resolve, reject) => {
    const id = ++seq, timer = setTimeout(() => { requests.delete(id); reject(Error(method + ' timed out')); }, 25000);
    requests.set(id, { resolve, reject, timer }); socket.send(JSON.stringify({ id, method, params }));
  });
}
async function js(code) {
  const result = await cdp('Runtime.evaluate', { expression: `(async()=>{${code}})()`, awaitPromise: true, returnByValue: true });
  if (result.exceptionDetails) throw Error(result.exceptionDetails.exception?.description || result.exceptionDetails.text);
  return result.result.value;
}
async function click(selector) {
  const point = await js(`const n=document.querySelector(${JSON.stringify(selector)});if(!n||n.disabled)throw Error('Button unavailable');n.scrollIntoView({block:'center'});const r=n.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2};`);
  await cdp('Input.dispatchMouseEvent', { type: 'mousePressed', ...point, button: 'left', clickCount: 1 });
  await cdp('Input.dispatchMouseEvent', { type: 'mouseReleased', ...point, button: 'left', clickCount: 1 });
}
async function api(route, body) {
  const r = await fetch(base + '/api/' + route, body === undefined ? {} : { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
  const value = await r.json(); if (!r.ok) throw Error(value.error); return value;
}
async function screenshot(name) {
  const r = await cdp('Page.captureScreenshot', { format: 'png' });
  fs.writeFileSync(path.join(out, name), Buffer.from(r.data, 'base64'));
}
try {
  const target = await wait(async () => {
    const pages = await (await fetch(`http://127.0.0.1:${session.debugPort}/json/list`)).json();
    return pages.find(p => p.type === 'page' && p.url.startsWith(session.base || 'http://127.0.0.1:') && !p.url.includes('/mini.html'));
  });
  base = new URL(target.url).origin;
  socket = new WebSocket(target.webSocketDebuggerUrl);
  socket.onmessage = event => {
    const m = JSON.parse(event.data), p = requests.get(m.id); if (!p) return;
    requests.delete(m.id); clearTimeout(p.timer); m.error ? p.reject(Error(JSON.stringify(m.error))) : p.resolve(m.result);
  };
  await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
  await cdp('Page.reload');
  await sleep(500);
  await wait(() => js('return !!window.__morphforge?.ready&&!window.__publishAuditRequests'));
  const initial = await api('state');
  check('isolated 0.26.1 project with OSC disabled', initial.project.id === session.projectId && initial.software.version === '0.26.1' && !initial.connection.enabled);
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  const sha256 = data => createHash('sha256').update(data).digest('hex');
  const embedded = await Promise.all(['studio.html', 'studio.js', 'creative-ui.js'].map(async name =>
    sha256(Buffer.from(await (await fetch(base + '/' + name)).arrayBuffer())) === sha256(fs.readFileSync(path.join(root, 'web', name)))));
  check('packaged publish UI matches the final source files', embedded.every(Boolean));
  await api('connect', { enabled: false, autoDetect: false, profile: 'mf6', port: 19012 });
  await api('edit', { op: 'box', min: [-4, 24, -4], size: [8, 8, 8], styleId: 1, group: 'rotor' });
  const catalog = JSON.parse(fs.readFileSync(path.join(root, 'materials/mf6/catalog.json'), 'utf8'));
  catalog.capabilities.push('mf6-motion4/1', 'mf6-motion-session/1');
  const form = new FormData(); form.append('file', new Blob([JSON.stringify(catalog)], { type: 'application/json' }), 'catalog.json');
  const imported = await fetch(base + '/api/vrc/profile/import', { method: 'POST', body: form });
  if (!imported.ok) throw Error(await imported.text());
  const clip = {
    name: '随模型发布的旋转', duration: 4, looping: true,
    tracks: [{ group: 'rotor', pivot: [0, 28, 0], interpolation: 'linear', keys: [0, 2, 4].map((time, i) => ({ time, translation: [0, 0, 0], rotation: [0, i * 180, 0], scale: [1, 1, 1] })) }],
  };
  for (const id of ['spin', 'second']) {
    const creative = await api('creative');
    await api('creative/animation', { action: 'save', id, baseCreativeRevision: creative.revision, clip: { ...clip, name: id === 'spin' ? clip.name : '第二个片段' } });
    if (id === 'spin') {
      await wait(() => js('return publishAnimationClip.value==="spin"&&window.__morphforge.creative.data.revision===1'));
      check('single saved clip is selected without opening animation panel', await js('return motionPanel.hidden&&motionClip.value==="spin"&&!motionExport.hidden'));
      await click('[data-tab="sync"]');
      await click('#publish');
      await wait(async () => (await api('state')).connection.motion.clipId === 'spin' && await js('return !publish.disabled'));
      check('default main publication starts the only animation', (await api('state')).connection.motion.state === 1);
      await cdp('Page.reload'); await sleep(500);
      await wait(() => js('return !!window.__morphforge?.ready&&publishAnimationClip.value==="spin"'));
      check('reopening keeps single animation ready without manual selection', await js('return motionPanel.hidden&&motionClip.value==="spin"'));
    }
  }
  const fixture = await api('state');
  await wait(() => js(`return document.querySelectorAll('#publishAnimationClip option').length===3&&window.__morphforge.state.revision===${fixture.revision}&&window.__morphforge.creative.data.revision===${fixture.creativeRevision}`));
  await js(`window.__publishAuditErrors=[];window.addEventListener('error',e=>window.__publishAuditErrors.push(e.message));window.addEventListener('unhandledrejection',e=>window.__publishAuditErrors.push(String(e.reason)));window.__publishAuditRequests=[];const original=window.fetch;window.fetch=async function(...args){const r=await original.apply(this,args);if(String(args[0])==='/api/publish')window.__publishAuditRequests.push({body:JSON.parse(args[1].body),status:r.status});return r;};`);
  await click('#animationOpen');
  await js('motionClip.value="spin";motionClip.dispatchEvent(new Event("change"));');
  check('animation panel selection is mirrored beside main publish', await js('return publishAnimationClip.value==="spin"&&publishAnimationHint.textContent.includes("随模型发布的旋转")'));
  await click('#motionClose');
  await click('[data-tab="sync"]');
  await click('#publish');
  await wait(async () => (await api('state')).connection.motion.clipId === 'spin');
  await wait(() => js('return window.__publishAuditRequests.length===1&&!publish.disabled'));
  const first = (await api('state')).connection;
  check('main button queues selected motion for playback with geometry', first.motion.attached && first.motion.state === 1 && first.motion.groups.rotor === 1 && first.motion.starting && first.pending > 0);
  check('model and motion use one publish request', await js('return window.__publishAuditRequests.length===1&&window.__publishAuditRequests[0].body.animation.id==="spin"&&window.__publishAuditRequests[0].status===200'));
  check('publishing leaves OSC disabled', !first.enabled && first.sentCommands === 0);
  await wait(() => js('return !publish.disabled'));
  await screenshot('main-publish-with-motion.png');

  await js('publishAnimationClip.value="second";publishAnimationClip.dispatchEvent(new Event("change"));motionAvatarSpeed.value="0.5";');
  check('main selector updates the animation editor', await js('return motionClip.value==="second"&&window.__morphforge.creative.clip.name==="第二个片段"'));
  await click('#publish');
  await wait(async () => (await api('state')).connection.motion.clipId === 'second');
  check('changing selection publishes that clip and its speed', (await api('state')).connection.motion.speed === 0.5);
  await wait(() => js('return !publish.disabled'));

  const beforeDirty = await api('state');
  await js('motionName.value="未保存修改";motionName.dispatchEvent(new Event("input"));');
  await click('#publish');
  await wait(() => js('return !publish.disabled'));
  check('unsaved edits are explained before any publication', await js('return window.__publishAuditRequests.length===2&&publishAnimationHint.textContent.includes("未保存")&&toast.textContent.includes("先保存")'));
  check('unsaved edits preserve the active animation and commit history', (await api('state')).commits.length === beforeDirty.commits.length && (await api('state')).connection.motion.clipId === 'second');

  await js('publishAnimationClip.value="spin";publishAnimationClip.dispatchEvent(new Event("change"));');
  await api('vrc/profile', { action: 'clear' });
  const beforeUnsupported = await api('state');
  await click('#publish');
  await wait(() => js('return window.__publishAuditRequests.length===3&&!publish.disabled'));
  check('unsupported target rejects the combined request without a static fallback', (await api('state')).commits.length === beforeUnsupported.commits.length && await js('return window.__publishAuditRequests.at(-1).status===400'));

  await js('publishAnimationClip.value="";publishAnimationClip.dispatchEvent(new Event("change"));');
  await click('#publish');
  await wait(() => js('return window.__publishAuditRequests.length===4&&!publish.disabled'));
  check('explicit static choice still publishes geometry', await js('return window.__publishAuditRequests.at(-1).body.animation===null&&window.__publishAuditRequests.at(-1).status===200'));
  check('no unhandled browser errors or OSC sends', await js('return window.__publishAuditErrors.length===0') && !(await api('state')).connection.enabled && (await api('state')).connection.sentCommands === 0);
  report = { success: true, checks, scope: 'Actual main publish buttons in isolated Chromium/WebView2; OSC disabled, no VRChat, microphone or model API calls.' };
} catch (e) {
  report = { success: false, checks, error: String(e) }; process.exitCode = 1;
  try { await screenshot('failure.png'); } catch {}
} finally {
  socket?.close(); fs.writeFileSync(path.join(out, 'validation.json'), JSON.stringify(report, null, 2) + '\n');
}
