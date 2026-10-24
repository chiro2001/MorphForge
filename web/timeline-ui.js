import {t as uiText, bindText} from "./i18n.js";
// The timeline is a read-only view. It never modifies model history or the draft.
import { VoxelRenderer } from "/renderer.js";
import { ScrollFollow } from "/scroll-follow.js";
const el = (tag, text, className) => {
  const node = document.createElement(tag);
  if (text !== undefined) bindText(node, () => (text));
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
    const checkbox = document.getElementById("timelineFollow");
    this.follow = new ScrollFollow(
      document.getElementById("timeline"),
      document.getElementById("timelineResume"),
      (following) => {
        checkbox.checked = following;
      },
    );
    checkbox.onchange = () => {
      if (checkbox.checked) this.follow.latest();
      else {
        this.follow.following = false;
        this.follow.update();
      }
    };
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
      this.reload()
        .then(() => this.follow.latest())
        .catch(onError);
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
      const scrollBefore = this.follow.snapshot();
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
      if (this.rows.size > 400 && this.follow.following) {
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
      bindText(document.getElementById("timelineStatus"), () => (uiText("已载入 {0} 条 · 思考仅展示上游实际返回内容", this.rows.size)));
      this.follow.restore(scrollBefore, { older, added: result.events.length });
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
        `#${row.seq} · ${row.revisionSource === "capture" ? uiText("截图") : row.revisionSource === "result" ? uiText("结果") : uiText("记录时")} v${row.revision}`,
        "caption",
      ),
    );
    if (row.runId) {
      const runButton = el("button", row.runId.slice(0, 8));
      bindText(runButton, () => (uiText("只查看此运行")), "title");
      runButton.onclick = () => {
        document.getElementById("eventRunFilter").value = row.runId;
        this.reload().catch(this.onError);
      };
      head.append(runButton);
    }
    const view = el("button", uiText("查看此版本"));
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
            ? uiText("思考过程 · 上游实际返回（流式内容见此前卡片）")
            : uiText("本响应未附带思考内容"),
        ),
      );
      if (body.reasoning) thought.append(el("pre", body.reasoning));
      card.append(thought);
      if (body.text) card.append(el("pre", body.text));
      if (body.usage)
        card.append(
          el(
            "div",
            uiText("耗时 {0}s · 用量 {1}", Number(body.seconds || 0).toFixed(2), JSON.stringify(body.usage)),
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
            ? uiText("上游思考片段（流式）")
            : uiText("上游文本片段（流式）"),
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
      const link = el("a", uiText("下载源码/快照"));
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
      img.alt = `${image.view || uiText("截图")} · ${image.role === "reinspection" ? uiText("历史/参考重读") : "v" + (image.revision ?? row.revision)}`;
      img.loading = "lazy";
      link.append(img);
      card.append(link);
    }
    const full = el("details");
    const summary = el("summary", uiText("展开完整记录"));
    const detail = el("pre", uiText("展开后按需读取原始事件中的可展示内容"));
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
        bindText(detail, () => (typeof args?.code === "string"
            ? uiText("工具：{0}\n\n{1}", body.name, args.code)
            : typeof args?.content === "string"
              ? uiText("源码：{0}\n\n{1}", args.path || "", args.content)
              : body.reasoning
                ? uiText("思考（上游返回）：\n{0}\n\n输出：\n{1}\n\n其他：\n{2}", body.reasoning, body.text || "", JSON.stringify({ ...body, reasoning: undefined, text: undefined }, null, 2))
                : JSON.stringify(body, null, 2)));
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
    bindText(document.getElementById("timelinePreviewTitle"), () => (uiText("历史版本 v{0} · 只读预览", revision)));
    dialog.showModal();
    if (!this.preview)
      this.preview = new VoxelRenderer(
        document.getElementById("timelinePreviewViewport"),
      );
    await this.preview.load(data.scene, data.materials, {
      mode: "art",
      surfaces: data.surfaces,
    });
    this.preview.resize();
    this.preview.view("hero");
  }
}
