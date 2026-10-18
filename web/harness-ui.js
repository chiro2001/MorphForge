const TERMINAL_PHASES = new Set([
  "idle",
  "completed",
  "failed",
  "stopped",
  "cancelled",
  "rejected",
]);

export function normalizeHarness(h = {}) {
  const run = h.run || null;
  const interaction = h.interaction || {};
  const status = run?.status || "idle";
  const phase =
    interaction.phase ||
    h.phase ||
    (h.paused ? "paused" : status === "running" ? "running" : status);
  const epoch = Number(interaction.epoch ?? h.epoch ?? run?.epoch ?? 0);
  const runId = interaction.runId ?? h.runId ?? run?.id ?? "";
  const paused = interaction.paused ?? h.paused ?? phase === "paused";
  const queue = Array.isArray(interaction.queue) ? interaction.queue : [];
  const terminalStatus = [
    "completed",
    "failed",
    "stopped",
    "cancelled",
    "rejected",
  ].includes(status);
  const active = !!run && !terminalStatus && !TERMINAL_PHASES.has(phase);
  return {
    run,
    status,
    phase,
    epoch,
    runId,
    activeRequestId: interaction.activeRequestId || "",
    paused,
    queue,
    active,
    stopping: phase === "cancelling" || phase === "stopping",
  };
}

export function primaryAction(snapshot, mode) {
  if (!snapshot.active) return { label: "开始创作", mode: "start" };
  if (mode === "interrupt") return { label: "打断并继续", mode };
  if (mode === "queue") return { label: "排队下一轮", mode };
  return { label: "插话 · 下一安全点", mode: "steer" };
}

export function messageModeHint(snapshot, mode) {
  if (!snapshot.active) return "空闲时发送会开始一轮。";
  if (snapshot.paused)
    return "当前已暂停。点击继续后，消息会按所选方式进入下一安全点或下一轮。";
  if (mode === "interrupt")
    return "取消当前模型请求或未提交工具，并在同一运行中接续这段文本。";
  if (mode === "queue")
    return "保留当前运行；这段文本会在本轮结束后作为下一轮执行。";
  return "保留当前模型输出，在下一个完整工具结果后插入这段文本。";
}

export function phaseLabel(snapshot) {
  const labels = {
    idle: "待命",
    running: "运行中",
    waiting_tool: "等待工具",
    tool: "执行工具",
    interrupting: "正在打断",
    cancelling: "正在打断",
    paused: "已暂停",
    compacting: "整理上下文",
    completed: "已完成",
    failed: "运行中断",
    stopped: "已停止",
    cancelled: "已取消",
  };
  const base = labels[snapshot.phase] || snapshot.status || snapshot.phase;
  return snapshot.paused && snapshot.phase !== "paused"
    ? `${base} · 已暂停`
    : base;
}

export function requestIdFromEvent(event) {
  const data = event?.data || event || {};
  return data.requestId || data.request_id || "";
}

export function eventMessageState(kind) {
  if (!kind) return "";
  if (kind.endsWith(".accepted")) return "accepted";
  if (kind.endsWith(".applied")) return "applied";
  if (kind.endsWith(".cancelled") || kind.endsWith(".canceled"))
    return "cancelled";
  if (kind.endsWith(".rejected") || kind.endsWith(".conflict"))
    return "rejected";
  if (kind.endsWith(".failed")) return "failed";
  return "";
}

export function completedRunSummary(event) {
  if (event?.kind !== "harness.finished") return null;
  const data = event.data || {};
  if (data.status !== "completed") return null;
  const message = typeof data.message === "string" ? data.message : "";
  if (!message.trim()) return null;
  const runId = String(event.runId || data.id || "");
  if (!runId) return null;
  return {
    runId,
    epoch: Number(data.epoch ?? 0),
    message,
  };
}

export function findExactAssistantMessage(entries, summary) {
  const target = String(summary?.message || "").trim();
  if (!target) return null;
  return (
    (entries || []).find(
      (entry) =>
        entry?.role === "assistant" &&
        entry.runId === summary.runId &&
        String(entry.text || "").trim() === target,
    ) || null
  );
}

export function statusLabel(status) {
  return (
    {
      idle: "待命",
      sending: "发送中",
      accepted: "已接收",
      applied: "已应用",
      cancelled: "已取消",
      rejected: "已拒绝",
      failed: "失败",
    }[status] || status
  );
}

export function mergeAck(pending, ack) {
  const accepted =
    ack?.accepted === true ||
    ack?.accepted === "accepted" ||
    (ack?.ok === true && ack?.accepted !== false);
  return {
    ...pending,
    runId: ack?.runId ?? pending.runId,
    epoch: Number(ack?.epoch ?? pending.epoch ?? 0),
    phase: ack?.phase ?? pending.phase,
    queuePosition: ack?.queuePosition ?? pending.queuePosition,
    status: accepted ? "accepted" : "rejected",
    error: accepted ? "" : ack?.error || "服务端未接受消息",
    acknowledgedAt: new Date().toISOString(),
  };
}

export function conflictFromError(error) {
  return (
    String(error?.code || "").includes("RUN_CONFLICT") ||
    String(error?.message || "").includes("RUN_CONFLICT")
  );
}

export function friendlyError(error) {
  const message = String(error?.message || error || "操作未完成");
  const status = Number(error?.status || 0);
  if (/REQUEST_ID_CONFLICT/.test(message)) {
    return "这次操作已经提交过。请先刷新，确认结果后再继续。";
  }
  if (/PROTECTED_GROUP_CONFLICT/.test(message)) {
    return "目标分组已锁定，不能绘制、擦除或变换。请先在左侧解除保护。";
  }
  if (conflictFromError(error)) {
    return "运行状态已经变化，请确认当前运行后重新发送。";
  }
  if (/PREVIEW_CONFLICT/.test(message)) {
    return "文字参数或工程版本已变化，请重新预览后再应用。";
  }
  if (/REVISION_CONFLICT/.test(message) || status === 409) {
    return "工程已被其他修改更新，请刷新后重新尝试。";
  }
  if (/RUN_CANCELLED/.test(message)) {
    return "当前请求已打断，已完成的修改仍会保留。";
  }
  if (/API Key|DeepSeek|provider\.env/i.test(message)) {
    return "尚未配置 AI 服务密钥，请按用户指南完成配置。";
  }
  if (/Failed to fetch|NetworkError|Load failed/i.test(message)) {
    return "无法连接本机服务，请确认 MorphForge 仍在运行。";
  }
  if (
    /requestId|requestFingerprint|previewRequestId|epoch|runId/i.test(message)
  ) {
    return "操作状态校验未通过，请刷新后重试。";
  }
  return message;
}

export function dedupeMessages(messages) {
  const byId = new Map();
  for (const message of messages || []) {
    if (!message?.requestId) continue;
    const previous = byId.get(message.requestId);
    byId.set(
      message.requestId,
      previous
        ? { ...previous, ...message, createdAt: previous.createdAt }
        : message,
    );
  }
  return [...byId.values()].sort(
    (a, b) =>
      new Date(a.createdAt || 0).getTime() -
      new Date(b.createdAt || 0).getTime(),
  );
}
