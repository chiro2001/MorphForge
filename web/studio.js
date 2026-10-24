import {t as uiText, bindText, onLanguageChange, diagnosticMessage} from "./i18n.js";
import {setupComposerSpeech,setupSpeechSettings} from "/speech-ui.js";
import {setupTextPanelUi} from "/text-panel-ui.js";
import {setupDrawingUi} from "/drawing-ui.js";
import { setupCreativeUi } from "/creative-ui.js";
import { composeCapture } from "/comparison.js";
import { createFeedback } from "/feedback-ui.js";
import { ActivityView } from "/activity-ui.js";
import { setupDesktopMenus } from "/desktop-menu.js";
import { TimelineView } from "/timeline-ui.js";
import { ReferencePicker } from "/reference-ui.js";
import { usageDisplay, contextUsageDisplay, supportsFileUpload } from "/usage-ui.js";
import { VoxelRenderer } from "/renderer.js";
import { setupMinecraftUi } from "/minecraft-ui.js";
import { setupMeshUi } from "/mesh-ui.js";
import {
  groupRows,
  isGroupProtected,
  normalizeProtectedGroups,
  protectionSavePayload,
  setProtectedGroup,
} from "/group-ui.js";
import {
  conflictFromError,
  completedRunSummary,
  dedupeMessages,
  eventMessageState,
  findExactAssistantMessage,
  friendlyError,
  mergeAck,
  messageModeHint,
  normalizeHarness,
  phaseLabel,
  primaryAction,
  requestIdFromEvent,
  statusLabel,
} from "/harness-ui.js";
import {
  buildTextRequest,
  formatPreviewError,
  normalizeApplyResult,
  previewBoxes,
  previewCanApply,
  previewMetrics,
  textRequestFingerprint,
} from "/text-ui.js";

const $ = (id) => document.getElementById(id);
const captureMode = new URLSearchParams(location.search).has("capture");
const renderer = new VoxelRenderer($("viewport"), {
  interactive: !captureMode,
});

let state = null;
let sceneData = null;
let loadedRevision = -1;
let selectedStyle = 2;
let tool = "box";
let manualEditing=false;
let polling = false;
let edits = Promise.resolve();
let connectionQueue = Promise.resolve();
let connectionPending = 0;
let connectionGeneration = 0;
let acceptedConnection = null;
let lastSeq = 0;
let eventRows = [];
let timelineView = null;
let activityView = null;
let creativeUi = null;
let textPanelUi=null;
let drawingUi=null;

let pointerStart = null;
let loading = false;
let selected = new Set();
let uiScope = "";
let pendingMessages = [];
let chatEntries = [];
let previousHarness = null;
let messageInFlight = false;
let messagePreparationInFlight = false;
let retryPayload = null;
let fontsLoaded = false;
let fontCatalog = [];
let previewSequence = 0;
let previewTimer = 0;
let currentTextPreview = null;
let currentPreviewSentJSON = "";
let currentPreviewServerFingerprint = "";
let currentPreviewRequestId = "";
let currentPreviewRequest = null;
let textApplyBusy = false;
let groupSettingsBusy = false;
let providerDraftDirty = false;
let providerSaveBusy = false;
let workspaceBusy = false;
let loadedProjectId = "";
let loadedPreviewCatalog = "";
let viewSettings = {quality:"high",captureQuality:"high",voxelGrid:false,captureGrid:false,surfaceCulling:true,showTargetBounds:false};
let pendingTransform = null;

window.__morphforge = {
  ready: false,
  state: null,
  renderer,
  projectPoint: (point) => renderer.projectPoint(point),
  harnessUi: {
    pending: () => structuredClone(pendingMessages),
    chat: () => structuredClone(chatEntries),
    send: (message) => postHarnessMessage(message),
  },
  text: {
    preview: () => currentTextPreview,
    fingerprint: () => currentPreviewServerFingerprint,
    previewRequestId: () => currentPreviewRequestId,
    request: () =>
      currentPreviewRequest ? structuredClone(currentPreviewRequest) : null,
    refresh: () => requestTextPreview(true),
  },
};

async function api(path, body) {
  const response = await fetch(
    "/api/" + path,
    body === undefined
      ? {}
      : {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(body),
        },
  );
  const text = await response.text();
  let value;
  try {
    value = text ? JSON.parse(text) : {};
  } catch {
    value = {
      ok: false,
      error: response.ok
        ? uiText("本机服务返回了无法识别的响应")
        : uiText("本机服务暂时不可用（{0}）", response.status),
    };
  }
  if (!response.ok) {
    const detail =
      typeof value.error === "string"
        ? value.error
        : value.error
          ? JSON.stringify(value.error)
          : uiText("请求失败（{0}）", response.status);
    const error = new Error(detail);
    error.status = response.status;
    error.payload = value;
    error.code = value.code || value.error?.code || "";
    throw error;
  }
  return value;
}

const toast = createFeedback($("toast"));

function error(value) {
  toast(friendlyError(value), "error");
  console.error(value);
}

function fmt(number) {
  return Number(number || 0).toLocaleString();
}

function time(seconds) {
  if (!seconds) return uiText("0秒");
  if (seconds < 60) return Math.ceil(seconds) + uiText("秒");
  if (seconds < 3600)
    return Math.floor(seconds / 60) + uiText("分") + Math.ceil(seconds % 60) + uiText("秒");
  return (
    Math.floor(seconds / 3600) + uiText("时") + Math.ceil((seconds % 3600) / 60) + uiText("分")
  );
}

function setValue(id, value) {
  const element = $(id);
  if (element && document.activeElement !== element) element.value = value;
}

function setChecked(id, value) {
  const element = $(id);
  if (element && document.activeElement !== element) element.checked = value;
}

function values(ids) {
  return ids.map((id) => Math.floor(Number($(id).value) || 0));
}

function brush() {
  return values(["brushX", "brushY", "brushZ"]).map((value) =>
    Math.max(1, value),
  );
}

function coord() {
  return values(["coordX", "coordY", "coordZ"]);
}

function operation(min, erase = false) {
  return {
    op: erase ? "erase" : tool === "erase" ? "erase" : "box",
    min,
    size: brush(),
    styleId: selectedStyle,
    tintId: Number($("tintId").value) || 0,
    group: $("groupName").value || "main",
  };
}

function safeStorageGet(key, fallback) {
  try {
    const value = localStorage.getItem(key);
    return value == null ? fallback : JSON.parse(value);
  } catch {
    return fallback;
  }
}

function safeStorageSet(key, value) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // The app remains usable when storage is unavailable or full.
  }
}

function switchUiScope(projectId, activeReferences = [], sessionId = "") {
  const next = (projectId || "default") + (sessionId ? ":" + sessionId : "");
  if (uiScope === next) return;
  uiScope = next;
  previousHarness = null;
  if(activityView){activityView.reset();activityView.load().catch(error);}
  referencePicker.setScope(uiScope, activeReferences);
  pendingMessages = dedupeMessages(
    safeStorageGet(`morphforge.pending.${uiScope}`, []),
  );
  chatEntries = safeStorageGet(`morphforge.chat.${uiScope}`, []);
  if (!Array.isArray(chatEntries)) chatEntries = [];
  renderPending();
  renderChat();
}

function savePending() {
  const keep = pendingMessages
    .filter((message) => !["applied", "cancelled"].includes(message.status))
    .slice(-50);
  const terminal = pendingMessages
    .filter((message) =>
      ["applied", "cancelled", "rejected", "failed"].includes(message.status),
    )
    .slice(-20);
  pendingMessages = dedupeMessages([...keep, ...terminal]).slice(-70);
  safeStorageSet(`morphforge.pending.${uiScope}`, pendingMessages);
}

function saveChat() {
  chatEntries = chatEntries.slice(-500);
  safeStorageSet(`morphforge.chat.${uiScope}`, chatEntries);
}

function upsertChat(entry) {
  if (!entry?.id) return false;
  const existing = chatEntries.find((item) => item.id === entry.id);
  if (existing) {
    const changed =
      existing.text !== entry.text ||
      existing.interrupted !== entry.interrupted ||
      existing.note !== entry.note;
    Object.assign(existing, entry);
    if (changed) {
      saveChat();
      renderChat();
    }
    return false;
  }
  chatEntries.push(entry);
  saveChat();
  renderChat();
  return true;
}

function renderChat() {
  const chat = $("chat");
  const nearBottom =
    chat.scrollHeight - chat.scrollTop - chat.clientHeight < 44;
  chat.querySelector(".chat-placeholder")?.remove();
  const existing = new Map(
    [...chat.querySelectorAll("[data-chat-id]")].map((element) => [
      element.dataset.chatId,
      element,
    ]),
  );
  for (const entry of chatEntries) {
    let element = existing.get(entry.id);
    if (!element) {
      element = document.createElement("div");
      element.dataset.chatId = entry.id;
      const label = document.createElement("small");
      const body = document.createElement("span");
      const note = document.createElement("span");
      note.className = "entry-note";
      element.append(label, body, note);
      chat.append(element);
    }
    element.className = `chat-entry ${entry.role || "assistant"}${
      entry.interrupted ? " interrupted" : ""
    }`;
    element.dataset.run = entry.runId || "";
    element.dataset.epoch = String(entry.epoch ?? 0);
    bindText(element.querySelector("small"), () => (entry.role === "user" ? entry.label || uiText("你") : "MorphForge"));
    const body = element.querySelector("span:not(.entry-note)");
    if (body.textContent !== (entry.text || ""))
      bindText(body, () => (entry.text || ""));
    const note = element.querySelector(".entry-note");
    bindText(note, () => (entry.note || ""));
    note.hidden = !entry.note;
  }
  const valid = new Set(chatEntries.map((entry) => entry.id));
  for (const [id, element] of existing) if (!valid.has(id)) element.remove();
  if (nearBottom) chat.scrollTop = chat.scrollHeight;
}

function addUserChat(entry) {
  if (chatEntries.some((item) => item.id === entry.id)) return false;
  return upsertChat({
    role: "user",
    label:
      entry.mode === "queue"
        ? uiText("你 · 排队")
        : entry.mode === "interrupt"
          ? uiText("你 · 打断后")
          : uiText("你"),
    ...entry,
  });
}

function markAssistantInterrupted(runId, epoch) {
  const key = `assistant:${runId}:${epoch}`;
  let entry = chatEntries.find((item) => item.id === key);
  if (!entry) {
    entry = {
      id: key,
      role: "assistant",
      text: "",
      runId,
      epoch,
      interrupted: true,
      note: uiText("当前请求尚未产生可见文本，已在生成前被取消。"),
    };
    chatEntries.push(entry);
    saveChat();
    renderChat();
    return;
  }
  entry.interrupted = true;
  entry.note = entry.text
    ? uiText("该段输出在安全边界前被取消。")
    : uiText("当前请求尚未产生可见文本。");
  saveChat();
  renderChat();
}

function addCompletedSummary(summary) {
  if (!summary?.message?.trim() || !summary.runId) return false;
  const exact = findExactAssistantMessage(chatEntries, summary);
  if (exact) {
    if (!exact.final) {
      exact.final = true;
      exact.note = exact.note || uiText("完成总结");
      saveChat();
      renderChat();
    }
    return false;
  }
  const id = `final:${summary.runId}:${summary.epoch}`;
  const existing = chatEntries.find((entry) => entry.id === id);
  if (existing) {
    upsertChat({
      ...existing,
      text: summary.message,
      final: true,
      note: uiText("完成总结"),
    });
    return false;
  }
  return upsertChat({
    id,
    role: "assistant",
    text: summary.message,
    runId: summary.runId,
    epoch: summary.epoch,
    final: true,
    interrupted: false,
    note: uiText("完成总结"),
  });
}

function renderPending() {
  pendingMessages = dedupeMessages(pendingMessages);
  bindText($("pendingCount"), () => (String(pendingMessages.length)));
  bindText($("composerOptionSummary"), () => (pendingMessages.length?uiText(" · 队列{0}", pendingMessages.length):""));
  $("pendingCount").classList.toggle(
    "pending",
    pendingMessages.some((message) =>
      ["sending", "accepted"].includes(message.status),
    ),
  );
  const list = $("pendingMessages");
  list.replaceChildren();
  if (!pendingMessages.length) {
    const empty = document.createElement("div");
    empty.className = "pending-empty";
    bindText(empty, () => (uiText("没有待处理消息")));
    list.append(empty);
    return;
  }
  for (const message of pendingMessages.slice().reverse()) {
    const row = document.createElement("div");
    row.className = `pending-item ${message.status}`;
    row.dataset.requestId = message.requestId;
    const prompt = document.createElement("span");
    prompt.className = "pending-prompt";
    bindText(prompt, () => (message.prompt || uiText("暂停并取消当前请求")));
    bindText(prompt, () => (message.prompt || ""), "title");
    const detail = document.createElement("span");
    detail.className = "pending-detail";
    const mode = document.createElement("span");
    mode.className = "caption";
    bindText(mode, () => ({ start: uiText("开始"), steer: uiText("插话"), interrupt: uiText("打断"), queue: uiText("排队") }[
        message.mode
      ] || message.mode));
    const status = document.createElement("span");
    status.className = `message-state ${message.status}`;
    bindText(status, () => (statusLabel(message.status) +
      (message.status === "accepted" && message.queuePosition
        ? ` · #${message.queuePosition}`
        : "")));
    detail.append(mode, status);
    row.append(prompt, detail);
    if (message.error) {
      const failure = document.createElement("span");
      failure.className = "pending-detail caption";
      bindText(failure, () => (message.error));
      row.append(failure);
    }
    list.append(row);
  }
}

