// The timeline is a read-only view. It never modifies model history or the draft.
import { VoxelRenderer } from "/renderer.js";
const el = (tag, text, className) => {
  const node = document.createElement(tag);
  if (text !== undefined) node.textContent = text;
  if (className) node.className = className;
  return node;
};
export class TimelineView {
  constructor(api, onError) {
    this.api = api;
    this.onError = onError;
    this.rows = new Map();
    this.cursor = null;
    this.before = null;
    this.busy = false;
    this.generation = 0;
    this.preview = null;
    this.filterTimer = 0;
    for (const id of [
      "eventFilter",
      "eventRunFilter",
      "eventRevisionFilter",
      "timelineSearch",
    ])
      document.getElementById(id).addEventListener("input", () => {
        clearTimeout(this.filterTimer);
        this.filterTimer = setTimeout(() => this.reload().catch(onError), 250);
      });
    document.getElementById("timelineOlder").onclick = () =>
      this.load(true).catch(onError);
    document.getElementById("timelineLatest").onclick = () =>
      this.reload().catch(onError);
    document.getElementById("timelinePreviewClose").onclick = () =>
      document.getElementById("timelinePreview").close();
  }
  query() {
    const q = new URLSearchParams({ limit: "100" });
    for (const [id, key] of [
      ["eventFilter", "kind"],
      ["eventRunFilter", "run"],
      ["timelineSearch", "search"],
    ]) {
      const v = document.getElementById(id).value.trim();
      if (v) q.set(key, v);
    }
    return q;
  }
  async reload() {
    this.generation++;
    this.rows.clear();
    this.cursor = null;
    this.before = null;
    document.getElementById("events").replaceChildren();
    while (this.busy) await new Promise((resolve) => setTimeout(resolve, 20));
    await this.load();
  }
  async load(older = false) {
    if (this.busy) return;
    this.busy = true;
    const generation = this.generation;
    const q = this.query();
    if (older && this.before !== null) q.set("before", this.before);
    else if (this.cursor !== null) q.set("cursor", this.cursor);
    try {
      const result = await this.api("timeline?" + q);
      if (generation !== this.generation) return;
      if (result.reset) {
        this.rows.clear();
        document.getElementById("events").replaceChildren();
      }
      if (older || this.before === null) this.before = result.previousCursor;
      if (!older) this.cursor = result.nextCursor;
      const revision = document.getElementById("eventRevisionFilter").value;
      for (const row of result.events) {
        if (revision !== "" && row.revision !== Number(revision)) continue;
        this.rows.set(row.seq, row);
      }
      const container = document.getElementById("events");
      if (this.rows.size > 400) {
        const sorted = [...this.rows.keys()].sort((a, b) => a - b);
        for (const seq of older ? sorted.slice(400) : sorted.slice(0, -400)) {
          this.rows.delete(seq);
          container.querySelector(`[data-seq="${seq}"]`)?.remove();
        }
        this.before =
          [...this.rows.values()].sort((a, b) => a.seq - b.seq)[0]?.cursor ??
          this.before;
      }
      for (const row of [...this.rows.values()].sort((a, b) => a.seq - b.seq)) {
        if (!container.querySelector(`[data-seq="${row.seq}"]`))
          container.append(this.card(row));
      }
      for (const node of [...container.children].sort(
        (a, b) => Number(a.dataset.seq) - Number(b.dataset.seq),
      ))
        container.append(node);
      document.getElementById("timelineOlder").disabled = this.before === 0;
      document.getElementById("timelineStatus").textContent =
        `已载入 ${this.rows.size} 条 · 思考仅展示上游实际返回内容`;
      if (!older && document.getElementById("timelineFollow").checked)
        container.lastElementChild?.scrollIntoView({ block: "start" });
    } finally {
      this.busy = false;
    }
  }
  card(row) {
    const card = el("article", undefined, "timeline-card");
    card.dataset.seq = row.seq;
    const head = el("div", undefined, "row");
    head.append(
      el("time", new Date(row.time).toLocaleString("zh-CN", { hour12: false })),
      el("b", row.title),
      el(
        "span",
        `#${row.seq} · ${row.revisionSource === "capture" ? "截图" : row.revisionSource === "result" ? "结果" : "记录时"} v${row.revision}`,
        "caption",
      ),
    );
    if (row.runId) {
      const runButton = el("button", row.runId.slice(0, 8));
      runButton.title = "只查看此运行";
      runButton.onclick = () => {
        document.getElementById("eventRunFilter").value = row.runId;
        this.reload().catch(this.onError);
      };
      head.append(runButton);
    }
    const view = el("button", "查看此版本");
    view.onclick = () => this.viewRevision(row.revision).catch(this.onError);
    head.append(view);
    card.append(head);
    const body = row.body || {};
    if (row.kind === "provider.response") {
      const thought = el("details");
      thought.append(
        el(
          "summary",
          body.reasoningProvided
            ? "思考过程 · 上游实际返回（流式内容见此前卡片）"
            : "本响应未附带思考内容",
        ),
      );
      if (body.reasoning) thought.append(el("pre", body.reasoning));
      card.append(thought);
      if (body.text) card.append(el("pre", body.text));
      if (body.usage)
        card.append(
          el(
            "div",
            `耗时 ${Number(body.seconds || 0).toFixed(2)}s · 用量 ${JSON.stringify(body.usage)}`,
            "caption",
          ),
        );
    } else if (
      row.kind === "provider.reasoning_delta" ||
      row.kind === "provider.text_delta"
    ) {
      const stream = el("details");
      stream.open = true;
      stream.append(
        el(
          "summary",
          row.kind === "provider.reasoning_delta"
            ? "上游思考片段（流式）"
            : "上游文本片段（流式）",
        ),
        el("pre", body.text || ""),
      );
      card.append(stream);
    } else {
      const details = el("details");
      details.append(
        el("summary", body.name || row.kind),
        el("pre", JSON.stringify(body, null, 2)),
      );
      card.append(details);
    }
    for (const name of [body.artifact, body.sourceSnapshot].filter(
      (name) =>
        typeof name === "string" &&
        /^[a-f0-9]{64}\.(js|json|txt|md)$/.test(name),
    )) {
      const link = el("a", "下载源码/快照");
      link.href = "/artifacts/" + name;
      link.download = name;
      card.append(link);
    }
    for (const image of row.images || []) {
      const link = el("a");
      link.href = image.url;
      link.target = "_blank";
      link.rel = "noopener";
      const img = el("img");
      img.src = image.url;
      img.alt = `${image.view || "截图"} · ${image.role === "reinspection" ? "历史/参考重读" : "v" + (image.revision ?? row.revision)}`;
      img.loading = "lazy";
      link.append(img);
      card.append(link);
    }
    const full = el("details");
    const summary = el("summary", "展开完整记录");
    const detail = el("pre", "展开后按需读取原始事件中的可展示内容");
    full.append(summary, detail);
    let loaded = false;
    full.ontoggle = async () => {
      if (!full.open || loaded) return;
      try {
        const value = await this.api("timeline/" + row.seq);
        const body = value.body || {};
        let args = null;
        try {
          args =
            typeof body.arguments === "string"
              ? JSON.parse(body.arguments)
              : null;
        } catch {}
        detail.textContent =
          typeof args?.code === "string"
            ? `工具：${body.name}\n\n${args.code}`
            : typeof args?.content === "string"
              ? `源码：${args.path || ""}\n\n${args.content}`
              : body.reasoning
                ? `思考（上游返回）：\n${body.reasoning}\n\n输出：\n${body.text || ""}\n\n其他：\n${JSON.stringify({ ...body, reasoning: undefined, text: undefined }, null, 2)}`
                : JSON.stringify(body, null, 2);
        loaded = true;
      } catch (error) {
        this.onError(error);
      }
    };
    card.append(full);
    return card;
  }
  async viewRevision(revision) {
    const data = await this.api("scene?revision=" + revision);
    const dialog = document.getElementById("timelinePreview");
    document.getElementById("timelinePreviewTitle").textContent =
      `历史版本 v${revision} · 只读预览`;
    dialog.showModal();
    if (!this.preview)
      this.preview = new VoxelRenderer(
        document.getElementById("timelinePreviewViewport"),
      );
    await this.preview.load(data.scene, data.materials, { mode: "art", surfaces:data.surfaces });
    this.preview.resize();
    this.preview.view("hero");
  }
}
