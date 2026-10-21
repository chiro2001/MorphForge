import { projectActivity } from "/activity-model.js";
import { StreamText } from "/stream-text.js";
import { ScrollFollow } from "/scroll-follow.js";
const el = (tag, text, cls) => {
  const n = document.createElement(tag);
  if (text !== undefined) n.textContent = text;
  if (cls) n.className = cls;
  return n;
};
export class ActivityView {
  constructor(api, onError) {
    this.api = api;
    this.onError = onError;
    this.rows = new Map();
    this.nodes = new Map();
    this.cursor = null;
    this.before = null;
    this.generation = 0;
    this.busy = false;
    this.container = document.getElementById("activityFeed");
    this.follow = new ScrollFollow(
      this.container,
      document.getElementById("activityLatest"),
    );
    document.getElementById("activityOlder").onclick = () =>
      this.load(true).catch(onError);
    document.getElementById("activityImageClose").onclick = () =>
      document.getElementById("activityImageDialog").close();
  }
  reset() {
    this.generation++;
    this.rows.clear();
    this.nodes.clear();
    this.cursor = null;
    this.before = null;
    this.container.replaceChildren();
    this.follow.following = true;
    this.follow.pending = 0;
    this.follow.update();
  }
  async load(older = false) {
    if (this.busy) return;
    this.busy = true;
    const generation = this.generation;
    try {
      // Drain bounded pages so thousands of thought deltas do not hide results.
      let rounds = 0;
      const initial = this.cursor === null && !older;
      let seekBefore = null;
      do {
        const q = new URLSearchParams({ limit: "200", session: "current" });
        if (seekBefore !== null) q.set("before", seekBefore);
        else if (older && this.before !== null) q.set("before", this.before);
        else if (this.cursor !== null) q.set("cursor", this.cursor);
        const data = await this.api("timeline?" + q);
        if (generation !== this.generation) return;
        if (data.reset) this.reset();
        if (older || seekBefore !== null || this.before === null)
          this.before = data.previousCursor;
        if (!older && seekBefore === null) this.cursor = data.nextCursor;
        for (const row of data.events) this.rows.set(row.seq, row);
        if (this.follow.following && this.rows.size > 10000) {
          const sorted = [...this.rows.keys()].sort((a, b) => a - b);
          for (const seq of sorted.slice(0, -10000)) this.rows.delete(seq);
          this.before = this.rows.get(sorted.at(-10000))?.cursor ?? this.before;
        }
        this.render(older);
        document.getElementById("activityOlder").disabled =
          !data.hasOlder || this.before === 0;
        // Other conversations can occupy the entire tail window. Seek the
        // newest matching session once, without resetting the forward cursor.
        if (
          initial &&
          this.nodes.size === 0 &&
          data.hasOlder &&
          ++rounds < 32
        ) {
          seekBefore = data.previousCursor;
          continue;
        }
        if (seekBefore !== null && this.nodes.size > 0) break;
        if (older || !data.hasMore || ++rounds >= 5) break;
      } while (true);
    } finally {
      this.busy = false;
    }
  }
  render(older) {
    const before = this.follow.snapshot(),
      cards = projectActivity(this.rows.values());
    let added = 0;
    this.container.querySelector(".activity-empty")?.remove();
    const valid = new Set(cards.map((c) => c.key));
    for (const [key, node] of this.nodes)
      if (!valid.has(key)) {
        node.remove();
        this.nodes.delete(key);
      }
    for (const c of cards) {
      let node = this.nodes.get(c.key);
      if (!node) {
        node = this.card(c);
        this.nodes.set(c.key, node);
        added++;
      }
      this.updateCard(node, c);
    }
    // Only insert/reorder nodes when the order changed, preserving selection.
    let previous = null;
    for (const c of cards) {
      const n = this.nodes.get(c.key);
      if (
        n.previousElementSibling !== previous ||
        n.parentNode !== this.container
      )
        this.container.insertBefore(
          n,
          previous ? previous.nextSibling : this.container.firstChild,
        );
      previous = n;
    }
    if (!cards.length)
      this.container.append(
        el(
          "p",
          "这里按创作顺序展示思考、正文、操作和截图。可以载入更早过程，或开始新的创作。",
          "caption activity-empty",
        ),
      );
    this.follow.restore(before, { older, added });
  }
  card(c) {
    const n = el("article", undefined, "activity-card " + c.type);
    n.dataset.key = c.key;
    const head = el("div", undefined, "activity-card-head"),
      title = el("strong"),
      meta = el("span", undefined, "caption");
    head.append(title, meta);
    const thought = el("details", undefined, "activity-thought"),
      summary = el("summary"),
      thoughtText = el("div");
    thought.append(summary, thoughtText);
    const body = el("div", undefined, "activity-text"),
      tools = el("div", undefined, "activity-tools"),
      images = el("div", undefined, "activity-images");
    n.append(head, thought, body, tools, images);
    n.parts = {
      title,
      meta,
      thought,
      summary,
      thoughtText,
      body,
      tools,
      images,
    };
    n.streamText = new StreamText(body);
    n.thoughtStream = new StreamText(thoughtText,{label:"展开思考"});
    n.imageKeys = new Set();
    n.toolNodes = new Map();
    thought.addEventListener("toggle", async () => {
      if (!thought.open || !n.responseSeq || n.loadedThought === n.responseSeq)
        return;
      try {
        const d = await this.api("timeline/" + n.responseSeq);
        const t = d.body?.reasoning;
        if (t && t.length > n.thoughtStream.text.length)
          n.thoughtStream.setText(t);
        n.loadedThought = n.responseSeq;
      } catch (e) {
        this.onError(e);
      }
    });
    return n;
  }
  updateCard(n, c) {
    const p = n.parts;
    n.responseSeq = c.responseSeq;
    p.title.textContent =
      c.type === "user"
        ? "你"
        : c.type === "result"
          ? c.status
          : c.type === "notice"
            ? "运行提示"
            : c.type === "operation"
              ? "手动操作"
              : "MorphForge";
    const time = c.time
      ? new Date(c.time).toLocaleTimeString("zh-CN", { hour12: false })
      : "";
    p.meta.textContent = `${time}${c.type === "step" ? ` · ${c.status} · v${c.revision}` : ""}`;
    p.thought.hidden = !c.thought;
    p.summary.textContent = `思考${c.status === "进行中" ? "中" : ""} · ${c.thought.length.toLocaleString()} 字符（上游返回）`;
    if (
      n.thoughtStream.text !== c.thought &&
      n.loadedThought !== c.responseSeq
    )
      n.thoughtStream.setText(c.thought);
    if (n.lastText !== c.text) {
      n.streamText.setText(c.text);
      n.lastText = c.text;
    }
    p.body.hidden = !c.text;
    for (const t of c.tools) {
      let row = n.toolNodes.get(t.id);
      if (!row) {
        row = el("details", undefined, "activity-tool");
        const summary = el("summary"),
          detail = el("pre");
        row.append(summary, detail);
        row.parts = { summary, detail };
        p.tools.append(row);
        n.toolNodes.set(t.id, row);
        row.addEventListener("toggle", async () => {
          if (!row.open || row.loaded === row.seq) return;
          try {
            const d = await this.api("timeline/" + row.seq);
            row.parts.detail.textContent = JSON.stringify(d.body, null, 2);
            row.loaded = row.seq;
          } catch (e) {
            this.onError(e);
          }
        });
      }
      row.seq = t.seq;
      row.parts.summary.textContent = `${t.status === "完成" ? "✓" : t.status === "失败" ? "!" : "◌"} ${t.title} · ${t.status}${t.seconds ? ` · ${t.seconds.toFixed(1)}秒` : ""}`;
      row.classList.toggle("failed", t.status === "失败");
      if (t.error && !row.open) row.title = t.error;
      let errorNode = row.querySelector(".tool-error");
      if (t.error && !errorNode) {
        errorNode = el("p", undefined, "tool-error");
        row.append(errorNode);
      }
      if (errorNode) errorNode.textContent = t.error;
    }
    for (const image of c.images) {
      if (
        n.imageKeys.has(image.artifact) ||
        !/^\/artifacts\/[a-f0-9]{64}\.(png|jpg|jpeg|webp|gif)$/.test(
          image.url || "",
        )
      )
        continue;
      n.imageKeys.add(image.artifact);
      const button = el("button", undefined, "activity-image"),
        img = el("img"),
        caption = el(
          "span",
          `${image.view || "截图"} · ${["reinspection", "reference"].includes(image.role) ? "参考/历史" : `v${image.revision ?? c.revision}`}`,
          "caption",
        );
      img.src = image.url;
      img.alt = caption.textContent;
      img.loading = "lazy";
      button.append(img, caption);
      p.images.append(button);
      button.onclick = () => {
        const dialog = document.getElementById("activityImageDialog"),
          full = document.getElementById("activityImage");
        full.src = image.url;
        full.alt = img.alt;
        document.getElementById("activityImageTitle").textContent = img.alt;
        dialog.showModal();
      };
    }
  }
}
