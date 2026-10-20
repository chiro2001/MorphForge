import { ActivityView } from "/activity-ui.js";
import { setupDesktopMenus } from "/desktop-menu.js";
import { TimelineView } from "/timeline-ui.js";
import { ReferencePicker } from "/reference-ui.js";
import { usageDisplay, supportsFileUpload } from "/usage-ui.js";
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
let polling = false;
let edits = Promise.resolve();
let lastSeq = 0;
let eventRows = [];
let timelineView = null;
let activityView = null;
let toastTimer;
let pointerStart = null;
let loading = false;
let selected = new Set();
let uiScope = "";
let pendingMessages = [];
let chatEntries = [];
let previousHarness = null;
let messageInFlight = false;
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
        ? "本机服务返回了无法识别的响应"
        : `本机服务暂时不可用（${response.status}）`,
    };
  }
  if (!response.ok) {
    const detail =
      typeof value.error === "string"
        ? value.error
        : value.error
          ? JSON.stringify(value.error)
          : `请求失败（${response.status}）`;
    const error = new Error(detail);
    error.status = response.status;
    error.payload = value;
    error.code = value.code || value.error?.code || "";
    throw error;
  }
  return value;
}

function toast(text) {
  $("toast").textContent = text;
  $("toast").classList.add("visible");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => $("toast").classList.remove("visible"), 4500);
}

function error(value) {
  toast(friendlyError(value));
  console.error(value);
}

function fmt(number) {
  return Number(number || 0).toLocaleString();
}

function time(seconds) {
  if (!seconds) return "0秒";
  if (seconds < 60) return Math.ceil(seconds) + "秒";
  if (seconds < 3600)
    return Math.floor(seconds / 60) + "分" + Math.ceil(seconds % 60) + "秒";
  return (
    Math.floor(seconds / 3600) + "时" + Math.ceil((seconds % 3600) / 60) + "分"
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
    element.querySelector("small").textContent =
      entry.role === "user" ? entry.label || "你" : "MorphForge";
    const body = element.querySelector("span:not(.entry-note)");
    if (body.textContent !== (entry.text || ""))
      body.textContent = entry.text || "";
    const note = element.querySelector(".entry-note");
    note.textContent = entry.note || "";
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
        ? "你 · 排队"
        : entry.mode === "interrupt"
          ? "你 · 打断后"
          : "你",
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
      note: "当前请求尚未产生可见文本，已在生成前被取消。",
    };
    chatEntries.push(entry);
    saveChat();
    renderChat();
    return;
  }
  entry.interrupted = true;
  entry.note = entry.text
    ? "该段输出在安全边界前被取消。"
    : "当前请求尚未产生可见文本。";
  saveChat();
  renderChat();
}

function addCompletedSummary(summary) {
  if (!summary?.message?.trim() || !summary.runId) return false;
  const exact = findExactAssistantMessage(chatEntries, summary);
  if (exact) {
    if (!exact.final) {
      exact.final = true;
      exact.note = exact.note || "完成总结";
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
      note: "完成总结",
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
    note: "完成总结",
  });
}

