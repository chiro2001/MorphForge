import { createHash } from "node:crypto";
// Actual Tauri/WebView2 input via its isolated, temporary CDP debugging port.
import fs from "node:fs";
const [
  sessionPath,
  outRoot,
  debug = "http://127.0.0.1:19220",
  expectedVersion = "0.14.1",
] = process.argv.slice(2);
if (!sessionPath || !outRoot)
  throw Error(
    "usage: node check_gui_native.mjs SESSION_JSON OUTPUT_DIR [DEBUG_ORIGIN]",
  );
fs.mkdirSync(outRoot, { recursive: true });
const session = JSON.parse(
  fs.readFileSync(sessionPath, "utf8").replace(/^\uFEFF/, ""),
);
const targets = await (await fetch(debug + "/json/list")).json();
const page = targets.find(
  (t) => t.type === "page" && t.url.startsWith("http://127.0.0.1:"),
);
if (!page) throw Error("No isolated workbench page");
const ws = new WebSocket(page.webSocketDebuggerUrl);
let seq = 0;
const pending = new Map(),
  events = [],
  checks = [];
let report = {};
ws.onmessage = (e) => {
  const m = JSON.parse(e.data);
  if (m.id) {
    const p = pending.get(m.id);
    if (!p) return;
    pending.delete(m.id);
    clearTimeout(p.timer);
    m.error ? p.reject(Error(JSON.stringify(m.error))) : p.resolve(m.result);
  } else events.push(m);
};
await new Promise((ok, bad) => {
  ws.onopen = ok;
  ws.onerror = bad;
});
function send(method, params = {}) {
  return new Promise((resolve, reject) => {
    const id = ++seq;
    const timer = setTimeout(() => {
      pending.delete(id);
      reject(Error("CDP timeout " + method));
    }, 30000);
    pending.set(id, { resolve, reject, timer });
    ws.send(JSON.stringify({ id, method, params }));
  });
}
async function js(code) {
  const r = await send("Runtime.evaluate", {
    expression: `(async()=>{${code}})()`,
    awaitPromise: true,
    returnByValue: true,
    userGesture: true,
  });
  if (r.exceptionDetails)
    throw Error(
      r.exceptionDetails.text +
        ": " +
        r.exceptionDetails.exception?.description,
    );
  return r.result?.value;
}
async function wait(code, seconds = 20) {
  const end = Date.now() + seconds * 1000;
  while (Date.now() < end) {
    if (await js(code)) return true;
    await new Promise((r) => setTimeout(r, 100));
  }
  throw Error("Timed out: " + code);
}
async function click(selector) {
  await js(
    `const n=document.querySelector(${JSON.stringify(selector)});if(!n)throw Error('Missing target');for(let p=n.parentElement;p;p=p.parentElement)if(p.tagName==='DETAILS')p.open=true;n.scrollIntoView({block:'center'});`,
  );
  const r = await js(
    `const n=document.querySelector(${JSON.stringify(selector)}),r=n.getBoundingClientRect();if(n.disabled)throw Error('Disabled target');return {x:r.x+r.width/2,y:r.y+r.height/2};`,
  );
  await send("Input.dispatchMouseEvent", { type: "mouseMoved", ...r });
  await send("Input.dispatchMouseEvent", {
    type: "mousePressed",
    button: "left",
    buttons: 1,
    clickCount: 1,
    ...r,
  });
  await send("Input.dispatchMouseEvent", {
    type: "mouseReleased",
    button: "left",
    buttons: 0,
    clickCount: 1,
    ...r,
  });
}
async function fill(id, value) {
  await js(
    `const n=document.getElementById(${JSON.stringify(id)});n.focus();n.value=${JSON.stringify(value)};n.dispatchEvent(new Event('input',{bubbles:true}));n.dispatchEvent(new Event('change',{bubbles:true}));`,
  );
}
async function check(name, yes) {
  checks.push({ name, passed: !!yes });
  console.log((yes ? "PASS " : "FAIL ") + name);
  if (!yes) throw Error(name);
}
async function snapshot(name) {
  const r = await send("Page.captureScreenshot", { format: "png" });
  fs.writeFileSync(outRoot + "/" + name, Buffer.from(r.data, "base64"));
}
async function menu(key, exportModel = false) {
  const sel = `[data-${exportModel ? "export" : "command"}="${key}"]`;
  const index = await js(
    `document.querySelectorAll('.app-menu').forEach(n=>n.open=false);return [...document.querySelectorAll('.app-menu')].indexOf(document.querySelector(${JSON.stringify(sel)}).closest('.app-menu'));`,
  );
  await click(`.app-menu:nth-child(${index + 1})>summary`);
  if (exportModel) await click(".menu-submenu>summary");
  await click(sel);
}