function updatePending(requestId, patch) {
  const index = pendingMessages.findIndex(
    (message) => message.requestId === requestId,
  );
  if (index < 0) return null;
  const currentStatus = pendingMessages[index].status;
  if (
    (currentStatus === "applied" && patch.status !== "applied") ||
    (currentStatus === "cancelled" && patch.status !== "cancelled")
  ) {
    return pendingMessages[index];
  }
  pendingMessages[index] = { ...pendingMessages[index], ...patch };
  savePending();
  renderPending();
  return pendingMessages[index];
}

function applyHarnessEvents(events) {
  for (const event of events || []) {
    const completed = completedRunSummary(event);
    if (completed) {
      addCompletedSummary(completed);
      continue;
    }
    const requestId = requestIdFromEvent(event);
    const next = eventMessageState(event.kind);
    if (!requestId || !next) continue;
    const previous = pendingMessages.find(
      (message) => message.requestId === requestId,
    );
    if (
      previous?.mode === "interrupt" &&
      Number(event.data?.epoch || 0) > Number(previous.epoch || 0)
    ) {
      markAssistantInterrupted(previous.runId, previous.epoch);
    }
    const pending = updatePending(requestId, {
      status: next,
      runId:
        event.data?.runId ??
        pendingMessages.find((x) => x.requestId === requestId)?.runId,
      epoch:
        event.data?.epoch ??
        pendingMessages.find((x) => x.requestId === requestId)?.epoch,
      error: event.data?.error || "",
    });
    if (
      pending?.prompt &&
      !["applied", "cancelled"].includes(previous?.status) &&
      ["accepted", "applied"].includes(next) &&
      pending.mode !== "start"
    ) {
      addUserChat({
        id: `message:${pending.requestId}`,
        text: pending.prompt,
        runId: pending.runId,
        epoch: pending.epoch,
        mode: pending.mode,
        note: next === "applied" ? uiText("已应用到当前运行") : uiText("已接收，等待安全点"),
      });
    }
  }
}

function syncHarness(h) {
  const snapshot = normalizeHarness(h);
  const run = snapshot.run;
  if (run) {
    upsertChat({
      id: `initial:${run.id}`,
      role: "user",
      text: run.prompt || "",
      runId: run.id,
      epoch: 0,
      note: "",
    });
    const key = `assistant:${run.id}:${snapshot.epoch}`;
    if (run.message) {
      if (snapshot.status === "completed" || snapshot.phase === "completed") {
        addCompletedSummary({
          runId: run.id,
          epoch: snapshot.epoch,
          message: run.message,
        });
      } else {
        const existing = chatEntries.find((entry) => entry.id === key);
        upsertChat({
          id: key,
          role: "assistant",
          text: run.message,
          runId: run.id,
          epoch: snapshot.epoch,
          interrupted: existing?.interrupted || false,
          note: existing?.note || "",
        });
      }
    }
    if (snapshot.phase === "stopped" || snapshot.phase === "cancelled") {
      markAssistantInterrupted(run.id, snapshot.epoch);
    }
  }
  previousHarness = snapshot;
  updateHarnessControls(snapshot);
}

function updateHarnessControls(snapshot) {
  const mode = $("messageMode").value;
  const action = primaryAction(snapshot, mode);
  bindText($("sendMessage"), () => (action.label));
  $("sendMessage").disabled =
    messageInFlight ||
    referencePicker.busy ||
    (snapshot.stopping && !snapshot.active);
  bindText($("messageModeHint"), () => (messageModeHint(snapshot, mode)));
  $("pauseAI").disabled = !snapshot.active || snapshot.paused;
  $("resumeAI").disabled = !snapshot.active || !snapshot.paused;
  $("stopAI").disabled = !snapshot.active;
  const run = snapshot.run;
  const status = [uiText("状态：{0}", phaseLabel(snapshot))];
  if (snapshot.queue.length) status.push(uiText("排队 {0}", snapshot.queue.length));
  if (run) {
    status.push(uiText("{0} 步 · {1} Token", run.steps || 0, fmt(run.tokens)));
    if (run.error) status.push(friendlyError({ message: run.error }));
  }
  bindText($("runStatus"), () => (status.join("\n")));
  bindText($("viewportRunStatus"), () => (status.join(" · ")));
}

async function refresh(force = false) {
  if (polling) return;
  polling = true;
  const connectionAtStart=connectionGeneration;
  try {
    const next = await api("state");
    // A state read may have started before an OSC configuration write. Never
    // let that old snapshot overwrite a newly accepted setting or draft.
    if(connectionAtStart!==connectionGeneration)next.connection=acceptedConnection||state?.connection||next.connection;
    else if(!connectionPending)acceptedConnection=next.connection;
    state = next;
    window.__morphforge.state = next;
    switchUiScope(next.project?.id, next.harness?.context?.referenceImages, next.harness?.context?.id);
    if (loadedProjectId !== next.project?.id) {
      loadedProjectId = next.project?.id;
      loadedRevision = -1;
      selected.clear();
      renderer.groupsHidden.clear();
      clearTextPreview();
      if (timelineView && !$("timeline").hidden) timelineView.reload().catch(error);
    }
    updateState();
    drawingUi?.sync();
    const previewCatalog=JSON.stringify([next.connection.profile,next.materialCompatibility?.fingerprint,$("renderMode").value]);
    const previewScale=$("renderMode").value === "vrc" ? next.connection.placement.scale : 1;
    if(renderer.mode===$("renderMode").value && renderer.scale!==previewScale) renderer.setScale(previewScale);
    if ((next.revision !== loadedRevision || previewCatalog!==loadedPreviewCatalog || force) && !loading) {
      loading = true;
      try {
        sceneData = await api("scene");
        if (
          !sceneData.materials.some((material) => material.id === selectedStyle)
        ) {
          selectedStyle = sceneData.materials[0]?.id ?? 1;
        }
        await renderer.load(sceneData.scene, sceneData.materials, {
          surfaces: sceneData.surfaces,
          textLayouts:sceneData.textLayouts,
          mode: $("renderMode").value,
          scale:
            $("renderMode").value === "vrc"
              ? state.connection.placement.scale
              : 1,
        });
        textPanelUi?.sync(sceneData);
        loadedRevision = sceneData.scene.revision;
        loadedPreviewCatalog=previewCatalog;
        selected = new Set(
          [...selected].filter((id) =>
            sceneData.scene.boxes.some((box) => box.id === id) || (sceneData.scene.triangles||[]).some(t=>t.id===id),
          ),
        );
        renderer.selection = selected;
        renderer.updateSelection();
        drawPalette();
        drawGroups();
        selectionInfo();
      } finally {
        loading = false;
      }
    }
    if(creativeUi)await creativeUi.sync(next);
    window.__morphforge.ready = true;
  } catch (cause) {
    bindText($("saveState"), () => (uiText("后端暂时不可用")));
    if (!state) error(cause);
  } finally {
    polling = false;
  }
}

