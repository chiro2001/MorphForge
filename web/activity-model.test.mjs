import test from "node:test";
import assert from "node:assert/strict";
import { projectActivity } from "./activity-model.js";
import { ScrollFollow } from "./scroll-follow.js";
const row = (seq, kind, body = {}, extra = {}) => ({
  seq,
  kind,
  body,
  runId: "run",
  revision: 4,
  time: "2026-10-20T12:00:00Z",
  ...extra,
});
test("stream deltas become one thought and one body without repeating the completed response", () => {
  const cards = projectActivity([
    row(1, "provider.request", {}, { requestId: "q" }),
    row(2, "provider.reasoning_delta", { text: "检查" }),
    row(3, "provider.reasoning_delta", { text: "比例" }),
    row(4, "provider.text_delta", { text: "做好了" }),
    row(
      5,
      "provider.response",
      { text: "做好了", reasoning: "检查比例" },
      { requestId: "q" },
    ),
  ]);
  assert.equal(cards.length, 1);
  assert.equal(cards[0].thought, "检查比例");
  assert.equal(cards[0].text, "做好了");
  assert.equal(cards[0].responseSeq, 5);
});
test("tool requests/results join by call ID and repeated screenshots stay unique", () => {
  const image = { artifact: "a.png", url: "/artifacts/a.png", view: "hero" };
  const cards = projectActivity([
    row(1, "provider.request", {}, { requestId: "q" }),
    row(
      2,
      "tool.request",
      { name: "capture_views", summary: "观察三个角度" },
      { callId: "c" },
    ),
    row(3, "capture.completed", {}, { images: [image] }),
    row(
      4,
      "tool.result",
      { name: "capture_views", elapsedMs: 100, result: { ok: true } },
      { callId: "c", images: [image] },
    ),
  ]);
  assert.equal(cards.length, 1);
  assert.equal(cards[0].tools.length, 1);
  assert.equal(cards[0].tools[0].title, "观察三个角度");
  assert.equal(cards[0].tools[0].status, "完成");
  assert.equal(cards[0].images.length, 1);
});
test("interleaved runs and retry requests never merge reasoning across requests", () => {
  const cards = projectActivity([
    row(1, "provider.request", {}, { requestId: "a" }),
    row(2, "provider.reasoning_delta", { requestId: "a", text: "旧" }),
    row(3, "provider.request", {}, { requestId: "b", runId: "run2" }),
    row(
      4,
      "provider.reasoning_delta",
      { requestId: "b", text: "新" },
      { runId: "run2" },
    ),
    row(5, "provider.cancelled", { message: "已打断" }),
  ]);
  assert.equal(cards[0].thought, "旧");
  assert.equal(cards[1].thought, "新");
  assert.equal(cards[2].type, "notice");
});
test("completion errors and user interjections remain visible in sequence", () => {
  const cards = projectActivity([
    row(1, "harness.message.applied", {
      prompt: "先改窗户",
      mode: "interrupt",
    }),
    row(2, "harness.finished", { status: "failed", error: "预算已到" }),
  ]);
  assert.equal(cards[0].type, "user");
  assert.equal(cards[0].text, "先改窗户");
  assert.equal(cards[1].text, "预算已到");
});
function scroller() {
  const listeners = {};
  const c = {
    scrollTop: 800,
    scrollHeight: 1000,
    clientHeight: 200,
    addEventListener: (name, fn) => (listeners[name] = fn),
  };
  const b = {
    hidden: true,
    addEventListener: (name, fn) => (listeners["button:" + name] = fn),
  };
  return { c, b, listeners, follow: new ScrollFollow(c, b) };
}
test("scrolling away from the bottom survives appended output, then returns on explicit action", () => {
  const { c, b, listeners, follow } = scroller();
  c.scrollTop = 250;
  listeners.scroll();
  const before = follow.snapshot();
  c.scrollHeight = 1500;
  follow.restore(before, { added: 3 });
  assert.equal(c.scrollTop, 250);
  assert.equal(b.hidden, false);
  assert.equal(follow.pending, 3);
  listeners["button:click"]();
  assert.equal(follow.following, true);
  assert.equal(follow.pending, 0);
});
test("loading older pages preserves the visible anchor and disables automatic follow", () => {
  const { c, follow } = scroller();
  const before = follow.snapshot();
  c.scrollHeight = 1400;
  follow.restore(before, { older: true });
  assert.equal(c.scrollTop, 1200);
  assert.equal(follow.following, false);
});
test("durable started/accepted/applied events display the user prompt once", () => {
  const cards = projectActivity([
    row(1, "harness.started", { requestId: "u", prompt: "画一个屋顶" }),
    row(2, "harness.message.accepted", {
      requestId: "u",
      prompt: "画一个屋顶",
      mode: "queue",
    }),
    row(3, "harness.message.applied", { requestId: "u", mode: "queue" }),
  ]);
  assert.equal(cards.length, 1);
  assert.equal(cards[0].text, "画一个屋顶");
  assert.equal(cards[0].status, "已发送");
});