function renderPending() {
  pendingMessages = dedupeMessages(pendingMessages);
  $("pendingCount").textContent = String(pendingMessages.length);
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
    empty.textContent = "没有待处理消息";
    list.append(empty);
    return;
  }
  for (const message of pendingMessages.slice().reverse()) {
    const row = document.createElement("div");
    row.className = `pending-item ${message.status}`;
    row.dataset.requestId = message.requestId;
    const prompt = document.createElement("span");
    prompt.className = "pending-prompt";
    prompt.textContent = message.prompt || "暂停并取消当前请求";
    prompt.title = message.prompt || "";
    const detail = document.createElement("span");
    detail.className = "pending-detail";
    const mode = document.createElement("span");
    mode.className = "caption";
    mode.textContent =
      { start: "开始", steer: "插话", interrupt: "打断", queue: "排队" }[
        message.mode
      ] || message.mode;
    const status = document.createElement("span");
    status.className = `message-state ${message.status}`;
    status.textContent =
      statusLabel(message.status) +
      (message.status === "accepted" && message.queuePosition
        ? ` · #${message.queuePosition}`
        : "");
    detail.append(mode, status);
    row.append(prompt, detail);
    if (message.error) {
      const failure = document.createElement("span");
      failure.className = "pending-detail caption";
      failure.textContent = message.error;
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
        note: next === "applied" ? "已应用到当前运行" : "已接收，等待安全点",
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
  $("sendMessage").textContent = action.label;
  $("sendMessage").disabled =
    messageInFlight ||
    referencePicker.busy ||
    (snapshot.stopping && !snapshot.active);
  $("messageModeHint").textContent = messageModeHint(snapshot, mode);
  $("pauseAI").disabled = !snapshot.active || snapshot.paused;
  $("resumeAI").disabled = !snapshot.active || !snapshot.paused;
  $("stopAI").disabled = !snapshot.active;
  const run = snapshot.run;
  const status = [`状态：${phaseLabel(snapshot)}`];
  if (snapshot.queue.length) status.push(`排队 ${snapshot.queue.length}`);
  if (run) {
    status.push(`${run.steps || 0} 步 · ${fmt(run.tokens)} Token`);
    if (run.error) status.push(friendlyError({ message: run.error }));
  }
  $("runStatus").textContent = status.join("\n");
}

async function refresh(force = false) {
  if (polling) return;
  polling = true;
  try {
    const next = await api("state");
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
    const previewCatalog=JSON.stringify([next.connection.profile,next.materialCompatibility?.fingerprint]);
    const previewScale=$("renderMode").value === "vrc" ? next.connection.placement.scale : 1;
    if(renderer.scale!==previewScale) renderer.setScale(previewScale);
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
          mode: $("renderMode").value,
          scale:
            $("renderMode").value === "vrc"
              ? state.connection.placement.scale
              : 1,
        });
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
    window.__morphforge.ready = true;
  } catch (cause) {
    $("saveState").textContent = "后端暂时不可用";
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
  setValue("modelingStyle", s.project.modelingStyle||"mixed");
  $("activeSessionTitle").textContent = harness.context.title || "对话";
  $("schemaUpgradeNotice").hidden=!harness.context.schemaUpgradeRequired;
  $("schemaUpgradeContext").hidden=!harness.context.schemaUpgradeRequired;
  $("saveState").textContent = "已保存 · v" + s.revision;
  $("boxCount").textContent =
    fmt(s.stats.boxCount) + " Box · " + fmt(s.stats.triangleCount||0) + " 面 / 目标 " + fmt(s.stats.targetBoxes);
  $("voxelCount").textContent = fmt(s.stats.voxelCount) + " 格";
  $("bounds").textContent = s.stats.bounds
    ? "范围 " +
      s.stats.bounds.min.join(",") +
      " → " +
      s.stats.bounds.max.join(",")
    : "范围为空";
  $("emptyHint").hidden = s.stats.boxCount > 0 || s.stats.triangleCount > 0;
  const target=s.targetCompatibility;
  $("vrcBoundsStatus").textContent=target ? `${target.fits?"坐标/容量可发布":"目标预检未通过"} · ${target.outOfBoundsCount} 个越界对象\n作品尺寸 ${target.extent.join(" × ")} 格${target.outOfBoundsGroups.length?"\n越界分组："+target.outOfBoundsGroups.join("、"):""}` : "";
  const catalog=s.materialCompatibility;
  $("materialCompatibility").textContent=catalog ? (catalog.status==="unknown"?"未声明目标目录，未核实上传的Avatar。":catalog.matched?`目录匹配${catalog.runtimeMaterials?" / 支持实时材质":""} · ${catalog.usedStyles} 种已用材质 · ${catalog.fingerprint.slice(0,12)}\n用户导入的目录，非远端确认。`:`目录不匹配：缺少 [${catalog.missingIds.join(",")}]，定义不同 [${catalog.differentIds.join(",")}]\n${catalog.note||"请更新Avatar目录后再发布。"}`) : "";
  setChecked("autoCenterOnFinish",s.project.autoCenterOnFinish);
  if(pendingTransform) $("transformApply").disabled=pendingTransform.baseRevision!==s.revision;
  $("renderDiagnostics").textContent=`实际 WebGL 抗锯齿样本：${renderer.renderer.getContext().getParameter(renderer.renderer.getContext().SAMPLES)} · 内部透明面 ${renderer.surfaceStats?.hiddenFaces||0} 个\n${renderer.textureWarnings.join("\n")}`;
  $("undo").disabled = !s.canUndo;
  $("redo").disabled = !s.canRedo;
  setValue("profile", connection.profile);
  setValue("targetPort", connection.port);
  setValue("interval", connection.interval);
  setValue("targetCapacity", connection.capacity);
  setValue("refreshSeconds", connection.refreshSeconds);
  setValue("sendOrder",connection.sendOrder||"volume");
  setChecked("loopReplay",connection.loopReplay||false);
  $("loopReplay").disabled=!["mf5","mf6"].includes(connection.profile);
  $("sendOrder").disabled=!["mf5","mf6"].includes(connection.profile);
  $("replayStatus").textContent=`已完成补播 ${connection.replayCycles||0} 轮 · ${connection.repairing?"正在补播":connection.loopReplay?(connection.enabled?"连续补播已启用":"循环已设置，发送关闭"):"按需发送"}。${connection.repairDeletions?.some(n=>n)?`含 ${connection.repairDeletions.reduce((a,b)=>a+b,0)} 条历史删除。`:""}更新先删除旧几何，再按所选顺序绘制；没有远端ACK。`;
  $("repairPublish").disabled=!["mf5","mf6"].includes(connection.profile);
  setChecked("autoPublish", connection.auto);
  $("connect").textContent = connection.enabled ? "停止发送" : "开启发送";
  $("connect").classList.toggle("selected", connection.enabled);
  $("connectionBadge").textContent = connection.enabled
    ? "OSC " + connection.port + " · " + connection.pending + "项"
    : "独立创作";
  $("connectionBadge").classList.toggle("live", connection.enabled);
  $("syncStatus").textContent =
    connection.error ||
    (!connection.enabled
      ? "发送已关闭；可继续独立创作。"
      : `本机发包中 · 无远端ACK
${connection.profile.toUpperCase()} · 几何 ${connection.syncedBits}bit${["mf4","mf5","mf6"].includes(connection.profile)?" · 本体仅本地":""}
目标 v${connection.desiredRevision} / 已发送 v${connection.sentRevision ?? "—"}
待发 ${fmt(connection.pending)} 批 · 预计 ${time(connection.etaSeconds)}
累计 ${fmt(connection.sentCommands)} 批`);
  setChecked("world", connection.placement.world);
  setChecked("visible", connection.placement.visible);
  setChecked("level", connection.placement.level);
  setValue("scale", Math.log10(connection.placement.scale/.0005));
  const percent=connection.placement.scale/.05*100;
  setValue("scalePercent",Number(percent.toFixed(2)));
  $("scaleValue").value=Number(percent.toFixed(1))+"%";
  $("scale").min=["mf5","mf6"].includes(connection.profile)?"0":"2";
  $("scalePercent").min=["mf5","mf6"].includes(connection.profile)?"1":"100";
  $("scale").max=["mf5","mf6"].includes(connection.profile)?"3":String(Math.log10(2000));
  $("scalePercent").max=["mf5","mf6"].includes(connection.profile)?"1000":"2000";
  $("scaleHint").textContent=`新100%＝旧5% · 当前格长 ${(0.25*connection.placement.scale*1000).toFixed(3)} 毫米。${["mf5","mf6"].includes(connection.profile)?"1%～1000%，对数刻度，便于微缩。":"旧头像不支持100%以下；请使用MF5配套头像。"}`;
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
  $("activeModel").textContent =
    `${harness.settings.model} · ${harness.provider.apiType === "chat" ? "Chat" : "Responses"}`;
  const usage = usageDisplay(harness.usage);
  $("usageTokens").textContent = usage.tokens;
  $("usageTokens").title = usage.tokenTitle;
  $("usagePrefix").textContent = usage.prefix;
  $("usagePrefix").title = usage.prefixTitle;
  $("usageRun").textContent = usage.run;
  $("usageRun").hidden = !usage.run;
  setChecked("sendImages", harness.settings.sendImages ?? true);
  setChecked("goalMode", harness.settings.goalMode ?? false);
  $("presetHint").textContent =
    ({
      sketch: "轮廓与大形优先",
      standard: "结构、材质与有限细节",
      detailed: "分阶段细化与多轮复核",
    }[harness.settings.preset] || "创作预设") +
    ` · 当前最多${harness.settings.maxSteps}步 / ${Math.round(harness.settings.maxRunSeconds / 60)}分钟，可在预算范围内调整。`;
  const goal = harness.context.goal;
  const goalLabels = {
    active: "进行中",
    achieved: "已保存完成",
    blocked: "等待调整",
    budget_exhausted: "预算已用尽",
    stopped: "已停止",
  };
  $("goalStatus").textContent = goal
    ? `${goalLabels[goal.status] || goal.status} · 复核 ${goal.reviewedGeometryHashes.length}/${goal.minReviewRounds} · 自动续作 ${goal.continuations}\n${goal.objective}\n${goal.assessment || ""}`
    : "尚无创作目标";
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
  const monitor = harness.cacheMonitor || {};
  $("cacheMonitorStatus").textContent =
    `前缀 ${monitor.prefix?.appendOnly === false ? "发生重置/变化" : monitor.prefix?.appendOnly === true ? "保持追加" : "待观察"} · 最近缓存 ${typeof monitor.recentHitRate === "number" ? (monitor.recentHitRate * 100).toFixed(1) + "%" : "未报告"} · ${monitor.alert || (monitor.mode ? (monitor.cacheRequired ? "严格监测" : "观察模式") : "等待首次请求；保护按配置启用")}`;
  $("cacheMonitorStatus").classList.toggle("kind-error", !!monitor.alert);
  $("cacheLiveStatus").textContent = $("cacheMonitorStatus").textContent;
  $("cacheLiveStatus").classList.toggle("kind-error", !!monitor.alert);
  $("cacheShowConfig").hidden = !monitor.alert;
  setValue("maxSteps", harness.settings.maxSteps);
  setValue("maxRunSeconds", harness.settings.maxRunSeconds);
  setValue("maxRunTokens", harness.settings.maxRunTokens);
  setValue("maxOutputTokens", harness.settings.maxOutputTokens);
  if (harness.modelLimits) {
    const limits = harness.modelLimits;
    $("modelLimitsInfo").textContent =
      `有效上下文 ${fmt(limits.effectiveContextWindow)} · 输出上限 ${fmt(limits.effectiveOutputCeiling)} Token（${limits.source === "upstream_models" ? "已读取上游 /models" : "按当前配置；尚未获取上游限制"}，单次输出按剩余窗口裁剪）`;
  }
  setValue("compactTokens", harness.settings.compactTokens);
  setValue("language", harness.settings.language || "zh-CN");
  setChecked("autoCompact", harness.settings.autoCompact);
  setValue("pinnedNotes", s.project.pinnedNotes);
  $("providerState").textContent = harness.provider.configured
    ? "连接已配置"
    : "需要配置Key";
  $("providerState").classList.toggle("live", harness.provider.configured);
  $("contextStats").textContent =
    `上下文约 ${fmt(harness.context.estimatedTokens)} Token\n` +
    `${harness.context.messages} 条消息 · compact ${harness.context.compactCount} 次`;
  $("contextSummary").textContent = harness.context.summary || "暂无";
  syncHarness(harness);
  $("renderInfo").textContent =
    `v${loadedRevision < 0 ? s.revision : loadedRevision} · ` +
    `${renderer.renderer.info.render.calls} 绘制调用 · 底部中心 / +Z前`;
}