function updateState() {
  const s = state;
  const connection = s.connection;
  const harness = s.harness;
  setValue("projectName", s.project.name);
  setValue("modelingStyle", s.project.modelingStyle||"auto");
  const extent=s.project.modelingExtent;
  const extentPreset=!extent?"auto":extent.every(n=>n===extent[0])&&[32,64,128].includes(extent[0])?String(extent[0]):"custom";
  setValue("modelingExtentPreset",extentPreset);
  $("modelingExtentCustom").hidden=extentPreset!=="custom";
  if(extent)["X","Y","Z"].forEach((axis,i)=>setValue("modelingExtent"+axis,extent[i]));
  bindText($("activeSessionTitle"), () => (harness.context.title || uiText("对话")));
  $("schemaUpgradeNotice").hidden=!harness.context.schemaUpgradeRequired;
  $("schemaUpgradeContext").hidden=!harness.context.schemaUpgradeRequired;
  bindText($("saveState"), () => (uiText("已保存 · v") + s.revision));
  bindText($("boxCount"), () => (fmt(s.stats.boxCount) + " Box · " + fmt(s.stats.triangleCount||0) + uiText(" 面 · ")+fmt(s.stats.textPanelCount||0)+uiText(" 文本框 / 目标 ") + fmt(s.stats.targetBoxes)));
  const usedIds=new Set((sceneData?.scene.boxes||[]).map(b=>b.styleId));
  const usedStyles=(sceneData?.materials||[]).filter(m=>usedIds.has(m.id));
  bindText($("usedMaterialCount"), () => (String(usedStyles.length)));
  const materialSignature=JSON.stringify(usedStyles.map(m=>[m.id,m.color,m.name]));
  if($("usedMaterialList").dataset.signature!==materialSignature){
    $("usedMaterialList").dataset.signature=materialSignature;$("usedMaterialList").replaceChildren();
    for(const m of usedStyles){const n=document.createElement("span"),swatch=document.createElement("i");n.className="used-material";swatch.style.background=`rgb(${(m.color||[.5,.5,.5]).map(v=>Math.round(v*255)).join(",")})`;n.append(swatch,document.createTextNode(m.name||String(m.id)));$("usedMaterialList").append(n);}
  }
  bindText($("voxelCount"), () => (fmt(s.stats.voxelCount) + uiText(" 格")));
  bindText($("bounds"), () => (s.stats.bounds
    ? uiText("范围 ") +
      s.stats.bounds.min.join(",") +
      " → " +
      s.stats.bounds.max.join(",")
    : uiText("范围为空")));
  $("emptyHint").hidden = s.stats.boxCount > 0 || s.stats.triangleCount > 0 || s.stats.textPanelCount>0;
  const target=s.targetCompatibility;
  bindText($("vrcBoundsStatus"), () => (target ? uiText("{0} · {1} 个越界对象\n作品尺寸 {2} 格{3}", target.fits?uiText("坐标/容量可发布"):uiText("目标预检未通过"), target.outOfBoundsCount, target.extent.join(" × "), target.outOfBoundsGroups.length?uiText("\n越界分组：")+target.outOfBoundsGroups.join("、"):"") : ""));
  const catalog=s.materialCompatibility;
  bindText($("materialCompatibility"), () => (catalog ? (catalog.status==="unknown"?uiText("未声明目标目录，未核实上传的Avatar。"):catalog.matched?uiText("目录匹配{0} · {1} 种已用材质 · {2}\n用户导入的目录，非远端确认。", catalog.runtimeMaterials?uiText(" / 支持实时材质"):"", catalog.usedStyles, catalog.fingerprint.slice(0,12)):uiText("目录不匹配：缺少 [{0}]，定义不同 [{1}]\n{2}", catalog.missingIds.join(","), catalog.differentIds.join(","), diagnosticMessage(catalog.note)||uiText("请更新Avatar目录后再发布。"))) : ""));
  setChecked("autoCenterOnFinish",s.project.autoCenterOnFinish);
  if(pendingTransform) $("transformApply").disabled=pendingTransform.baseRevision!==s.revision;
  bindText($("renderDiagnostics"), () => (uiText("实际 WebGL 抗锯齿样本：{0} · 内部透明面 {1} 个\n{2}", renderer.renderer.getContext().getParameter(renderer.renderer.getContext().SAMPLES), renderer.surfaceStats?.hiddenFaces||0, renderer.textureWarnings.join("\n"))));
  $("voiceRecordingBadge").hidden=!s.speechStatus?.active;
  $("voiceRecordingBadge").disabled=s.speechStatus?.phase==="finishing";
  $("undo").disabled = !s.canUndo;
  $("redo").disabled = !s.canRedo;
  if(!connectionPending){
  setValue("profile", connection.profile);
  setValue("targetPort", connection.port);
  // Preserve an advanced/API-configured value instead of leaving the select
  // empty and accidentally posting interval=0 on an unrelated settings edit.
  const intervalValue=String(connection.interval),intervalSelect=$("interval");
  for(const option of [...intervalSelect.options])if(option.dataset.custom&&option.value!==intervalValue)option.remove();
  if(![...intervalSelect.options].some(option=>option.value===intervalValue)){
    const option=new Option(uiText("{0}秒（自定义）", connection.interval),intervalValue);option.dataset.custom="true";intervalSelect.add(option);
  }
  setValue("interval", connection.interval);
  setValue("targetCapacity", connection.capacity);
  setValue("refreshSeconds", connection.refreshSeconds);
  setValue("sendOrder",connection.sendOrder||"volume");
  setChecked("loopReplay",connection.loopReplay||false);
  setChecked("autoPublish", connection.auto);
  setChecked("autoDetectAvatar",connection.autoDetect??true);
  setValue("publishStrategy",connection.publishStrategy||"auto");
  }
  const discovered=connection.discovery||{},info=discovered.info;
  bindText($("discoveryStatus"), () => (info?uiText("已读取Avatar协议 v{0} · {1}\n材质指纹 {2} · {3}{4}\n{5}能力查询不确认执行或其他玩家接收。", info.version, info.region8?uiText("连续区域编码可用"):uiText("基础编码"), info.catalogHash.slice(0,16), discovered.catalogKnown?uiText("本机有匹配目录"):uiText("本机缺少此目录，请导入构建包"), discovered.lastError?"\n"+discovered.lastError:"", connection.catalogIdentity===info.catalogHash?uiText("当前目录与Avatar一致。"):uiText("当前目录与Avatar不同，发送前需匹配。")):(discovered.lastError||uiText("等待识别；Unity测试可手动选择配套目录。"))));
  const plan=connection.publication;
  bindText($("publicationPlan"), () => (plan?uiText("{0}方案：{1} · {2}\n增量 {3} 帧 / {4}；重建 {5} 帧 / {6}（含Clear）", plan.layout==="run_boxes"?uiText("同材质段合并，减少{0}个显示对象 · ", plan.compactedBoxes):"", plan.selected==="rebuild"?uiText("清空重建"):uiText("增量"), diagnosticMessage(plan.reason), plan.incrementalFrames??uiText("不可行"), plan.incrementalSeconds==null?"—":time(plan.incrementalSeconds), plan.rebuildFrames??uiText("不可行"), plan.rebuildSeconds==null?"—":time(plan.rebuildSeconds)):uiText("发布时比较真实编码计划；时间为本机节拍估算，不代表远端收到。")));
  $("loopReplay").disabled=connection.profile!=="mf6";
  $("sendOrder").disabled=connection.profile!=="mf6";
  bindText($("replayStatus"), () => (uiText("已完成补播 {0} 轮 · {1}。{2}更新先删除旧几何，再按所选顺序绘制；没有远端ACK。", connection.replayCycles||0, connection.repairing?uiText("正在补播"):connection.loopReplay?(connection.enabled?uiText("连续补播已启用"):uiText("循环已设置，发送关闭")):uiText("按需发送"), connection.repairDeletions?.some(n=>n)?uiText("含 {0} 条历史删除。", connection.repairDeletions.reduce((a,b)=>a+b,0)):"")));
  $("repairPublish").disabled=connection.profile!=="mf6";
  $("connect").disabled=connectionPending>0;
  bindText($("connect"), () => (connection.enabled ? uiText("停止发送") : uiText("开启发送")));
  $("connect").classList.toggle("selected", connection.enabled);
  bindText($("connectionBadge"), () => (connection.enabled
    ? "OSC " + connection.port + " · " + connection.pending + uiText("项")
    : uiText("独立创作")));
  $("connectionBadge").classList.toggle("live", connection.enabled);
  const progress=connection.progress;
  const progressBar=$("syncProgress");
  if(progress?.totalFrames!=null){
    progressBar.value=progress.percent;
    bindText($("syncProgressLabel"), () => (uiText("{0} {1}% · {2} / {3} 帧{4}", connection.repairing?uiText("补播"):uiText("发布"), progress.percent.toFixed(1), fmt(progress.sentFrames), fmt(progress.totalFrames), connection.enabled?"":uiText(" · 发送关闭"))));
  }else{
    if(connection.enabled&&connection.pending)progressBar.removeAttribute("value");else progressBar.value=0;
    bindText($("syncProgressLabel"), () => (connection.pending?uiText("待发 {0} 批{1}", fmt(connection.pending), connection.enabled?"":uiText(" · 发送关闭")):uiText("尚未发布")));
  }
  $("viewportSyncProgress").value=progressBar.value;
  bindText($("viewportSyncLabel"), () => ($("syncProgressLabel").textContent));
  bindText($("viewportETA"), () => (connection.enabled&&connection.pending?uiText("预计剩余 {0} · 本机发送", time(connection.etaSeconds)):""));
  bindText($("syncStatus"), () => (connection.error ||
    (!connection.enabled
      ? uiText("发送已关闭；可继续独立创作。")
      : uiText("本机发包中 · 无远端ACK\n{0} · 几何 {1}bit{2}\n当前目标 v{3} / 最近完整发包 v{4}\n本机已发记录 {5} Box / {6} 面；VRC接收情况未知\n待发 {7} 批 · 预计 {8}\n累计 {9} 批", connection.profile.toUpperCase(), connection.syncedBits, uiText(" · 本体仅本地"), connection.desiredRevision, connection.sentRevision ?? "—", fmt(connection.sentObjects?.boxes??0), fmt(connection.sentObjects?.triangles??0), fmt(connection.pending), time(connection.etaSeconds), fmt(connection.sentCommands)))));
  setChecked("world", connection.placement.world);
  setChecked("visible", connection.placement.visible);
  setChecked("allowGrab",connection.placement.allowGrab!==false);
  setChecked("level", connection.placement.level);
  setValue("scale", Math.log10(connection.placement.scale/.0005));
  const percent=connection.placement.scale/.05*100;
  setValue("scalePercent",Number(percent.toFixed(2)));
  $("scaleValue").value=Number(percent.toFixed(1))+"%";
  $("scale").min="0";
  $("scalePercent").min="1";
  $("scale").max="3";
  $("scalePercent").max="1000";
  bindText($("scaleHint"), () => (uiText("新100%＝旧5% · 当前格长 {0} 毫米。{1}", (0.25*connection.placement.scale*1000).toFixed(3), uiText("1%～1000%，对数刻度，便于微缩。"))));
  for (const id of [
    "preset",
    "contextWindowTokens",
    "goalMinReviewRounds",
    "goalMaxContinuations",
    "detailInstructions",
    "goalCriteria",
  ]) {
    if (harness.settings[id] !== undefined) setValue(id, harness.settings[id]);
  }
  setValue("settingsPreset", harness.settings.preset || "standard");
  bindText($("activeModel"), () => (`${harness.settings.model} · ${harness.provider.apiType === "chat" ? "Chat" : "Responses"}`));
  const usage = usageDisplay(harness.usage);
  bindText($("usageTokens"), () => (usage.tokens));
  bindText($("usageTokens"), () => (usage.tokenTitle), "title");
  bindText($("usagePrefix"), () => (usage.prefix));
  bindText($("usagePrefix"), () => (usage.prefixTitle), "title");
  bindText($("usageRun"), () => (usage.run));
  $("usageRun").hidden = !usage.run;
  const contextUsage=contextUsageDisplay(harness.context.usage);
  bindText($("usageContext"), () => (contextUsage.text));
  bindText($("usageContext"), () => (contextUsage.title), "title");
  $("usageContext").dataset.level=contextUsage.level;
  setChecked("sendImages", harness.settings.sendImages ?? true);
  setChecked("goalMode", harness.settings.goalMode ?? false);
  bindText($("presetHint"), () => (({
      sketch: uiText("轮廓与大形优先"),
      standard: uiText("结构、材质与有限细节"),
      detailed: uiText("分阶段细化与多轮复核"),
    }[harness.settings.preset] || uiText("创作预设")) +
    uiText(" · 当前最多{0}步 / {1}分钟，可在预算范围内调整。", harness.settings.maxSteps, Math.round(harness.settings.maxRunSeconds / 60))));
  const goal = harness.context.goal;
  const goalLabels = {
    active: uiText("进行中"),
    achieved: uiText("已保存完成"),
    blocked: uiText("等待调整"),
    budget_exhausted: uiText("预算已用尽"),
    stopped: uiText("已停止"),
  };
  bindText($("goalStatus"), () => (goal
    ? uiText("{0} · 复核 {1}/{2} · 自动续作 {3}\n{4}\n{5}", goalLabels[goal.status] || goal.status, goal.reviewedGeometryHashes.length, goal.minReviewRounds, goal.continuations, goal.objective, goal.assessment || "")
    : uiText("尚无创作目标")));
  $("goalStatus").hidden = !goal;
  $("saveProvider").disabled =
    normalizeHarness(harness).active || providerSaveBusy;
  if (!providerDraftDirty) {
    const provider = harness.provider;
    for (const [id, key] of [
      ["providerBase", "baseUrl"],
      ["providerApi", "apiType"],
      ["providerImages", "imageDelivery"],
      ["providerTokenField", "chatTokenField"],
    ]) {
      if (provider[key] !== undefined) setValue(id, provider[key]);
    }
    for (const [id, key] of [
      ["providerStream", "stream"],
      ["providerReasoning", "sendReasoning"],
      ["providerRequiresKey", "requiresApiKey"],
    ]) {
      if (provider[key] !== undefined) setChecked(id, provider[key]);
    }
  }
  updateProviderImagePolicy();
  setValue("model", harness.settings.model);
  setValue("reasoning", harness.settings.reasoning);
  setValue("cacheGuard", harness.settings.cacheGuard || "auto");
  setChecked("strictAppendOnly", harness.settings.strictAppendOnly ?? true);
  setChecked("enforceTargetLimits",harness.settings.enforceTargetLimits ?? true);
  document.body.classList.toggle("ai-editing",!!harness.manualEditLocked);
  $("manualEditToggle").disabled=!!harness.manualEditLocked;
  const monitor = harness.cacheMonitor || {};
  bindText($("cacheMonitorStatus"), () => (uiText("前缀 {0} · 最近缓存 {1} · {2}{3}", monitor.prefix?.appendOnly === false ? uiText("发生重置/变化") : monitor.prefix?.appendOnly === true ? uiText("保持追加") : uiText("待观察"), typeof monitor.recentHitRate === "number" ? (monitor.recentHitRate * 100).toFixed(1) + "%" : uiText("未报告"), monitor.alert || (monitor.mode ? (monitor.cacheRequired ? uiText("严格监测") : uiText("观察模式")) : uiText("等待首次请求；保护按配置启用")), monitor.unreportedInterruptions ? uiText(" · {0} 次中断未回报缓存，费用未知，未计入命中率", monitor.unreportedInterruptions) : "")));
  $("cacheMonitorStatus").classList.toggle("kind-error", !!monitor.alert);
  bindText($("cacheLiveStatus"), () => (monitor.alert ? uiText("缓存告警 · 查看设置") : monitor.prefix?.appendOnly===false ? uiText("前缀发生变化") : ""));
  bindText($("cacheLiveStatus"), () => ($("cacheMonitorStatus").textContent), "title");
  $("cacheLiveStatus").classList.toggle("kind-error", !!monitor.alert);
  $("cacheShowConfig").hidden = !monitor.alert;
  setValue("maxSteps", harness.settings.maxSteps);
  setValue("maxRunSeconds", harness.settings.maxRunSeconds);
  setValue("maxRunTokens", harness.settings.maxRunTokens);
  setValue("maxOutputTokens", harness.settings.maxOutputTokens);
  if (harness.modelLimits) {
    const limits = harness.modelLimits;
    bindText($("modelLimitsInfo"), () => (uiText("有效上下文 {0} · 输出上限 {1} Token（{2}，单次输出按剩余窗口裁剪）", fmt(limits.effectiveContextWindow), fmt(limits.effectiveOutputCeiling), limits.source === "upstream_models" ? uiText("已读取上游 /models") : uiText("按当前配置；尚未获取上游限制"))));
  }
  setValue("compactTokens", harness.settings.compactTokens);
  setValue("language", harness.settings.language || "zh-CN");
  setChecked("autoCompact", harness.settings.autoCompact);
  setValue("pinnedNotes", s.project.pinnedNotes);
  bindText($("providerState"), () => (harness.provider.configured
    ? uiText("连接已配置")
    : uiText("需要配置Key")));
  $("providerState").classList.toggle("live", harness.provider.configured);
  bindText($("contextStats"), () => (`${contextUsage.text}\n` +
    uiText("{0} 条消息 · compact {1} 次", harness.context.messages, harness.context.compactCount)));
  bindText($("contextSummary"), () => (harness.context.summary || uiText("暂无")));
  syncHarness(harness);
  bindText($("renderInfo"), () => (`v${loadedRevision < 0 ? s.revision : loadedRevision} · ` +
    uiText("{0} 绘制调用 · 底部中心 / +Z前", renderer.renderer.info.render.calls)));
}

function edit(value) {
  if(state?.harness?.manualEditLocked){toast(uiText("AI正在修改，暂停后再手工编辑"));return Promise.resolve();}
  edits = edits.then(async () => {
    try {
      const result = await api("edit", {
        ...value,
        baseRevision: state?.revision,
        requestId: crypto.randomUUID(),
      });
      if (result.warning) toast(result.warning);
      if (Array.isArray(result.selectionIds)) {
        selected = new Set(result.selectionIds);
        renderer.selection = selected;
      }
      await refresh();
      if (Array.isArray(result.selectionIds)) { renderer.updateSelection(); selectionInfo(); }
    } catch (cause) {
      error(cause);
      await refresh();
    }
  });
  return edits;
}

function drawPalette() {
  if (!sceneData) return;
  const query = $("styleSearch").value.toLowerCase();
  $("palette").replaceChildren();
  for (const material of sceneData.materials) {
    if (
      query &&
      !`${material.name} ${material.uid} ${material.family}`
        .toLowerCase()
        .includes(query)
    ) {
      continue;
    }
    const button = document.createElement("button");
    button.className =
      "swatch" + (material.id === selectedStyle ? " selected" : "");
    button.style.background = `rgb(${material.color
      .map((value) => Math.round(value * 255))
      .join(",")})`;
    bindText(button, () => (material.name +
      " · " +
      material.family +
      (material.pack ? uiText(" · Avatar需预打包") : "")), "title");
    if (material.textures?.length)
      button.style.backgroundImage = `url("${material.textures[0]}")`;
    button.onclick = () => {
      selectedStyle = material.id;
      drawPalette();
      scheduleTextPreview();
    };
    $("palette").append(button);
  }
  const selectedMaterial = sceneData.materials.find(
    (material) => material.id === selectedStyle,
  );
  bindText($("styleName"), () => (selectedMaterial?.name || uiText("未知材质")));
  bindText($("textStyleName"), () => (selectedMaterial?.name || uiText("未知材质")));
}