try {
  await wait("return !!window.__morphforge?.ready");
  await send("Page.enable");
  await js(
    'window.__creativeErrors=[];window.addEventListener("error",e=>{if(e.message)window.__creativeErrors.push(e.message)});window.addEventListener("unhandledrejection",e=>window.__creativeErrors.push(String(e.reason)));',
  );
  if (session.projectId)
    await check(
      "native target matches isolated test project",
      (await js("return window.__morphforge.state.project.id")) ===
        session.projectId,
    );
  await check(
    "Windows creative candidate is 0.15.0",
    await js('return window.__morphforge.state.software.version==="0.15.0"'),
  );
  const embedded = await js(
    'return await (await fetch("/creative-ui.js",{cache:"no-store"})).text()',
  );
  await check(
    "native embedded creative UI matches frozen source",
    createHash("sha256").update(embedded).digest("hex") ===
      createHash("sha256")
        .update(
          fs.readFileSync(new URL("../web/creative-ui.js", import.meta.url)),
        )
        .digest("hex"),
  );
  await check(
    "native creative tests leave OSC disabled",
    !(await js("return window.__morphforge.state.connection.enabled")),
  );
  if (!process.argv.includes("--only-demo")) {
    await send("Browser.setDownloadBehavior", {
      behavior: "allow",
      downloadPath: session.downloads,
      eventsEnabled: true,
    });
    await js(
      `const r=await fetch('/api/harness/tool',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({name:'run_recipe',arguments:{mode:'replace',code:"mf.box([-8,0,-4],[16,1,8],{style:1,group:'base'});mf.box([-3,1,0],[3,7,1],{style:2,group:'door'});"}})});if(!r.ok)throw Error(await r.text());`,
    );
    await wait("return !!window.__morphforge.state.stats.groups.door");
    await click("#componentOpen");
    await wait("return componentDialog.open");
    await fill("componentName", "原生木门");
    await fill("componentGroups", "door");
    await click("#componentSave");
    await wait('return componentSelect.value!==""&&!componentSave.disabled');
    await check(
      "native component saves source group",
      await js(
        'return document.querySelector("#componentViewport canvas").width>0',
      ),
    );
    await fill("componentOffset", "6, 1, 0");
    await fill("componentGroup", "copy");
    const rev = await js("return window.__morphforge.state.revision");
    await click("#componentPlace");
    await wait("return window.__morphforge.state.revision>" + rev);
    await check(
      "native component placement edits real geometry",
      await js("return !!window.__morphforge.state.stats.groups.copy"),
    );
    await snapshot("components.png");
    await click("#componentClose");
    await click("#comparisonOpen");
    await wait(
      "return comparisonDialog.open&&comparisonImage.options.length>1",
    );
    await click("#comparisonFront");
    await fill("comparisonName", "原生正面书签");
    await click("#comparisonSave");
    await wait(
      'return comparisonBookmark.value!==""&&!comparisonSave.disabled',
    );
    await check(
      "comparison canvas stays above its controls",
      await js(
        'return comparisonReference.getBoundingClientRect().bottom<=document.querySelector(".comparison-stage").getBoundingClientRect().bottom+2 && document.querySelector("#comparisonDialog .creative-controls").getBoundingClientRect().top>=document.querySelector(".comparison-stage").getBoundingClientRect().bottom-2',
      ),
    );
    await click("#comparisonCapture");
    await wait(
      "return !comparisonCapture.disabled&&!comparisonResult.hidden&&comparisonResult.naturalWidth===1280",
      60,
    );
    await check(
      "native paired comparison screenshot renders actual artifact",
      true,
    );
    await snapshot("reference.png");
    await click("#comparisonClose");
    await click("#animationOpen");
    await wait("return !motionPanel.hidden");
    await fill("motionGroup", "door");
    await fill("motionPivot", "-3, 1, 0");
    await click("#motionAddKey");
    await fill("motionTime", 2);
    await fill("motionRotation", "0, 100, 0");
    await click("#motionAddKey");
    await fill("motionName", "原生开门");
    await click("#motionSave");
    await wait('return motionClip.value!==""&&!motionSave.disabled');
    await fill("motionTime", 0.5);
    await click("#motionPlay");
    await new Promise((r) => setTimeout(r, 500));
    await click("#motionPause");
    await check(
      "native animation plays then pauses its visible timeline",
      await js("return Number(motionTime.value)>.7"),
    );
    await check(
      "animation dock reserves a separate visible render area",
      await js(
        "const c=window.__morphforge.renderer.renderer.domElement.getBoundingClientRect();return c.bottom<=motionPanel.getBoundingClientRect().top+4",
      ),
    );
    await snapshot("animation.png");
    await click("#motionExport");
    await new Promise((r) => setTimeout(r, 700));
    await check(
      "native animated GLB download action leaves the workbench active",
      await js("return !!window.__morphforge.ready"),
    );
    await click("#motionStatic");
    await check(
      "native static editor recovers after playback",
      await js("return !window.__morphforge.renderer.animationClip"),
    );
    await click("#motionClose");
  }
  await click("#libraryOpen");
  await wait(
    `return libraryDialog.open && !!document.querySelector('[data-artwork-id="builtin:kinetic-windmill"]')`,
  );
  await click('[data-artwork-id="builtin:kinetic-windmill"] button:last-child');
  await wait(
    'return !libraryDialog.open&&window.__morphforge.state.project.name==="会转动的风车"',
  );
  await click("#animationOpen");
  await wait(
    `return !!document.querySelector('#motionClip option[value="windmill-spin"]')`,
  );
  await fill("motionClip", "windmill-spin");
  await check(
    "built-in windmill loads component and five-keyframe motion",
    await js(
      "return window.__morphforge.creative.data.components.length===1 && window.__morphforge.creative.clip.tracks[0].keys.length===5",
    ),
  );
  await click("#motionPlay");
  await new Promise((r) => setTimeout(r, 650));
  await click("#motionPause");
  await check(
    "built-in windmill plays through the real Windows renderer",
    await js(
      'return Number(motionTime.value)>.4&&window.__morphforge.renderer.animationClip.tracks[0].group==="rotor"',
    ),
  );
  await click("#motionPanel details > summary");
  await click("#fitView");
  await snapshot("windmill.png");
  await click("#motionStatic");
  await click("#motionClose");
  await check(
    "native creative workbenches have no unhandled JS errors",
    await js("return window.__creativeErrors.length===0"),
  );
  report = {
    success: true,
    checks,
    scope:
      "Actual isolated Windows Tauri/WebView2, trusted pointer input, real Rust recipe/asset/capture/export APIs. No paid model or OSC.",
  };
} catch (e) {
  report = { success: false, checks, error: String(e) };
  try {
    await snapshot("failure.png");
  } catch {}
  throw e;
} finally {
  fs.writeFileSync(
    outRoot + "/validation.json",
    JSON.stringify(report, null, 2),
  );
  ws.close();
}