function edit(value) {
  edits = edits.then(async () => {
    try {
      const result = await api("edit", {
        ...value,
        baseRevision: state?.revision,
        requestId: crypto.randomUUID(),
      });
      if (result.warning) toast(result.warning);
      await refresh();
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
    button.title =
      material.name +
      " · " +
      material.family +
      (material.pack ? " · Avatar需预打包" : "");
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
  $("styleName").textContent = selectedMaterial?.name || "未知材质";
  $("textStyleName").textContent = selectedMaterial?.name || "未知材质";
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
    check.title = `显示“${name}”的本地预览`;
    check.setAttribute("aria-label", `显示分组 ${name}`);
    check.onchange = async () => {
      if (check.checked) renderer.groupsHidden.delete(name);
      else renderer.groupsHidden.add(name);
      await renderer.load(sceneData.scene, sceneData.materials, {
          surfaces: sceneData.surfaces,
        mode: $("renderMode").value,
      });
    };
    const label = document.createElement("span");
    label.textContent = name;
    label.onclick = () => {
      selected = new Set(
        sceneData.scene.boxes.concat(sceneData.scene.triangles||[])
          .filter((box) => box.group === name)
          .map((box) => box.id),
      );
      renderer.selection = selected;
      renderer.updateSelection();
      selectionInfo();
    };
    const number = document.createElement("small");
    number.textContent = count;
    const protection = document.createElement("button");
    const protectedGroup = isGroupProtected(
      state?.project?.protectedGroups,
      name,
    );
    protection.className = `group-protect${protectedGroup ? " selected" : ""}`;
    protection.dataset.group = name;
    protection.type = "button";
    protection.setAttribute("aria-pressed", String(protectedGroup));
    protection.title = protectedGroup
      ? `“${name}”已受保护；点击解除后再编辑`
      : `保护“${name}”，阻止绘制、擦除和变换`;
    protection.textContent = protectedGroup ? "已保护" : "保护";
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
  button.textContent = "保存中";
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
      throw new Error("分组保护未保存，当前服务端可能尚未启用该设置");
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
        ? `已保护分组“${name}”；绘制、擦除和变换会被拒绝`
        : `已解除分组“${name}”的保护，现在可以编辑`,
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
  $("selectionInfo").textContent = selected.size
    ? `已选 ${selected.size} 个对象 · 移动/复制读取精确位置XYZ作为偏移`
    : "选择工具点击，Shift多选；拖动框选";
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
    if (!state || loading) return;
    const hit = renderer.pick(lastPointer, {
      erase: tool === "erase" || lastPointer.shiftKey,
      layer: Number($("layer").value),
    });
    if (tool !== "select") {
      renderer.hover(hit, brush(), tool === "erase" || lastPointer.shiftKey);
    }
    if (hit) {
      $("cursorPosition").textContent =
        `X ${hit.pos[0]} · Y ${hit.pos[1]} · Z ${hit.pos[2]}`;
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
  if (event.button === 0) pointerStart = [event.clientX, event.clientY];
});
renderer.renderer.domElement.addEventListener("pointerup", (event) => {
  if (event.button !== 0 || !pointerStart || loading) return;
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
    } else {
      const hit = renderer.pick(event, { erase: true });
      if (hit?.box || hit?.triangle) {
        const id=(hit.box||hit.triangle).id;
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
  if(hit?.triangle&&(tool==="erase"||event.shiftKey))edit({op:"remove_triangles",ids:[hit.triangle.id]});
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
    if (!selected.size) return toast("请先选择内容");
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
  if (!selected.size) return toast("请先选择内容");
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
  edit({ operations });
};

$("fitView").onclick = () => renderer.view();
document.querySelectorAll("[data-view]").forEach((button) => {
  button.onclick = () => renderer.view(button.dataset.view);
});
$("renderMode").onchange = () => refresh(true);
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
    $("workspaceError").textContent = "";
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
      const label = document.createElement("span");
      label.className = "grow";
      label.textContent = item.name || item.title || "未命名";
      const detail = document.createElement("span");
      detail.className = "caption";
      detail.textContent = action === "open_project" ? `${fmt(item.boxCount)} Box · v${item.revision}` : `${fmt(item.messages)} 条上下文 · ${new Date(item.createdAt).toLocaleString()}`;
      const open = document.createElement("button");
      open.textContent = item.id === active ? "当前" : "打开";
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
  $("workspaceError").textContent = "";
  try {
    await edits;
    await api("workspaces", { ...request, baseProjectId: state.project.id, baseSessionId: state.harness.context.id });
    // Reinitialize per-context drafts, selection, history and polling together.
    location.reload();
  } catch (cause) {
    $("workspaceError").textContent = cause.message;
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
    $("transformReport").textContent=`偏移 ${result.offset.join(", ")} 格\n原范围 ${JSON.stringify(result.before.bounds)}\n新范围 ${JSON.stringify(result.after.bounds)}\n${result.after.fits?"坐标和容量可发布":"目标预检仍未通过，请检查尺寸/容量"}`;
    $("transformApply").disabled=false;
  }catch(cause){$("transformReport").textContent=cause.message;error(cause);}
}
$("transformPreview").onclick=previewTransform;
for(const id of ["transformAction","shiftX","shiftY","shiftZ"]) $(id).oninput=()=>{pendingTransform=null;$("transformApply").disabled=true;};
$("transformApply").onclick=async()=>{if(!pendingTransform)return;try{await api("harness/tool",{name:"transform_scene",arguments:pendingTransform});pendingTransform=null;$("transformDialog").close();await refresh(true);renderer.view();toast("整体定位已保存，可撤销");}catch(cause){error(cause);}};
$("schemaUpgradeContext").onclick=async()=>{try{await api("harness/control",{action:"upgrade_context"});await refresh();toast("已升级工具说明；首次请求重新预热，实际缓存仍持续监测");}catch(cause){error(cause);}};
$("autoCenterOnFinish").onchange=async()=>{try{await api("project",{autoCenterOnFinish:$("autoCenterOnFinish").checked});await refresh();}catch(cause){error(cause);}};
function runtimeFields(){const kind=$("runtimeKind").value,proc=["water","cloud","wood","stone"].includes(kind);$("runtimeSurfaceFields").hidden=kind==="color";$("runtimeNoiseFields").hidden=!proc;$("runtimeMetal").disabled=proc;$("runtimeWireHint").textContent=kind==="color"?"RGB24精确颜色；MF5直接发送，无需重新上传头像。":kind==="pbr"?"VRC使用RGB565；金属度/粗糙度/透明度/发光各4档。艺术原值保持。":"VRC颜色RGB444；噪声比例和种子各8档，其他表面参数4档。";}
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
$("imageToolInspect").onclick=async()=>{try{const args=imageToolArgs("inspect");args.points=[[0,0]];args.grid=[8,8];const value=await api("harness/tool",{name:"image_tools",arguments:args});$("imageToolReport").textContent=JSON.stringify(value,null,2);}catch(cause){error(cause);}};
$("imageToolResize").onclick=async()=>{try{const value=await api("harness/tool",{name:"image_tools",arguments:imageToolArgs("resize")});await imageToolsCatalog(value.id);$("imageToolReport").textContent=JSON.stringify({id:value.id,width:value.width,height:value.height,parentImageId:value.parentImageId},null,2);toast("缩放副本已加入图库，原图保留");}catch(cause){error(cause);}};
$("imageToolPreview").onclick=async()=>{try{const args=imageToolArgs("voxel_plane");const value=await api("harness/tool",{name:"image_tools",arguments:args});imagePlanePreview={...args,dryRun:false};imagePlaneProjectId=state.project.id;$("imageToolPreviewImage").src=value.previewUrl;$("imageToolPreviewImage").hidden=false;$("imageToolReport").textContent=JSON.stringify({size:value.size,generatedBoxes:value.generatedBoxes,paletteColors:value.paletteColors,materialsAdded:value.materialsAdded,target:value.target,canvasMode:value.canvasMode,canvasReplacesRegion:value.canvasReplacesRegion},null,2);$("imageToolApply").disabled=false;}catch(cause){invalidateImagePlane();error(cause);}};
$("imageToolApply").onclick=async()=>{if(!imagePlanePreview)return;if(imagePlaneProjectId!==state.project.id){invalidateImagePlane();return toast("工程已切换，请重新预览图片平面");}$("imageToolApply").disabled=true;try{const result=await api("harness/tool",{name:"image_tools",arguments:imagePlanePreview});invalidateImagePlane();await refresh(true);renderer.view("front");toast(`图片平面已应用 · v${result.revision}，可撤销`);$("imageToolsDialog").close();}catch(cause){invalidateImagePlane();error(cause);}};
$("runtimeMaterialOpen").onclick=()=>{runtimeFields();$("runtimeReceipt").textContent="";$("runtimeMaterialDialog").showModal();};
$("runtimeMaterialClose").onclick=()=>$("runtimeMaterialDialog").close();
$("runtimeMaterialApply").onclick=async()=>{const button=$("runtimeMaterialApply");button.disabled=true;try{const kind=$("runtimeKind").value,entry={name:$("runtimeName").value,color:$("runtimeColor").value,kind};if(kind!=="color")Object.assign(entry,{metallic:Number($("runtimeMetal").value),roughness:Number($("runtimeRough").value),opacity:Number($("runtimeOpacity").value),emission:Number($("runtimeEmission").value)});if(["water","cloud","wood","stone"].includes(kind))Object.assign(entry,{noiseScale:Number($("runtimeNoiseScale").value),seed:Number($("runtimeSeed").value)});const r=await api("harness/tool",{name:"define_materials",arguments:{baseRevision:state.revision,materials:[entry]}});selectedStyle=r.materials[0].id;$("runtimeReceipt").textContent=JSON.stringify(r.materials[0].vrcEffective,null,2);await refresh(true);toast("材质已添加并选用；MF5实时发送");}catch(cause){error(cause);}finally{button.disabled=false;}};
$("pixelThemeOpen").onclick=()=>$("pixelThemeDialog").showModal();
$("pixelThemeClose").onclick=()=>$("pixelThemeDialog").close();
$("applyPixelTheme").onclick=async()=>{try{const result=await api("harness/tool",{name:"builtin_materials",arguments:{action:"apply",id:$("pixelThemeChoice").value,baseRevision:state.revision,replaceBaseMaterials:$("pixelThemeReplace").checked}});await refresh(true);$("pixelThemeDialog").close();toast(`像素主题已加入 · 新增${result.materialsAdded}种材质`);}catch(cause){error(cause);}};
$("avatarCatalogImport").onclick=()=>$("avatarCatalogFile").click();
$("avatarCatalogFile").onchange=async(event)=>{try{const file=event.target.files[0];if(!file)return;const form=new FormData();form.append("file",file);const r=await fetch("/api/vrc/profile/import",{method:"POST",body:form});const value=await r.json();if(!r.ok)throw new Error(value.error);await refresh();toast(value.matched?"目标目录与已用材质匹配":"已记录目标目录，存在材质差异");}catch(cause){error(cause);}event.target.value="";};
$("avatarCatalogMf6").onclick=async()=>{try{await api("vrc/profile",{action:"bundled_mf6"});await api("connect",{profile:"mf6",enabled:false,scale:Math.min(.5,Math.max(.0005,state.connection.placement.scale))});await refresh(true);toast("已载入 MF6 16K 目录并选择统一240bit协议，发送保持关闭");}catch(cause){error(cause);}};
$("avatarCatalogBundled").onclick=async()=>{try{await api("vrc/profile",{action:"bundled_mf5"});await api("connect",{profile:"mf5",enabled:false,scale:Math.min(.5,Math.max(.0005,state.connection.placement.scale))});await refresh(true);toast("已载入MF5目录；请使用配套MF5头像");}catch(cause){error(cause);}};
$("avatarCatalogMf4").onclick=async()=>{try{await api("vrc/profile",{action:"bundled_mf4"});await api("connect",{profile:"mf4",enabled:false,scale:Math.max(.05,state.connection.placement.scale)});await refresh(true);}catch(cause){error(cause);}};
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
$("catalogExport").onclick = () => {
  const link = document.createElement("a");
  link.href = "/api/vrc/catalog";
  link.download = "morphforge-materials.zip";
  link.click();
};
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
    toast(`已导入 ${value.materialsAdded} 种材质；Avatar使用需预打包`);
    $("packReportContent").textContent = JSON.stringify(value, null, 2);
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
    loopReplay: ["mf5","mf6"].includes($("profile").value) && $("loopReplay").checked,
    sendOrder: $("sendOrder").value,
    auto: $("autoPublish").checked,
  };
}

async function configure(enabled) {
  try {
    await api("connect", connection(enabled));
    await refresh();
  } catch (cause) {
    error(cause);
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
  try {
    await api("publish", {});
    await refresh();
    toast(
      state.connection.enabled
        ? "版本已进入发送队列"
        : "版本已准备；开启发送后执行",
    );
  } catch (cause) {
    error(cause);
  }
};
$("repairPublish").onclick=async()=>{try{await api("publish",{repair:true});await refresh();toast("已排入补播，保留现有显示；没有远端ACK。若有残留请清空并重建。");}catch(cause){error(cause);}};
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

for (const id of ["world", "visible", "level"]) {
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
    for (const name of ["ai", "text", "sync", "context"]) {
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
for (const key of ["sendImages", "goalMode"]) {
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
  $("providerImageHint").textContent = supported
    ? "DeepSeek官方Responses可选择Files引用；默认内嵌图片也可使用。"
    : "此接口自动内嵌图片，不调用专用上传通道。";
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
$("settingsOpen").onclick = openSettings;
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
    toast("API连接已保存");
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
    return toast("请输入要发送的内容");
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
  if (messageInFlight || (referencePicker.busy && prompt?.trim())) return;
  const snapshot = normalizeHarness(state?.harness);
  const submittedPrompt = prompt ?? "";
  if (!submittedPrompt && !(mode === "interrupt" && snapshot.active)) {
    return toast("空消息只能用于立即打断");
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
      toast(merged.error || "消息未被接受，输入仍保留");
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
        note: "已接收，等待安全点",
      });
    } else if (intent === "interrupt") {
      addUserChat({
        id: `message:${payload.requestId}`,
        text: "暂停并取消当前请求",
        runId: merged.runId,
        epoch: merged.epoch,
        mode,
        note: "已接收，等待取消当前请求",
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
      await api("harness/control", { action });
      await refresh();
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
    toast("固定要求已保存");
  } catch (cause) {
    error(cause);
  }
};

async function runRecipe(dryRun) {
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
        `预计 ${value.stats.boxCount} Box / ${fmt(value.stats.voxelCount)} 格`,
      );
    } else {
      await refresh();
      renderer.view();
    }
  } catch (cause) {
    error(cause);
  }
}
$("runRecipe").onclick = () => runRecipe(false);
$("dryRecipe").onclick = () => runRecipe(true);

async function capture() {
  const result = await api("harness/tool", {
    name: "capture_views",
    arguments: { revision: state.revision, views: ["hero", "front", "top"], mode: $("renderMode").value, quality: viewSettings.captureQuality, voxelGrid: viewSettings.captureGrid, surfaceCulling: viewSettings.surfaceCulling },
  });
  window.__morphforge.lastCapture = result;
  $("captureRevision").textContent = "v" + result.revision;
  $("captureImages").replaceChildren();
  for (const image of result.images) {
    const wrap = document.createElement("div");
    const img = document.createElement("img");
    const caption = document.createElement("p");
    img.src = image.url;
    caption.textContent = image.view;
    wrap.append(img, caption);
    $("captureImages").append(wrap);
  }
  return result;
}
$("capture").onclick = async () => {
  try {
    toast("正在生成独立三视图…");
    await capture();
    $("captureDialog").showModal();
  } catch (cause) {
    error(cause);
  }
};
$("captureClose").onclick = () => $("captureDialog").close();
$("saveArtwork").onclick = async () => {
  try {
    const images = await capture();
    await api("artworks", {
      action: "save",
      title: state.project.name,
      tags: [],
      thumbnail: images.images[0]?.artifact,
      views: images.images.map((image) => image.artifact),
    });
    toast("作品和三视图已保存");
  } catch (cause) {
    error(cause);
  }
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
    name.textContent = artwork.title;
    info.textContent = `${artwork.origin === "builtin" ? "内置示例" : "我的作品"} · ${fmt(artwork.boxCount)} Box · ${fmt(artwork.triangleCount||0)} 面 · ${fmt(artwork.voxelCount)} 格`;
    load.textContent =
      artwork.origin === "builtin" ? "载入可编辑副本" : "载入并继续";
    body.append(name, info);
    if (artwork.description) {
      const description = document.createElement("p");
      description.textContent = artwork.description;
      body.append(description);
    }
    if (artwork.sourceCredits?.length) {
      const credits = document.createElement("p");
      credits.className = "caption";
      credits.textContent = artwork.sourceCredits
        .map((credit) => `${credit.author} · ${credit.license}`)
        .join("；");
      body.append(credits);
    }
    if (artwork.views?.length) {
      const preview = document.createElement("button");
      preview.textContent = "多视图";
      preview.onclick = () => {
        $("captureRevision").textContent = artwork.title;
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
    toast("已确认告警；恢复运行后将继续监测");
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
      label.textContent = "v" + revision;
      const commit = history.commits.find((item) => item.revision === revision);
      const title = document.createElement("span");
      title.className = "grow";
      title.textContent = commit?.title || "草稿";
      const restore = document.createElement("button");
      restore.textContent = "从此版本继续";
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
      option.textContent = font.name;
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
      option.textContent = "字体服务没有可用字体";
      select.append(option);
    }
    syncTextButtons();
    return fontsLoaded;
  } catch (cause) {
    fontsLoaded = false;
    $("textMetrics").textContent = "字体服务尚未就绪：" + friendlyError(cause);
    $("textMetrics").className = "status error";
    syncTextButtons();
    return false;
  }
}

$("textFontRefresh").onclick = () => loadFonts(true);

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
  $("textMetrics").textContent = "预览已清除。草稿没有变化。";
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
    return toast("请输入文字");
  }
  const fingerprint = textRequestFingerprint(request);
  const sequence = ++previewSequence;
  const previewRequestId = newRequestId();
  $("textPreview").disabled = true;
  $("textMetrics").textContent = "预览计算中…";
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
      throw new Error("文字预览状态校验失败，请重新预览。");
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
      throw new Error("文字预览结果无法校验，请重新预览。");
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
      `尺寸 ${dimensionText} 格`,
      `${fmt(metrics.boxCount)} Box · ${fmt(metrics.voxelCount)} 体素`,
      `目标容量 ${fmt(state?.stats?.targetBoxes)} Box`,
    ];
    if (metrics.missing.length)
      lines.push("缺字：" + metrics.missing.join(" "));
    if (metrics.overCapacity) lines.push("超出目标材料容量");
    if (metrics.warnings.length) lines.push(...metrics.warnings);
    if (!boxes.length) lines.push("预览没有返回 Box");
    $("textMetrics").textContent = lines.join("\n");
    $("textMetrics").className =
      metrics.missing.length || metrics.overCapacity
        ? "status error"
        : "status ok";
    if (manual) toast("预览已更新，草稿未修改");
  } catch (cause) {
    if (sequence !== previewSequence) return;
    currentTextPreview = null;
    currentPreviewSentJSON = "";
    currentPreviewServerFingerprint = "";
    currentPreviewRequestId = "";
    currentPreviewRequest = null;
    renderer.clearTextPreview();
    $("textMetrics").textContent = formatPreviewError(cause);
    $("textMetrics").className = "status error";
    if (manual && !formatPreviewError(cause).includes("缺字")) error(cause);
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
    return toast("请先预览当前参数；缺字或超容量时不能应用");
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
        ? `文字已应用 · v${applied.revision} · ${applied.warning}`
        : `文字已应用 · v${applied.revision}`,
    );
  } catch (cause) {
    error(cause);
    if (cause.status === 409) {
      $("textMetrics").textContent =
        "草稿版本已变化。预览未修改草稿，请重新预览后再应用。";
      $("textMetrics").className = "status error";
    }
  } finally {
    textApplyBusy = false;
    syncTextButtons();
  }
};

setupMinecraftUi({$,api,getState:()=>state,getMaterials:()=>sceneData?.materials,refresh,error,toast});

setupMeshUi({$,getScene:()=>sceneData,getState:()=>state,getSelected:()=>selected,getSelectedStyle:()=>selectedStyle,api,edit,refresh,error});

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
        });
        const images = await renderer.capture(
          job.views,
          job.width,
          job.height,
          job.shots,
          job,
        );
        await api("capture/result", {
          id: job.id,
          revision: renderer.renderRevision,
          images,
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
  setInterval(()=>activityView.load().catch(error),1500);
}