function drawGroups() {
  $("groups").replaceChildren();
  for (const group of groupRows(
    state?.stats?.groups,
    state?.project?.protectedGroups,
  )) {
    const { name, count } = group;
    const row = document.createElement("div");
    row.className = `group-row${group.empty ? " empty" : ""}`;
    row.dataset.group = name;
    const check = document.createElement("input");
    check.type = "checkbox";
    check.checked = !renderer.groupsHidden.has(name);
    check.className = "group-visibility";
    bindText(check, () => (uiText("显示“{0}”的本地预览", name)), "title");
    check.setAttribute("aria-label", uiText("显示分组 {0}", name));
    check.onchange = async () => {
      if (check.checked) renderer.groupsHidden.delete(name);
      else renderer.groupsHidden.add(name);
      await renderer.load(sceneData.scene, sceneData.materials, {
          surfaces: sceneData.surfaces,
        mode: $("renderMode").value,
      });
    };
    const label = document.createElement("span");
    bindText(label, () => (name));
    label.onclick = () => {
      selected = new Set(
        sceneData.scene.boxes.concat(sceneData.scene.triangles||[],sceneData.scene.drawings||[])
          .filter((box) => box.group === name)
          .map((box) => box.id),
      );
      renderer.selection = selected;
      renderer.updateSelection();
      selectionInfo();
    };
    const number = document.createElement("small");
    bindText(number, () => (count));
    const protection = document.createElement("button");
    const protectedGroup = isGroupProtected(
      state?.project?.protectedGroups,
      name,
    );
    protection.className = `group-protect${protectedGroup ? " selected" : ""}`;
    protection.dataset.group = name;
    protection.type = "button";
    protection.setAttribute("aria-pressed", String(protectedGroup));
    bindText(protection, () => (protectedGroup
      ? uiText("“{0}”已受保护；点击解除后再编辑", name)
      : uiText("保护“{0}”，阻止绘制、擦除和变换", name)), "title");
    bindText(protection, () => (protectedGroup ? uiText("已保护") : uiText("保护")));
    protection.disabled = groupSettingsBusy;
    protection.onclick = () =>
      saveGroupProtection(name, !protectedGroup, protection);
    row.append(check, label, number, protection);
    $("groups").append(row);
  }
}

async function saveGroupProtection(name, protect, button) {
  if (groupSettingsBusy) return;
  groupSettingsBusy = true;
  button.disabled = true;
  bindText(button, () => (uiText("保存中")));
  try {
    const groups = setProtectedGroup(
      state.project.protectedGroups,
      name,
      protect,
    );
    const savedState = await api(
      "project",
      protectionSavePayload(groups, state.revision),
    );
    const saved = normalizeProtectedGroups(savedState.project?.protectedGroups);
    if (saved.includes(name) !== protect) {
      throw new Error(uiText("分组保护未保存，当前服务端可能尚未启用该设置"));
    }
    state.project.protectedGroups = saved;
    if (Number.isSafeInteger(Number(savedState.revision))) {
      state.revision = Number(savedState.revision);
    }
    window.__morphforge.state = state;
    updateState();
    await refresh(true);
    toast(
      protect
        ? uiText("已保护分组“{0}”；绘制、擦除和变换会被拒绝", name)
        : uiText("已解除分组“{0}”的保护，现在可以编辑", name),
    );
  } catch (cause) {
    error(cause);
    await refresh();
  } finally {
    groupSettingsBusy = false;
    if (sceneData) drawGroups();
  }
}

function selectionInfo() {
  bindText($("selectionInfo"), () => (selected.size
    ? uiText("已选 {0} 个对象 · 移动/复制读取精确位置XYZ作为偏移", selected.size)
    : uiText("选择工具点击，Shift多选；拖动框选")));
}

function setTool(next) {
  tool = next;
  for (const [id, name] of [
    ["addTool", "box"],
    ["eraseTool", "erase"],
    ["selectTool", "select"],
  ]) {
    $(id).classList.toggle("selected", next === name);
  }
  renderer.ghost.visible = false;
}

$("addTool").onclick = () => setTool("box");
$("eraseTool").onclick = () => setTool("erase");
$("selectTool").onclick = () => setTool("select");
$("styleSearch").oninput = drawPalette;

let hoverFrame = 0;
let lastPointer;
renderer.renderer.domElement.addEventListener("pointermove", (event) => {
  lastPointer = event;
  if (hoverFrame) return;
  hoverFrame = requestAnimationFrame(() => {
    hoverFrame = 0;
    if (!state || loading || !manualEditing || state.harness?.manualEditLocked) return;
    const hit = renderer.pick(lastPointer, {
      erase: tool === "erase" || lastPointer.shiftKey,
      layer: Number($("layer").value),
    });
    if (tool !== "select") {
      renderer.hover(hit, brush(), tool === "erase" || lastPointer.shiftKey);
    }
    if (hit) {
      bindText($("cursorPosition"), () => (`X ${hit.pos[0]} · Y ${hit.pos[1]} · Z ${hit.pos[2]}`));
    }
  });
});
renderer.renderer.domElement.addEventListener("pointerleave", () => {
  renderer.ghost.visible = false;
});
renderer.renderer.domElement.addEventListener("contextmenu", (event) =>
  event.preventDefault(),
);
renderer.renderer.domElement.addEventListener("pointerdown", (event) => {
  if (event.button === 0 && manualEditing && !state?.harness?.manualEditLocked && !event.altKey) pointerStart = [event.clientX, event.clientY];
});
renderer.renderer.domElement.addEventListener("pointerup", (event) => {
  if (event.button !== 0 || !pointerStart || loading || renderer.animationClip || !manualEditing || state?.harness?.manualEditLocked) return;
  const start = pointerStart;
  pointerStart = null;
  const distance = Math.hypot(
    event.clientX - start[0],
    event.clientY - start[1],
  );
  if (tool === "select") {
    if (!event.shiftKey) selected.clear();
    if (distance > 5) {
      const cell = sceneData.scene.cellSize;
      for (const box of sceneData.scene.boxes) {
        const point = renderer.projectPoint(
          box.min.map((value, axis) => (value + box.size[axis] / 2) * cell),
        );
        if (
          point.x >= Math.min(start[0], event.clientX) &&
          point.x <= Math.max(start[0], event.clientX) &&
          point.y >= Math.min(start[1], event.clientY) &&
          point.y <= Math.max(start[1], event.clientY)
        ) {
          selected.add(box.id);
        }
      }
      for(const face of sceneData.scene.triangles||[]){const point=renderer.projectPoint([0,1,2].map(a=>face.vertices.reduce((sum,p)=>sum+p[a],0)/3*cell));if(point.x>=Math.min(start[0],event.clientX)&&point.x<=Math.max(start[0],event.clientX)&&point.y>=Math.min(start[1],event.clientY)&&point.y<=Math.max(start[1],event.clientY))selected.add(face.id);}
      for(const drawing of sceneData.scene.drawings||[]){
        if(renderer.groupsHidden.has(drawing.group))continue;
        const point=renderer.projectPoint(drawing.start.map((v,a)=>(v+drawing.end[a])*.5*cell));
        if(point.x>=Math.min(start[0],event.clientX)&&point.x<=Math.max(start[0],event.clientX)&&point.y>=Math.min(start[1],event.clientY)&&point.y<=Math.max(start[1],event.clientY))selected.add(drawing.id);
      }
    } else {
      const hit = renderer.pick(event, { erase: true });
      if (hit?.box || hit?.triangle || hit?.drawing) {
        const id=(hit.box||hit.triangle||hit.drawing).id;
        if (event.shiftKey && selected.has(id))
          selected.delete(id);
        else selected.add(id);
      }
    }
    renderer.selection = selected;
    renderer.updateSelection();
    selectionInfo();
    return;
  }
  if (distance > 5) return;
  const hit = renderer.pick(event, {
    erase: tool === "erase" || event.shiftKey,
    layer: Number($("layer").value),
  });
  if(hit?.drawing&&(tool==="erase"||event.shiftKey))edit({op:"remove_drawings",ids:[hit.drawing.id]});
  else if(hit?.triangle&&(tool==="erase"||event.shiftKey))edit({op:"remove_triangles",ids:[hit.triangle.id]});
  else if (hit) edit(operation(hit.pos, event.shiftKey));
});

$("applyPosition").onclick = () => edit(operation(coord()));
$("layer").oninput = () => {
  $("layerValue").value = $("layer").value;
  renderer.setLayer(Number($("layer").value));
};
for (const [id, name] of [
  ["houseDemo", "house"],
  ["stairsDemo", "stairs"],
  ["directionsDemo", "directions"],
  ["stress5k", "stress5k"],
  ["stress16k", "stress16k"],
]) {
  $(id).onclick = () => edit({ op: "demo", name }).then(() => renderer.view());
}
$("clearScene").onclick = () => edit({ op: "clear" });
$("undo").onclick = () => edit({ op: "undo" });
$("redo").onclick = () => edit({ op: "redo" });
for (const [id, copy, turns, mirror] of [
  ["moveSelection", false, 0, null],
  ["copySelection", true, 0, null],
  ["rotateSelection", false, 1, null],
  ["mirrorSelection", false, 0, "x"],
]) {
  $(id).onclick = () => {
    if (!selected.size) return toast(uiText("请先选择内容"));
    edit({
      op: "transform",
      ids: [...selected],
      offset: turns || mirror ? [0, 0, 0] : coord(),
      turns,
      mirror,
      copy,
    });
  };
}
$("paintSelection").onclick = () => {
  if (!selected.size) return toast(uiText("请先选择内容"));
  const operations = sceneData.scene.boxes
    .filter((box) => selected.has(box.id))
    .map((box) => ({
      op: "paint",
      min: box.min,
      size: box.size,
      styleId: selectedStyle,
      tintId: Number($("tintId").value) || 0,
    }));
  const triangleIds=(sceneData.scene.triangles||[]).filter(t=>selected.has(t.id)).map(t=>t.id);
  if(triangleIds.length){const material=sceneData.materials.find(m=>m.id===selectedStyle);operations.push({op:"paint_triangles",ids:triangleIds,color:(material?.color||[.75,.75,.75]).map(v=>Math.round(Math.max(0,Math.min(1,v))*255))});}
  const drawingIds=(sceneData.scene.drawings||[]).filter(p=>selected.has(p.id)).map(p=>p.id);
  if(drawingIds.length){const material=sceneData.materials.find(m=>m.id===selectedStyle);operations.push({op:"paint_drawings",ids:drawingIds,color:(material?.color||[.75,.75,.75]).map(v=>Math.round(Math.max(0,Math.min(1,v))*255))});}
  edit({ operations });
};

$("fitView").onclick = () => renderer.view();
document.querySelectorAll("[data-view]").forEach((button) => {
  button.onclick = () => renderer.view(button.dataset.view);
});
$("renderMode").onchange = () => refresh(true).catch(error);
$("projectName").onchange = async () => {
  try {
    await api("project", {
      name: $("projectName").value,
      baseRevision: state.revision,
    });
    await refresh();
  } catch (cause) {
    error(cause);
  }
};
window.addEventListener("keydown", (event) => {
  if (["INPUT", "TEXTAREA", "SELECT"].includes(document.activeElement.tagName))
    return;
  if (
    (event.ctrlKey || event.metaKey) &&
    ["z", "y"].includes(event.key.toLowerCase())
  ) {
    event.preventDefault();
    edit({
      op: event.key.toLowerCase() === "y" || event.shiftKey ? "redo" : "undo",
    });
  }
});

async function showWorkspaces(kind) {
  try {
    bindText($("workspaceError"), () => (""));
    if (kind) $("workspaceKind").value = kind;
    await renderWorkspaces();
    if (!$("workspaceDialog").open) $("workspaceDialog").showModal();
    if (kind) $("workspaceName").focus();
  } catch (cause) { error(cause); }
}
async function renderWorkspaces() {
  const data = await api("workspaces");
  for (const [container, entries, active, action] of [
    ["workspaceProjects", data.projects, data.projectId, "open_project"],
    ["workspaceSessions", data.sessions, data.sessionId, "open_session"],
  ]) {
    $(container).replaceChildren();
    for (const item of entries) {
      const row = document.createElement("div");
      row.className = "workspace-item";
      row.dataset.workspaceId = item.id;
      const label = document.createElement("span");
      label.className = "grow";
      bindText(label, () => (item.name || item.title || uiText("未命名")));
      const detail = document.createElement("span");
      detail.className = "caption";
      bindText(detail, () => (action === "open_project" ? `${fmt(item.boxCount)} Box · v${item.revision}` : uiText("{0} 条上下文 · {1}", fmt(item.messages), new Date(item.createdAt).toLocaleString())));
      const open = document.createElement("button");
      bindText(open, () => (item.id === active ? uiText("当前") : uiText("打开")));
      open.disabled = item.id === active || workspaceBusy;
      open.onclick = () => changeWorkspace({ action, id: item.id });
      row.append(label, detail, open);
      $(container).append(row);
    }
  }
}
async function changeWorkspace(request) {
  if (workspaceBusy) return;
  workspaceBusy = true;
  $("workspaceCreate").disabled = true;
  bindText($("workspaceError"), () => (""));
  try {
    await edits;
    await api("workspaces", { ...request, baseProjectId: state.project.id, baseSessionId: state.harness.context.id });
    // Reinitialize per-context drafts, selection, history and polling together.
    location.reload();
  } catch (cause) {
    bindText($("workspaceError"), () => (cause.message));
    error(cause);
  } finally {
    workspaceBusy = false;
    $("workspaceCreate").disabled = false;
  }
}
$("newProject").onclick = () => showWorkspaces("new_project");
$("newContext").onclick = () => showWorkspaces("new_session");
$("workspaceOpen").onclick = () => showWorkspaces();
$("workspaceClose").onclick = () => $("workspaceDialog").close();
$("workspaceCreateForm").onsubmit = (event) => {
  event.preventDefault();
  changeWorkspace({action: $("workspaceKind").value, name: $("workspaceName").value});
};

