import assert from "node:assert/strict";
import test from "node:test";
import {
  completedRunSummary,
  conflictFromError,
  dedupeMessages,
  eventMessageState,
  findExactAssistantMessage,
  friendlyError,
  mergeAck,
  messageModeHint,
  normalizeHarness,
  phaseLabel,
  primaryAction,
  statusLabel,
} from "./harness-ui.js";

const running = {
  run: { id: "run-1", status: "running", epoch: 4 },
  paused: false,
  interaction: {
    phase: "running",
    epoch: 4,
    runId: "run-1",
    activeRequestId: "req-live",
    paused: false,
    queue: [{ requestId: "req-queued", mode: "queue" }],
  },
};

test("interaction exposes epoch, active request, queue and live run", () => {
  const value = normalizeHarness(running);
  assert.equal(value.active, true);
  assert.equal(value.epoch, 4);
  assert.equal(value.activeRequestId, "req-live");
  assert.equal(value.queue.length, 1);
});

test("primary action names the boundary for every mode", () => {
  const value = normalizeHarness(running);
  assert.equal(primaryAction(value, "steer").label, "插话 · 下一安全点");
  assert.equal(primaryAction(value, "interrupt").label, "打断并继续");
  assert.equal(primaryAction(value, "queue").label, "排队下一轮");
  assert.equal(
    primaryAction({ ...value, active: false }, "queue").label,
    "开始创作",
  );
  assert.match(messageModeHint(value, "interrupt"), /取消当前模型请求/);
});

test("ACK merging is explicit and queue position survives", () => {
  const pending = {
    requestId: "req-a",
    runId: "run-1",
    epoch: 4,
    status: "sending",
    queuePosition: null,
  };
  const accepted = mergeAck(pending, {
    ok: true,
    accepted: true,
    requestId: "req-a",
    runId: "run-1",
    epoch: 5,
    phase: "running",
    queuePosition: 2,
  });
  assert.equal(accepted.status, "accepted");
  assert.equal(accepted.epoch, 5);
  assert.equal(accepted.queuePosition, 2);
  assert.equal(
    mergeAck(pending, { ok: true, accepted: false }).status,
    "rejected",
  );
});

test("message events map to durable terminal states", () => {
  assert.equal(eventMessageState("harness.message.accepted"), "accepted");
  assert.equal(eventMessageState("harness.message.applied"), "applied");
  assert.equal(eventMessageState("harness.message.cancelled"), "cancelled");
  assert.equal(statusLabel("applied"), "已应用");
  assert.equal(conflictFromError({ status: 409, message: "busy" }), false);
  assert.equal(conflictFromError({ message: "RUN_CONFLICT: stale" }), true);
  assert.equal(conflictFromError({ status: 400, message: "bad" }), false);
});

test("completed harness.finished exposes the Run summary by runId", () => {
  const summary = completedRunSummary({
    kind: "harness.finished",
    runId: "run-final",
    data: {
      id: "run-final",
      status: "completed",
      epoch: 7,
      message: "作品完成，旧建筑保持不变。",
    },
  });
  assert.deepEqual(summary, {
    runId: "run-final",
    epoch: 7,
    message: "作品完成，旧建筑保持不变。",
  });
  assert.equal(
    completedRunSummary({
      kind: "harness.finished",
      runId: "run-final",
      data: { status: "failed", message: "失败" },
    }),
    null,
  );
  assert.equal(
    completedRunSummary({
      kind: "harness.finished",
      runId: "run-final",
      data: { status: "completed", message: "  " },
    }),
    null,
  );
});

test("identical final summary is found across stored chat entries", () => {
  const entries = [
    {
      id: "assistant:run-final:7",
      role: "assistant",
      runId: "run-final",
      text: "旧片段",
    },
    {
      id: "final:run-final:7",
      role: "assistant",
      runId: "run-final",
      text: "  作品完成，旧建筑保持不变。 ",
      final: true,
    },
  ];
  assert.equal(
    findExactAssistantMessage(entries, {
      runId: "run-final",
      message: "作品完成，旧建筑保持不变。",
    })?.id,
    "final:run-final:7",
  );
  assert.equal(
    findExactAssistantMessage(entries, {
      runId: "other-run",
      message: "作品完成，旧建筑保持不变。",
    }),
    null,
  );
});

test("user-facing errors translate protocol identifiers", () => {
  assert.match(
    friendlyError({ status: 409, message: "RUN_CONFLICT: stale" }),
    /运行状态已经变化/,
  );
  assert.match(
    friendlyError({
      message: "REQUEST_ID_CONFLICT：requestId已用于不同消息",
    }),
    /已经提交过/,
  );
  assert.match(
    friendlyError({ message: "文字应用响应 requestId 不匹配" }),
    /状态校验未通过/,
  );
  assert.match(
    friendlyError({
      status: 409,
      message: "PREVIEW_CONFLICT：文字设置已改变，请重新预览",
    }),
    /重新预览/,
  );
  assert.match(
    friendlyError({
      status: 409,
      message: "PROTECTED_GROUP_CONFLICT：分组 主楼 已受保护",
    }),
    /已锁定.*解除保护/,
  );
  assert.doesNotMatch(
    friendlyError({ message: "requestId mismatch" }),
    /requestId/,
  );
});

test("interrupting remains a visible active phase", () => {
  const value = normalizeHarness({
    run: { id: "run-1", status: "interrupting", epoch: 5 },
    interaction: {
      phase: "interrupting",
      epoch: 5,
      runId: "run-1",
      queue: [],
    },
  });
  assert.equal(value.active, true);
  assert.equal(phaseLabel(value), "正在打断");
});

test("rapid messages are deduplicated by requestId and keep the latest state", () => {
  const values = dedupeMessages([
    { requestId: "a", status: "accepted", createdAt: "2026-01-01T00:00:00Z" },
    { requestId: "b", status: "accepted", createdAt: "2026-01-01T00:00:01Z" },
    { requestId: "a", status: "applied", createdAt: "2026-01-01T00:00:02Z" },
  ]);
  assert.deepEqual(
    values.map((value) => [value.requestId, value.status]),
    [
      ["a", "applied"],
      ["b", "accepted"],
    ],
  );
});