function applyViewSettings() {
  renderer.setViewOptions(viewSettings);
  $("renderQuality").value=viewSettings.quality;
  $("captureQuality").value=viewSettings.captureQuality;
  for(const id of ["voxelGrid","captureGrid","surfaceCulling","showTargetBounds"]) $(id).checked=!!viewSettings[id];
}
$("renderOptionsOpen").onclick=()=>{$("renderOptionsDialog").showModal();};
$("renderOptionsClose").onclick=()=>$("renderOptionsDialog").close();
for(const [id,key] of [["renderQuality","quality"],["captureQuality","captureQuality"],...["voxelGrid","captureGrid","surfaceCulling","showTargetBounds"].map(k=>[k,k])]){
  $(id).onchange=async()=>{try{viewSettings=await api("view-settings",{[key]:$(id).type==="checkbox"?$(id).checked:$(id).value});applyViewSettings();}catch(cause){error(cause);}};
}
$("centerScene").onclick=()=>{$("transformDialog").showModal();previewTransform();};
$("transformClose").onclick=()=>$("transformDialog").close();
async function previewTransform(){
  $("transformApply").disabled=true;pendingTransform=null;
  try{
    const args={action:$("transformAction").value,offset:["shiftX","shiftY","shiftZ"].map(id=>Number($(id).value)),baseRevision:state.revision,dryRun:true};
    const result=await api("harness/tool",{name:"transform_scene",arguments:args});
    pendingTransform={...args,dryRun:false};
    bindText($("transformReport"), () => (uiText("偏移 {0} 格\n原范围 {1}\n新范围 {2}\n{3}", result.offset.join(", "), JSON.stringify(result.before.bounds), JSON.stringify(result.after.bounds), result.after.fits?uiText("坐标和容量可发布"):uiText("目标预检仍未通过，请检查尺寸/容量"))));
    $("transformApply").disabled=false;
  }catch(cause){bindText($("transformReport"), () => (cause.message));error(cause);}
}
$("transformPreview").onclick=previewTransform;
for(const id of ["transformAction","shiftX","shiftY","shiftZ"]) $(id).oninput=()=>{pendingTransform=null;$("transformApply").disabled=true;};
$("transformApply").onclick=async()=>{if(!pendingTransform)return;try{await api("harness/tool",{name:"transform_scene",arguments:pendingTransform});pendingTransform=null;$("transformDialog").close();await refresh(true);renderer.view();toast(uiText("整体定位已保存，可撤销"));}catch(cause){error(cause);}};
$("schemaUpgradeContext").onclick=async()=>{try{await api("harness/control",{action:"upgrade_context"});await refresh();toast(uiText("已升级工具说明；首次请求重新预热，实际缓存仍持续监测"));}catch(cause){error(cause);}};
$("autoCenterOnFinish").onchange=async()=>{try{await api("project",{autoCenterOnFinish:$("autoCenterOnFinish").checked});await refresh();}catch(cause){error(cause);}};
function runtimeFields(){const kind=$("runtimeKind").value,proc=["water","cloud","wood","stone"].includes(kind);$("runtimeSurfaceFields").hidden=kind==="color";$("runtimeNoiseFields").hidden=!proc;$("runtimeMetal").disabled=proc;bindText($("runtimeWireHint"), () => (kind==="color"?uiText("RGB24精确颜色；MF6直接发送，无需重新上传头像。"):kind==="pbr"?uiText("VRC使用RGB565；金属度/粗糙度/透明度/发光各4档。艺术原值保持。"):uiText("VRC颜色RGB444；噪声比例和种子各8档，其他表面参数4档。")));}
$("runtimeKind").onchange=()=>{const kind=$("runtimeKind").value;$("runtimeMetal").value=kind==="water"?.2:kind==="pbr"?.8:0;$("runtimeRough").value=kind==="water"?.12:kind==="cloud"?1:kind==="pbr"?.25:.9;$("runtimeOpacity").value=kind==="water"?.65:kind==="cloud"?.5:1;runtimeFields();};
let imagePlanePreview=null;
let imagePlaneProjectId="";
function imageToolArgs(action){const args={action,imageId:$("imageToolId").value,width:Number($("imageToolWidth").value),filter:$("imageToolFilter").value};if(!$("imageToolAspect").checked)args.height=Number($("imageToolHeight").value);if(action==="voxel_plane"){Object.assign(args,{baseRevision:state.revision,dryRun:true,plane:$("imageToolPlane").value,thickness:Number($("imageToolThickness").value),maxColors:Number($("imageToolColors").value),alphaThreshold:Number($("imageToolAlpha").value),group:"image-plane",canvasMode:$("imageToolCanvas").value});if(!$("imageToolCenter").checked)args.origin=["X","Y","Z"].map(a=>Number($("imageTool"+a).value));}return args;}
function invalidateImagePlane(){imagePlanePreview=null;$("imageToolApply").disabled=true;$("imageToolPreviewImage").hidden=true;}
async function imageToolsCatalog(selected){const value=await api("references");$("imageToolId").replaceChildren(...value.images.map(i=>new Option(i.title,i.id)));if(selected)$("imageToolId").value=selected;updateImageToolSource();}
function updateImageToolSource(){invalidateImagePlane();$("imageToolSource").src="/api/references/"+encodeURIComponent($("imageToolId").value)+"/image";}
$("imageToolsOpen").onclick=async()=>{try{await imageToolsCatalog(referencePicker.selected()[0]?.id);$("imageToolsDialog").showModal();}catch(cause){error(cause);}};
$("imageToolsClose").onclick=()=>$("imageToolsDialog").close();
$("imageToolId").onchange=updateImageToolSource;
for(const id of ["Width","Height","Aspect","Filter","Colors","Plane","Thickness","Center","X","Y","Z","Alpha","Canvas"])$("imageTool"+id).oninput=invalidateImagePlane;
$("imageToolInspect").onclick=async()=>{try{const args=imageToolArgs("inspect");args.points=[[0,0]];args.grid=[8,8];const value=await api("harness/tool",{name:"image_tools",arguments:args});bindText($("imageToolReport"), () => (JSON.stringify(value,null,2)));}catch(cause){error(cause);}};
$("imageToolResize").onclick=async()=>{try{const value=await api("harness/tool",{name:"image_tools",arguments:imageToolArgs("resize")});await imageToolsCatalog(value.id);bindText($("imageToolReport"), () => (JSON.stringify({id:value.id,width:value.width,height:value.height,parentImageId:value.parentImageId},null,2)));toast(uiText("缩放副本已加入图库，原图保留"));}catch(cause){error(cause);}};
$("imageToolPreview").onclick=async()=>{try{const args=imageToolArgs("voxel_plane");const value=await api("harness/tool",{name:"image_tools",arguments:args});imagePlanePreview={...args,dryRun:false};imagePlaneProjectId=state.project.id;$("imageToolPreviewImage").src=value.previewUrl;$("imageToolPreviewImage").hidden=false;bindText($("imageToolReport"), () => (JSON.stringify({size:value.size,generatedBoxes:value.generatedBoxes,paletteColors:value.paletteColors,materialsAdded:value.materialsAdded,target:value.target,canvasMode:value.canvasMode,canvasReplacesRegion:value.canvasReplacesRegion},null,2)));$("imageToolApply").disabled=false;}catch(cause){invalidateImagePlane();error(cause);}};
$("imageToolApply").onclick=async()=>{if(!imagePlanePreview)return;if(imagePlaneProjectId!==state.project.id){invalidateImagePlane();return toast(uiText("工程已切换，请重新预览图片平面"));}$("imageToolApply").disabled=true;try{const result=await api("harness/tool",{name:"image_tools",arguments:imagePlanePreview});invalidateImagePlane();await refresh(true);renderer.view("front");toast(uiText("图片平面已应用 · v{0}，可撤销", result.revision));$("imageToolsDialog").close();}catch(cause){invalidateImagePlane();error(cause);}};
$("runtimeMaterialOpen").onclick=()=>{runtimeFields();bindText($("runtimeReceipt"), () => (""));$("runtimeMaterialDialog").showModal();};
$("runtimeMaterialClose").onclick=()=>$("runtimeMaterialDialog").close();
$("runtimeMaterialApply").onclick=async()=>{const button=$("runtimeMaterialApply");button.disabled=true;try{const kind=$("runtimeKind").value,entry={name:$("runtimeName").value,color:$("runtimeColor").value,kind};if(kind!=="color")Object.assign(entry,{metallic:Number($("runtimeMetal").value),roughness:Number($("runtimeRough").value),opacity:Number($("runtimeOpacity").value),emission:Number($("runtimeEmission").value)});if(["water","cloud","wood","stone"].includes(kind))Object.assign(entry,{noiseScale:Number($("runtimeNoiseScale").value),seed:Number($("runtimeSeed").value)});const r=await api("harness/tool",{name:"define_materials",arguments:{baseRevision:state.revision,materials:[entry]}});selectedStyle=r.materials[0].id;bindText($("runtimeReceipt"), () => (JSON.stringify(r.materials[0].vrcEffective,null,2)));await refresh(true);toast(uiText("材质已添加并选用；MF6实时发送"));}catch(cause){error(cause);}finally{button.disabled=false;}};
$("pixelThemeOpen").onclick=()=>$("pixelThemeDialog").showModal();
$("pixelThemeClose").onclick=()=>$("pixelThemeDialog").close();
$("applyPixelTheme").onclick=async()=>{try{const result=await api("harness/tool",{name:"builtin_materials",arguments:{action:"apply",id:$("pixelThemeChoice").value,baseRevision:state.revision,replaceBaseMaterials:$("pixelThemeReplace").checked}});await refresh(true);$("pixelThemeDialog").close();toast(uiText("像素主题已加入 · 新增{0}种材质", result.materialsAdded));}catch(cause){error(cause);}};
$("avatarCatalogImport").onclick=()=>$("avatarCatalogFile").click();
$("avatarCatalogFile").onchange=async(event)=>{try{const file=event.target.files[0];if(!file)return;const form=new FormData();form.append("file",file);const r=await fetch("/api/vrc/profile/import",{method:"POST",body:form});const value=await r.json();if(!r.ok)throw new Error(value.error);await refresh();toast(value.matched?uiText("目标目录与已用材质匹配"):uiText("已记录目标目录，存在材质差异"));}catch(cause){error(cause);}event.target.value="";};
$("avatarCatalogMf6").onclick=async()=>{try{await api("vrc/profile",{action:"bundled_mf6"});await api("connect",{profile:"mf6",enabled:false,scale:Math.min(.5,Math.max(.0005,state.connection.placement.scale))});await refresh(true);toast(uiText("已载入基础目录；实际Avatar可自动识别或导入上传清单，发送保持关闭"));}catch(cause){error(cause);}};
$("avatarCatalogClear").onclick=async()=>{try{await api("vrc/profile",{action:"clear"});await refresh();}catch(cause){error(cause);}};

$("exportProject").onclick = async () => {
  try {
    const value = await api("export");
    const blob = new Blob([JSON.stringify(value, null, 2)], {
      type: "application/json",
    });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = (state.project.name || "morphforge") + ".json";
    link.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  } catch (cause) {
    error(cause);
  }
};
$("importProject").onclick = () => $("projectFile").click();
$("projectFile").onchange = async (event) => {
  try {
    const file = event.target.files[0];
    if (!file) return;
    await api("import", JSON.parse(await file.text()));
    await refresh(true);
    renderer.view();
  } catch (cause) {
    error(cause);
  }
  event.target.value = "";
};
$("packReportClose").onclick = () => $("packReportDialog").close();
function catalogOptions(){return `usedOnly=${$("catalogScope").value==="used"}&cellPixels=${$("catalogPixels").value}`;}
$("catalogBudget").onclick=async()=>{const button=$("catalogBudget");button.disabled=true;try{const v=await api("vrc/catalog/budget?"+catalogOptions());bindText($("catalogBudgetStatus"), () => (uiText("{0}种材质 · {1}个去重纹理层 · {2}组数组\nGPU纹理预计 {3}MiB（图集 {4}MiB + 含mipmap数组 {5}MiB）\n{6} · 不增加同步bit/动画器层/粒子系统\n{7}", v.styles, v.uniqueSlices, v.layerBanks, (v.bytes.totalGpuEstimate/1048576).toFixed(2), (v.bytes.atlas/1048576).toFixed(2), (v.bytes.layerMipmaps/1048576).toFixed(2), v.fits?uiText("预算通过"):uiText("预算超限，请减少材质或降低分辨率"), v.note)));}catch(cause){error(cause);}finally{button.disabled=false;}};
$("catalogExport").onclick=()=>{const link=document.createElement("a");link.href="/api/vrc/catalog?"+catalogOptions();link.download="morphforge-materials.zip";link.click();};
$("avatarMaterials").onclick=async()=>{const button=$("avatarMaterials");button.disabled=true;try{const r=await api("vrc/materials",{});await refresh(true);toast(uiText("已加入{0}种头像材质，现有几何保持不变，可在左侧搜索使用", r.materialsAdded));}catch(cause){error(cause);}finally{button.disabled=false;}};
$("discoverAvatar").onclick=async()=>{const button=$("discoverAvatar");button.disabled=true;try{const text=$("oscQueryPort").value.trim();await api("vrc/discover",{port:text?Number(text):null});await refresh(true);toast(uiText("已识别Avatar能力；查询不会开启发送"));}catch(cause){error(cause);}finally{button.disabled=false;}};
$("packImport").onclick = () => $("packFile").click();
$("packFile").onchange = async (event) => {
  try {
    const file = event.target.files[0];
    if (!file) return;
    const data = new FormData();
    data.append("file", file);
    const response = await fetch("/api/packs/import", {
      method: "POST",
      body: data,
    });
    const value = await response.json();
    if (!response.ok) throw new Error(value.error);
    toast(uiText("已导入 {0} 种材质；Avatar使用需预打包", value.materialsAdded));
    bindText($("packReportContent"), () => (JSON.stringify(value, null, 2)));
    $("packReportDialog").showModal();
    await refresh(true);
  } catch (cause) {
    error(cause);
  }
  event.target.value = "";
};

function connection(enabled = state.connection.enabled) {
  return {
    enabled,
    port: Number($("targetPort").value),
    profile: $("profile").value,
    interval: Number($("interval").value),
    capacity: Number($("targetCapacity").value),
    refreshSeconds: Number($("refreshSeconds").value),
    loopReplay: $("profile").value === "mf6" && $("loopReplay").checked,
    sendOrder: $("sendOrder").value,
    auto: $("autoPublish").checked,
    autoDetect:$("autoDetectAvatar").checked,
    publishStrategy:$("publishStrategy").value,
  };
}

async function configure(enabled) {
  // Capture the user's current controls immediately, then serialize writes.
  // In-flight status polling must not change controls used by the next edit.
  const patch=connection(enabled),projectId=state?.project.id;
  connectionPending++;connectionGeneration++;$("connect").disabled=true;
  const operation=connectionQueue.then(async()=>{
    if(state?.project.id!==projectId)throw new Error(uiText("工程已切换，旧的发送设置未提交；请在当前工程重新设置。"));
    const accepted=await api("connect",patch);
    acceptedConnection=accepted;
    if(state)state.connection=accepted;
  });
  connectionQueue=operation.catch(()=>{});
  try {
    await operation;
  } catch (cause) {
    error(cause);
  } finally {
    connectionPending--;connectionGeneration++;
    if(!connectionPending&&state)updateState();
    await refresh();
  }
}

$("connect").onclick = () => configure(!state.connection.enabled);
for (const id of [
  "targetPort",
  "profile",
  "interval",
  "targetCapacity",
  "refreshSeconds",
  "autoPublish",
  "autoDetectAvatar",
  "publishStrategy",
  "loopReplay",
  "sendOrder",
]) {
  $(id).onchange = () => configure(state.connection.enabled);
}
$("unityPort").onclick = () => {
  $("targetPort").value = 9012;
  configure(state.connection.enabled);
};
$("vrcPort").onclick = () => {
  $("targetPort").value = 9000;
  configure(state.connection.enabled);
};
$("publish").onclick = async () => {
  if ($("publish").disabled) return;
  $("publish").disabled = true;
  try {
    const selection = creativeUi?.publication() || {};
    const queued=await api("publish", {
      projectId: state.project.id, baseRevision: state.revision, ...selection,
    });
    if (selection.animation) creativeUi.publishedMotion(queued.motion);
    await refresh();
    toast(
      selection.animation
        ? queued.enabled ? uiText("模型与所选动画已进入发送队列；定义与绑定发完后开始播放") : uiText("模型与所选动画已准备；开启发送后播放")
        : queued.enabled
        ? queued.pending>0?uiText("版本已进入发送队列"):uiText("当前版本与本机已发记录一致；需要补漏可使用保留显示补播")
        : uiText("版本已准备；开启发送后执行"),
    );
  } catch (cause) {
    error(cause);
  } finally {
    $("publish").disabled = false;
  }
};
$("repairPublish").onclick=async()=>{try{await api("publish",{repair:true});await refresh();toast(uiText("已排入补播，保留现有显示；没有远端ACK。若有残留请清空并重建。"));}catch(cause){error(cause);}};
$("resync").onclick = async () => {
  try {
    await api("publish", { resync: true });
    await refresh();
  } catch (cause) {
    error(cause);
  }
};

async function placement(value) {
  try {
    await api("placement", value);
    await refresh();
    if ($("renderMode").value === "vrc")
      renderer.setScale(state.connection.placement.scale);
  } catch (cause) {
    error(cause);
  }
}

for (const id of ["world", "visible", "level", "allowGrab"]) {
  $(id).onchange = () => placement({ [id]: $(id).checked });
}
function showScale(physical){const p=physical/.05*100;$("scaleValue").value=Number(p.toFixed(1))+"%";$("scalePercent").value=Number(p.toFixed(2));if($("renderMode").value==="vrc")renderer.setScale(physical);}
$("scale").oninput=()=>showScale(.0005*10**Number($("scale").value));
$("scale").onchange=()=>placement({scale:.0005*10**Number($("scale").value)});
$("scalePercent").onchange=()=>placement({scale:Number($("scalePercent").value)/100*.05});
document.querySelectorAll("[data-scale]").forEach(button=>button.onclick=()=>placement({scale:Number(button.dataset.scale)/100*.05}));
$("resetAnchor").onclick = () => placement({ reset: true });

document.querySelectorAll("[data-tab]").forEach((button) => {
  button.onclick = () => {
    document.querySelectorAll("[data-tab]").forEach((item) => {
      item.classList.toggle("selected", item === button);
    });
    for (const name of ["ai", "text", "sync"]) {
      $("tab-" + name).hidden = name !== button.dataset.tab;
    }
    if (button.dataset.tab === "text") loadFonts();
  };
});

async function configHarness(value) {
  try {
    await api("harness/config", value);
    await refresh();
  } catch (cause) {
    error(cause);
  }
}

for (const [id, key] of [
  ["model", "model"],
  ["reasoning", "reasoning"],
  ["cacheGuard", "cacheGuard"],
  ["maxSteps", "maxSteps"],
  ["compactTokens", "compactTokens"],
  ["maxRunSeconds", "maxRunSeconds"],
  ["maxRunTokens", "maxRunTokens"],
  ["maxOutputTokens", "maxOutputTokens"],
  ["language", "language"],
  ["preset", "preset"],
  ["settingsPreset", "preset"],
  ["contextWindowTokens", "contextWindowTokens"],
  ["goalMinReviewRounds", "goalMinReviewRounds"],
  ["goalMaxContinuations", "goalMaxContinuations"],
  ["detailInstructions", "detailInstructions"],
  ["goalCriteria", "goalCriteria"],
]) {
  $(id).onchange = () => {
    const numeric = [
      "maxSteps",
      "compactTokens",
      "maxRunSeconds",
      "maxRunTokens",
      "maxOutputTokens",
      "contextWindowTokens",
      "goalMinReviewRounds",
      "goalMaxContinuations",
    ].includes(key);
    configHarness({ [key]: numeric ? Number($(id).value) : $(id).value });
  };
}
for (const key of ["sendImages", "goalMode", "strictAppendOnly", "enforceTargetLimits"]) {
  $(key).onchange = () => configHarness({ [key]: $(key).checked });
}
for (const id of [
  "providerBase",
  "providerApi",
  "providerImages",
  "providerKey",
  "providerStream",
  "providerReasoning",
  "providerRequiresKey",
  "providerClearKey",
  "providerTokenField",
]) {
  $(id).addEventListener("input", () => {
    providerDraftDirty = true;
    updateProviderImagePolicy();
  });
}
function updateProviderImagePolicy() {
  const supported = supportsFileUpload(
    $("providerBase").value,
    $("providerApi").value,
  );
  $("providerImageSetting").hidden = !supported;
  bindText($("providerImageHint"), () => (supported
    ? uiText("DeepSeek官方Responses可选择Files引用；默认内嵌图片也可使用。")
    : uiText("此接口自动内嵌图片，不调用专用上传通道。")));
  if (!supported) $("providerImages").value = "inline";
}

let enteredSettingsHere = false;
function renderSettingsPage() {
  const active = location.hash === "#settings" && !captureMode;
  $("settingsPage").hidden = !active;
  $("workbenchMain").hidden = active;
  document.body.classList.toggle("settings-page", active);
  $("settingsOpen").classList.toggle("selected", active);
  $("settingsOpen").setAttribute("aria-expanded", String(active));
}
function openSettings() {
  if (location.hash !== "#settings") {
    enteredSettingsHere = true;
    location.hash = "settings";
  }
}
function backToWorkbench() {
  if (enteredSettingsHere) {
    enteredSettingsHere = false;
    history.back();
  } else {
    history.replaceState(null, "", location.pathname + location.search);
    renderSettingsPage();
  }
}
$("settingsOpen").onclick = () => location.hash==="#settings"?backToWorkbench():openSettings();
$("aiSettingsOpen").onclick = openSettings;
$("settingsBack").onclick = backToWorkbench;
window.addEventListener("hashchange", renderSettingsPage);
document.addEventListener("keydown", (event) => {
  if (
    event.key === "Escape" &&
    !$("settingsPage").hidden &&
    !document.querySelector("dialog[open]")
  )
    backToWorkbench();
});
renderSettingsPage();

$("providerApi").addEventListener("change", () => {
  updateProviderImagePolicy();
});
$("providerBase").addEventListener("change", () => {
  try {
    if (new URL($("providerBase").value).hostname !== "api.deepseek.com")
      $("providerReasoning").checked = false;
  } catch {}
});
$("saveProvider").onclick = async () => {
  providerSaveBusy = true;
  $("saveProvider").disabled = true;
  try {
    await api("provider/config", {
      baseUrl: $("providerBase").value,
      apiType: $("providerApi").value,
      imageDelivery: $("providerImages").value,
      apiKey: $("providerKey").value,
      sendReasoning: $("providerReasoning").checked,
      stream: $("providerStream").checked,
      requiresApiKey: $("providerRequiresKey").checked,
      chatTokenField: $("providerTokenField").value,
      clearKey: $("providerClearKey").checked,
    });
    $("providerKey").value = "";
    $("providerClearKey").checked = false;
    providerDraftDirty = false;
    toast(uiText("API连接已保存"));
  } catch (cause) {
    error(cause);
  } finally {
    providerSaveBusy = false;
    await refresh();
  }
};
$("autoCompact").onchange = () =>
  configHarness({ autoCompact: $("autoCompact").checked });

const referencePicker = new ReferencePicker({
  api,
  error,
  storageGet: safeStorageGet,
  storageSet: safeStorageSet,
  onBusy: () => {
    if (state?.harness) updateHarnessControls(normalizeHarness(state.harness));
  },
});
referencePicker.setScope(uiScope || "default");

function promptDraftKey() {
  return `morphforge.prompt.${uiScope || "default"}`;
}

function savePromptDraft() {
  safeStorageSet(promptDraftKey(), $("prompt").value);
}

function restorePromptDraft() {
  const draft = safeStorageGet(promptDraftKey(), "");
  if (typeof draft === "string" && $("prompt").value === "")
    $("prompt").value = draft;
}

$("prompt").addEventListener("input", savePromptDraft);
$("prompt").addEventListener("keydown", (event) => {
  if (event.ctrlKey && event.key === "Enter") $("sendMessage").click();
});
$("messageMode").onchange = () => {
  if (state?.harness) updateHarnessControls(normalizeHarness(state.harness));
  safeStorageSet(
    `morphforge.mode.${uiScope || "default"}`,
    $("messageMode").value,
  );
};

function newRequestId() {
  return crypto.randomUUID
    ? crypto.randomUUID()
    : `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
}

async function sendFromComposer() {
  const prompt =
    $("prompt").value.trim() ||
    (referencePicker.selected().length
      ? "请根据所选参考图片进行体素建模。"
      : "");
  const snapshot = normalizeHarness(state?.harness);
  const selectedMode = $("messageMode").value;
  const mode = snapshot.active ? selectedMode : "steer";
  const intent = snapshot.active ? mode : "start";
  if (!prompt) {
    $("prompt").focus();
    return toast(uiText("请输入要发送的内容"));
  }
  await postHarnessMessage({ prompt, mode, intent, clearPrompt: true });
}

async function postHarnessMessage({
  prompt,
  mode,
  intent,
  clearPrompt = false,
  referenceIds,
}) {
  if (messageInFlight || messagePreparationInFlight || (referencePicker.busy && prompt?.trim())) return;
  messagePreparationInFlight = true;
  try { await creationPreferences.flush(); } catch(cause) { error(cause); return; }
  finally { messagePreparationInFlight = false; }
  const snapshot = normalizeHarness(state?.harness);
  const submittedPrompt = prompt ?? "";
  if (!submittedPrompt && !(mode === "interrupt" && snapshot.active)) {
    return toast(uiText("空消息只能用于立即打断"));
  }
  const payload = {
    prompt: submittedPrompt,
    mode: intent === "start" ? "steer" : mode,
    requestId: newRequestId(),
    referenceIds: submittedPrompt
      ? referenceIds || referencePicker.selected().map((item) => item.id)
      : [],
  };
  if (snapshot.active && snapshot.runId) payload.runId = snapshot.runId;
  messageInFlight = true;
  retryPayload = { payload, intent, submittedPrompt, clearPrompt };
  $("sendMessage").disabled = true;
  const pending = {
    requestId: payload.requestId,
    runId: payload.runId || "",
    epoch: snapshot.epoch,
    mode: intent,
    prompt: submittedPrompt,
    status: "sending",
    createdAt: new Date().toISOString(),
    error: "",
  };
  pendingMessages.push(pending);
  savePending();
  renderPending();
  try {
    let ack;
    try {
      ack = await api("harness/message", payload);
    } catch (cause) {
      const retryable =
        !cause.status || cause.status === 429 || cause.status >= 500;
      if (!retryable) throw cause;
      await new Promise((resolve) => setTimeout(resolve, 180));
      ack = await api("harness/message", payload);
    }
    const merged = mergeAck(pending, ack);
    updatePending(payload.requestId, merged);
    if (merged.status !== "accepted") {
      toast(merged.error || uiText("消息未被接受，输入仍保留"));
      return;
    }
    retryPayload = null;
    if (
      intent === "interrupt" &&
      previousHarness?.runId &&
      merged.epoch > previousHarness.epoch
    ) {
      markAssistantInterrupted(previousHarness.runId, previousHarness.epoch);
    }
    if (submittedPrompt && intent !== "start") {
      addUserChat({
        id: `message:${payload.requestId}`,
        text: submittedPrompt,
        runId: merged.runId,
        epoch: merged.epoch,
        mode,
        note: uiText("已接收，等待安全点"),
      });
    } else if (intent === "interrupt") {
      addUserChat({
        id: `message:${payload.requestId}`,
        text: uiText("暂停并取消当前请求"),
        runId: merged.runId,
        epoch: merged.epoch,
        mode,
        note: uiText("已接收，等待取消当前请求"),
      });
    }
    if (clearPrompt && $("prompt").value === submittedPrompt) {
      $("prompt").value = "";
      savePromptDraft();
    }
    await refresh();
    messageInFlight = false;
    updateHarnessControls(normalizeHarness(state?.harness));
  } catch (cause) {
    if (cause.status === 404 && intent === "start") {
      try {
        await api("harness/start", { prompt: submittedPrompt });
        updatePending(payload.requestId, {
          status: "accepted",
          error: "",
          acknowledgedAt: new Date().toISOString(),
        });
        retryPayload = null;
        if ($("prompt").value === submittedPrompt) {
          $("prompt").value = "";
          savePromptDraft();
        }
        await refresh();
        return;
      } catch (fallbackError) {
        cause = fallbackError;
      }
    }
    const conflict = conflictFromError(cause);
    updatePending(payload.requestId, {
      status: conflict ? "rejected" : "failed",
      error: friendlyError(cause),
    });
    error(cause);
  } finally {
    messageInFlight = false;
    if (state?.harness) updateHarnessControls(normalizeHarness(state.harness));
  }
}

$("sendMessage").onclick = sendFromComposer;
$("pauseAI").onclick = async () => {
  const snapshot = normalizeHarness(state?.harness);
  if (!snapshot.active) return;
  await postHarnessMessage({
    prompt: "",
    mode: "interrupt",
    intent: "interrupt",
  });
};
for (const [id, action] of [
  ["resumeAI", "resume"],
  ["stopAI", "stop"],
  ["compact", "compact"],
]) {
  $(id).onclick = async () => {
    try {
      if (action === "compact") toast(uiText("正在整理上下文…"));
      await api("harness/control", { action });
      await refresh();
      if (action === "compact") toast(uiText("上下文已整理"));
    } catch (cause) {
      error(cause);
    }
  };
}
$("saveNotes").onclick = async () => {
  try {
    await api("project", {
      pinnedNotes: $("pinnedNotes").value,
      baseRevision: state.revision,
    });
    await refresh();
    toast(uiText("固定要求已保存"));
  } catch (cause) {
    error(cause);
  }
};

async function runRecipe(dryRun) {
  const button = $(dryRun ? "dryRecipe" : "runRecipe");
  if (button.disabled) return;
  button.disabled = true;
  toast(dryRun ? uiText("正在估算绘图代码…") : uiText("正在执行绘图代码…"));
  try {
    const value = await api("harness/tool", {
      name: "run_recipe",
      arguments: {
        code: $("recipeCode").value,
        mode: "apply",
        seed: 1,
        baseRevision: state.revision,
        requestId: crypto.randomUUID(),
        dryRun,
      },
    });
    if (dryRun) {
      toast(
        uiText("预计 {0} Box / {1} 格", value.stats.boxCount, fmt(value.stats.voxelCount)),
      );
    } else {
      await refresh();
      renderer.view();
      toast(uiText("绘图代码已应用，可撤销"));
    }
  } catch (cause) {
    error(cause);
  } finally { button.disabled = false; }
}
$("runRecipe").onclick = () => runRecipe(false);
$("dryRecipe").onclick = () => runRecipe(true);

async function capture() {
  const result = await api("harness/tool", {
    name: "capture_views",
    arguments: { revision: state.revision, views: ["hero", "front", "top"], mode: $("renderMode").value, quality: viewSettings.captureQuality, voxelGrid: viewSettings.captureGrid, surfaceCulling: viewSettings.surfaceCulling },
  });
  window.__morphforge.lastCapture = result;
  bindText($("captureRevision"), () => ("v" + result.revision));
  $("captureImages").replaceChildren();
  for (const image of result.images) {
    const wrap = document.createElement("div");
    const img = document.createElement("img");
    const caption = document.createElement("p");
    img.src = image.url;
    bindText(caption, () => (image.view));
    wrap.append(img, caption);
    $("captureImages").append(wrap);
  }
  return result;
}
$("capture").onclick = async () => {
  try {
    toast(uiText("正在生成独立三视图…"));
    await capture();
    $("captureDialog").showModal();
  } catch (cause) {
    error(cause);
  }
};
$("captureClose").onclick = () => $("captureDialog").close();
$("saveArtwork").onclick = async () => {
  const button = $("saveArtwork");
  if (button.disabled) return;
  button.disabled = true;
  toast(uiText("正在保存作品和三视图…"));
  try {
    const images = await capture();
    await api("artworks", {
      action: "save",
      title: state.project.name,
      tags: [],
      thumbnail: images.images[0]?.artifact,
      views: images.images.map((image) => image.artifact),
    });
    toast(uiText("作品和三视图已保存"));
  } catch (cause) {
    error(cause);
  } finally { button.disabled = false; }
};

async function library() {
  const value = await api("artworks");
  const query = $("librarySearch").value.toLowerCase();
  $("libraryGrid").replaceChildren();
  for (const artwork of [...value.artworks, ...(value.examples || [])]) {
    if (
      query &&
      !`${artwork.title} ${artwork.tags.join(" ")}`
        .toLowerCase()
        .includes(query)
    ) {
      continue;
    }
    const card = document.createElement("article");
    card.className = "artwork";
    card.dataset.artworkId = artwork.id;
    card.dataset.origin = artwork.origin || "user";
    const img = document.createElement("img");
    if (artwork.thumbnail)
      img.src = "/artifacts/" + encodeURIComponent(artwork.thumbnail);
    const body = document.createElement("div");
    const name = document.createElement("h3");
    const info = document.createElement("p");
    const load = document.createElement("button");
    bindText(name, () => (artwork.title));
    bindText(info, () => (uiText("{0} · {1} Box · {2} 面 · {3} 格", artwork.origin === "builtin" ? uiText("内置示例") : uiText("我的作品"), fmt(artwork.boxCount), fmt(artwork.triangleCount||0), fmt(artwork.voxelCount))));
    bindText(load, () => (artwork.origin === "builtin" ? uiText("载入可编辑副本") : uiText("载入并继续")));
    body.append(name, info);
    if (artwork.description) {
      const description = document.createElement("p");
      bindText(description, () => (artwork.description));
      body.append(description);
    }
    if (artwork.sourceCredits?.length) {
      const credits = document.createElement("p");
      credits.className = "caption";
      bindText(credits, () => (artwork.sourceCredits
        .map((credit) => `${credit.author} · ${credit.license}`)
        .join("；")));
      body.append(credits);
    }
    if (artwork.views?.length) {
      const preview = document.createElement("button");
      bindText(preview, () => (uiText("多视图")));
      preview.onclick = () => {
        bindText($("captureRevision"), () => (artwork.title));
        $("captureImages").replaceChildren();
        for (const view of artwork.views) {
          const image = document.createElement("img");
          image.src = "/artifacts/" + encodeURIComponent(view);
          $("captureImages").append(image);
        }
        $("captureDialog").showModal();
      };
      body.append(preview);
    }
    load.onclick = async () => {
      try {
        await api("artworks", {
          action: "fork",
          id: artwork.id,
          baseRevision: state.revision,
        });
        await refresh(true);
        renderer.view();
        $("libraryDialog").close();
      } catch (cause) {
        error(cause);
      }
    };
    body.append(load);
    if (artwork.origin !== "builtin") {
      const remove = document.createElement("button");
      bindText(remove, () => (uiText("删除"))); remove.className = "danger";
      remove.onclick = async () => {
        remove.disabled=true;
        try {
          const response=await fetch("/api/artworks/"+encodeURIComponent(artwork.id),{method:"DELETE"});
          const result=await response.json();
          if(!response.ok)throw Error(result.error||uiText("删除作品失败"));
          await library();toast(uiText("作品已从共享库删除；当前工程保留，原文件存入回收归档。"));
        } catch(cause) { error(cause); remove.disabled=false; }
      };
      body.append(remove);
    }
    card.append(img, body);
    $("libraryGrid").append(card);
  }
}

$("libraryOpen").onclick = async () => {
  try {
    await library();
    $("libraryDialog").showModal();
  } catch (cause) {
    error(cause);
  }
};
$("libraryClose").onclick = () => $("libraryDialog").close();
$("librarySearch").oninput = () => library().catch(error);
$("diagnosticsExport").onclick = () => {
  const link = document.createElement("a");
  link.href = "/api/diagnostics";
  link.download = "morphforge-diagnostics.zip";
  link.click();
};
$("timelineToggle").onclick = () => {
  $("timeline").hidden = !$("timeline").hidden;
  if (!timelineView) timelineView = new TimelineView(api, error);
  if (!$("timeline").hidden) timelineView.reload().catch(error);
  setTimeout(() => renderer.resize(), 30);
};
$("cacheShowConfig").onclick = () => $("settingsOpen").click();
$("cacheAcknowledge").onclick = async () => {
  try {
    await api("harness/control", { action: "ack_cache_warning" });
    await refresh();
    toast(uiText("已确认告警；恢复运行后将继续监测"));
  } catch (cause) {
    error(cause);
  }
};

async function pollEvents() {
  try {
    const value = await api("events?brief=true&after=" + lastSeq);
    if (value.events.length) {
      lastSeq = value.events.at(-1).seq;
      eventRows.push(...value.events);
      eventRows = eventRows.slice(-1500);
      applyHarnessEvents(value.events);
      activityView?.load().catch(error);
      if (timelineView && !$("timeline").hidden)
        timelineView.load().catch(error);
    }
  } catch {
    // The main refresh loop reports backend availability.
  }
}

$("historyOpen").onclick = async () => {
  try {
    const history = await api("history");
    $("historyList").replaceChildren();
    for (const revision of history.revisions.slice().reverse()) {
      const row = document.createElement("div");
      row.className = "history-row";
      const label = document.createElement("span");
      bindText(label, () => ("v" + revision));
      const commit = history.commits.find((item) => item.revision === revision);
      const title = document.createElement("span");
      title.className = "grow";
      bindText(title, () => (commit?.title || uiText("草稿")));
      const restore = document.createElement("button");
      bindText(restore, () => (uiText("从此版本继续")));
      restore.onclick = async () => {
        try {
          await api("undo", { revision });
          await refresh();
          $("historyDialog").close();
        } catch (cause) {
          error(cause);
        }
      };
      row.append(label, title, restore);
      $("historyList").append(row);
    }
    $("historyDialog").showModal();
  } catch (cause) {
    error(cause);
  }
};
$("historyClose").onclick = () => $("historyDialog").close();

function normalizeFont(font) {
  if (typeof font === "string") return { id: font, name: font };
  const familyStyle = [font.family, font.style]
    .filter(Boolean)
    .filter((value, index, list) => list.indexOf(value) === index)
    .join(" · ");
  return {
    id: font.id || font.fontId || font.family || "",
    name:
      font.name ||
      font.displayName ||
      font.label ||
      familyStyle ||
      font.id ||
      font.fontId,
  };
}

async function loadFonts(force = false) {
  if (fontsLoaded && !force) return true;
  try {
    const value = await api("fonts");
    fontCatalog = (value.fonts || [])
      .map(normalizeFont)
      .filter((font) => font.id);
    const select = $("textFont");
    const previous = select.value;
    select.replaceChildren();
    for (const font of fontCatalog) {
      const option = document.createElement("option");
      option.value = font.id;
      bindText(option, () => (font.name));
      select.append(option);
    }
    const preferred =
      [...fontCatalog].find((font) => font.id === previous) ||
      [...fontCatalog].find((font) => font.id === "builtin:noto-sans-sc") ||
      fontCatalog[0];
    if (preferred) select.value = preferred.id;
    fontsLoaded = fontCatalog.length > 0;
    select.disabled = !fontsLoaded;
    if (!fontsLoaded) {
      const option = document.createElement("option");
      bindText(option, () => (uiText("字体服务没有可用字体")));
      select.append(option);
    }
    syncTextButtons();
    return fontsLoaded;
  } catch (cause) {
    fontsLoaded = false;
    bindText($("textMetrics"), () => (uiText("字体服务尚未就绪：") + friendlyError(cause)));
    $("textMetrics").className = "status error";
    syncTextButtons();
    return false;
  }
}

$("textFontRefresh").onclick = async () => {
  $("textFontRefresh").disabled=true;
  toast(uiText("正在刷新字体…"));
  try { if(await loadFonts(true)) toast(uiText("字体列表已刷新 · {0}种", fontCatalog.length)); }
  finally { $("textFontRefresh").disabled=false; }
};

function collectTextRequest() {
  return buildTextRequest({
    text: $("textContent").value,
    fontId: $("textFont").value,
    fontSize: $("textSize").value,
    depth: $("textDepth").value,
    origin: values(["textX", "textY", "textZ"]),
    plane: $("textPlane").value,
    facing: $("textFacing").value,
    align: $("textAlign").value,
    letterSpacing: $("textLetterSpacing").value,
    lineSpacing: $("textLineSpacing").value,
    maxWidth: $("textMaxWidth").value,
    styleId: selectedStyle,
    tintId: $("tintId").value,
    motionPhase: 0,
    group: $("textGroup").value,
  });
}

function currentFingerprint() {
  return textRequestFingerprint(collectTextRequest());
}

function syncTextButtons() {
  const request = collectTextRequest();
  const hasText = request.text.trim().length > 0;
  $("textPreview").disabled = !fontsLoaded || !hasText;
  $("textClearPreview").disabled = !currentTextPreview;
  $("textApply").disabled =
    textApplyBusy ||
    !previewCanApply(
      currentTextPreview,
      currentPreviewSentJSON,
      textRequestFingerprint(request),
      state?.stats?.targetBoxes,
      currentPreviewServerFingerprint,
    );
}

function clearTextPreview() {
  previewSequence++;
  renderer.clearTextPreview();
  currentTextPreview = null;
  currentPreviewSentJSON = "";
  currentPreviewServerFingerprint = "";
  currentPreviewRequestId = "";
  currentPreviewRequest = null;
  bindText($("textMetrics"), () => (uiText("预览已清除。草稿没有变化。")));
  $("textMetrics").className = "status";
  syncTextButtons();
}

$("textClearPreview").onclick = clearTextPreview;

function scheduleTextPreview() {
  clearTimeout(previewTimer);
  if (!$("textContent").value.trim()) {
    clearTextPreview();
    return;
  }
  previewTimer = setTimeout(() => requestTextPreview(false), 320);
  syncTextButtons();
}

for (const id of [
  "textContent",
  "textFont",
  "textSize",
  "textDepth",
  "textPlane",
  "textFacing",
  "textAlign",
  "textLetterSpacing",
  "textLineSpacing",
  "textMaxWidth",
  "textX",
  "textY",
  "textZ",
  "textGroup",
  "tintId",
]) {
  $(id).addEventListener("input", scheduleTextPreview);
  $(id).addEventListener("change", scheduleTextPreview);
}
$("textPreview").onclick = () => requestTextPreview(true);

async function requestTextPreview(manual) {
  clearTimeout(previewTimer);
  if (!fontsLoaded && !(await loadFonts())) return;
  const request = collectTextRequest();
  if (!request.text.trim()) {
    clearTextPreview();
    return toast(uiText("请输入文字"));
  }
  const fingerprint = textRequestFingerprint(request);
  const sequence = ++previewSequence;
  const previewRequestId = newRequestId();
  $("textPreview").disabled = true;
  bindText($("textMetrics"), () => (uiText("预览计算中…")));
  $("textMetrics").className = "status";
  try {
    const value = await api("text/preview", {
      ...request,
      requestId: previewRequestId,
    });
    if (sequence !== previewSequence || fingerprint !== currentFingerprint())
      return;
    if (
      (value.requestId || value.preview?.requestId) &&
      (value.requestId || value.preview?.requestId) !== previewRequestId
    ) {
      throw new Error(uiText("文字预览状态校验失败，请重新预览。"));
    }
    if (value.ok === false || value.error) {
      const failure = new Error(formatPreviewError(value));
      failure.payload = value;
      throw failure;
    }
    const preview = value.preview && !value.operations ? value.preview : value;
    const serverFingerprint =
      value.requestFingerprint || preview?.requestFingerprint || "";
    if (!/^[0-9a-f]{64}$/i.test(serverFingerprint)) {
      throw new Error(uiText("文字预览结果无法校验，请重新预览。"));
    }
    const boxes = previewBoxes(preview);
    const metrics = previewMetrics(preview);
    const style =
      sceneData?.materials.find(
        (material) => material.id === request.styleId,
      ) || {};
    renderer.setTextPreview(boxes, {
      ...style,
      tintId: request.tintId,
    });
    currentTextPreview = preview;
    currentPreviewSentJSON = fingerprint;
    currentPreviewServerFingerprint = serverFingerprint;
    currentPreviewRequestId = previewRequestId;
    currentPreviewRequest = request;
    const dimensionText = metrics.dimensions
      ? metrics.dimensions.join(" × ")
      : "—";
    const lines = [
      uiText("尺寸 {0} 格", dimensionText),
      uiText("{0} Box · {1} 体素", fmt(metrics.boxCount), fmt(metrics.voxelCount)),
      uiText("目标容量 {0} Box", fmt(state?.stats?.targetBoxes)),
    ];
    if (metrics.missing.length)
      lines.push(uiText("缺字：") + metrics.missing.join(" "));
    if (metrics.overCapacity) lines.push(uiText("超出目标材料容量"));
    if (metrics.warnings.length) lines.push(...metrics.warnings);
    if (!boxes.length) lines.push(uiText("预览没有返回 Box"));
    bindText($("textMetrics"), () => (lines.join("\n")));
    $("textMetrics").className =
      metrics.missing.length || metrics.overCapacity
        ? "status error"
        : "status ok";
    if (manual) toast(uiText("预览已更新，草稿未修改"));
  } catch (cause) {
    if (sequence !== previewSequence) return;
    currentTextPreview = null;
    currentPreviewSentJSON = "";
    currentPreviewServerFingerprint = "";
    currentPreviewRequestId = "";
    currentPreviewRequest = null;
    renderer.clearTextPreview();
    bindText($("textMetrics"), () => (formatPreviewError(cause)));
    $("textMetrics").className = "status error";
    if (manual && !formatPreviewError(cause).includes(uiText("缺字"))) error(cause);
  } finally {
    syncTextButtons();
  }
}

$("textApply").onclick = async () => {
  if (textApplyBusy) return;
  const fingerprint = currentFingerprint();
  if (
    !previewCanApply(
      currentTextPreview,
      currentPreviewSentJSON,
      fingerprint,
      state?.stats?.targetBoxes,
      currentPreviewServerFingerprint,
    )
  ) {
    return toast(uiText("请先预览当前参数；缺字或超容量时不能应用"));
  }
  textApplyBusy = true;
  syncTextButtons();
  try {
    const applyRequestId = newRequestId();
    const value = await api("text/apply", {
      ...currentPreviewRequest,
      baseRevision: state.revision,
      requestId: applyRequestId,
      previewRequestId: currentPreviewRequestId,
      previewFingerprint: currentPreviewServerFingerprint,
    });
    const applied = normalizeApplyResult(value, applyRequestId);
    clearTextPreview();
    await refresh(true);
    renderer.view();
    toast(
      applied.warning
        ? uiText("文字已应用 · v{0} · {1}", applied.revision, applied.warning)
        : uiText("文字已应用 · v{0}", applied.revision),
    );
  } catch (cause) {
    error(cause);
    if (cause.status === 409) {
      bindText($("textMetrics"), () => (uiText("草稿版本已变化。预览未修改草稿，请重新预览后再应用。")));
      $("textMetrics").className = "status error";
    }
  } finally {
    textApplyBusy = false;
    syncTextButtons();
  }
};

textPanelUi=setupTextPanelUi({$,api,getScene:()=>sceneData,getState:()=>state,renderer,refresh,error,toast,canEdit:()=>manualEditing&&!state?.harness?.manualEditLocked});
drawingUi=setupDrawingUi({$,api,getState:()=>state,renderer,refresh,error,toast});

setupMinecraftUi({$,api,getState:()=>state,getMaterials:()=>sceneData?.materials,refresh,error,toast});

const creationPreferences=setupMeshUi({$,getScene:()=>sceneData,getState:()=>state,getSelected:()=>selected,getSelectedStyle:()=>selectedStyle,api,edit,refresh,error});

if (captureMode) {
  document.body.classList.add("capture-mode");
  renderer.renderer.setAnimationLoop(null);
  window.__morphforge.ready = true;
  let capturing = false;
  setInterval(async () => {
    if (capturing) return;
    capturing = true;
    let job;
    try {
      job = await api("capture/jobs");
      if (job) {
        await renderer.load(job.scene, job.materials, {
          mode: job.mode,
          scale: 1,
          surfaces: job.surfaces,
          textLayouts:job.textLayouts,
        });
        if(job.animationClip)renderer.setAnimationPose(job.animationClip,job.animationTime||0);
        const images = await renderer.capture(
          job.views,
          job.comparison?.mode==="split"?Math.floor(job.width/2):job.width,
          job.height,
          job.shots,
          job,
        );
        await api("capture/result", {
          id: job.id,
          revision: renderer.renderRevision,
          images:await composeCapture(images,job.comparison,job.width,job.height),
        });
      }
    } catch (cause) {
      if (job) {
        await api("capture/result", {
          id: job.id,
          revision: job.revision,
          error: cause.message,
        }).catch(() => {});
      }
      console.error(cause);
    } finally {
      capturing = false;
    }
  }, 200);
} else {
  const storedMode = safeStorageGet("morphforge.mode.default", "steer");
  if (["steer", "interrupt", "queue"].includes(storedMode))
    $("messageMode").value = storedMode;
  try{viewSettings=await api("view-settings");applyViewSettings();}catch(cause){error(cause);}
  creativeUi = setupCreativeUi({$,api,renderer,getState:()=>state,getScene:()=>sceneData,getSelected:()=>selected,refresh,error,toast});
  window.__morphforge.creative=creativeUi;
  activityView = new ActivityView(api,error);
  setupDesktopMenus({getCellSize:()=>sceneData?.scene.cellSize??.25,getName:()=>state?.project.name||"morphforge",onError:error,onLayout:()=>renderer.resize()});
  await refresh();
  switchUiScope(state?.project?.id, state?.harness?.context?.referenceImages, state?.harness?.context?.id);
  restorePromptDraft();
  renderPending();
  renderChat();
  renderer.view();
  loadFonts();
  setInterval(() => refresh(), 700);
  setInterval(pollEvents, 700);
  let lastActivityPoll=0;setInterval(()=>{const active=["running","compacting","interrupting"].includes(state?.harness?.interaction?.phase);if(document.hidden||Date.now()-lastActivityPoll<(active?250:1500))return;lastActivityPoll=Date.now();activityView.load().catch(error);},250);
}

$("manualEditToggle").onclick=()=>{manualEditing=!manualEditing;bindText($("manualEditToggle"), () => (uiText("手工编辑：{0}", manualEditing?uiText("开启"):uiText("关闭"))));$("manualEditToggle").setAttribute("aria-pressed",String(manualEditing));renderer.ghost.visible=false;};
const pendingFold=$("pendingMessages").closest("details");
$("composerQueueSlot").append($("pendingMessages").parentElement);pendingFold.remove();
$("composerOptions").prepend($("preset").closest("details"));
for(const dialog of document.querySelectorAll("dialog"))dialog.addEventListener("click",event=>{if(event.target!==dialog)return;const rect=dialog.getBoundingClientRect();if(event.clientX<rect.left||event.clientX>rect.right||event.clientY<rect.top||event.clientY>rect.bottom)dialog.close();});

$("miniOpen").onclick=async()=>{try{const status=await api("desktop/window");if(!status.native){window.open("/mini.html?mode=speech","morphforge-mini","width=480,height=360");return;}await api("desktop/window",{action:"open",mode:"speech"});await api("desktop/window",{action:"hide_main"});}catch(e){error(e);}};
$("mainWindowPin").onchange=()=>api("desktop/window",{action:"pin",window:"main",pinned:$("mainWindowPin").checked}).catch(error);
api("desktop/window").then(v=>{$("mainWindowPin").checked=!!v.preferences.mainPinned;$("mainWindowPin").disabled=!v.native;}).catch(error);

if(!captureMode){setupComposerSpeech({$,api,getState:()=>state,getPanelId:()=>textPanelUi?.selected()||"",toast,fail:error});
setupSpeechSettings({$,api,fail:error});}

$("voiceRecordingBadge").onclick=()=>api("speech/control",{action:"stop"}).catch(error);

onLanguageChange(() => { if(state) { updateState(); renderPending(); activityView?.render(false); timelineView?.render?.(); } });
