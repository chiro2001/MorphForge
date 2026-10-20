const LABELS = {
  get_context: "读取工程状态",
  define_palette: "创建配色",
  define_materials: "创建材质",
  run_recipe: "绘制模型",
  recipe_sources: "编辑与运行源码",
  capture_views: "观察模型截图",
  read_artifact: "查看图片或文件",
  finish_artwork: "完成并保存作品",
  commit: "提交作品版本",
  artworks: "访问作品库",
  mesh_tools: "编辑三角面",
  components: "复用组件",
  reference_workbench: "参考图对照",
  animation: "编辑分组动画",
  transform_scene: "整体移动作品",
  voxel_text: "生成体素文字",
  image_pixels: "读取参考图像素",
};
export function toolLabel(name) {
  return LABELS[name] || name || "执行操作";
}
function clipped(value) {
  return typeof value === "string" ? value : "";
}
export function projectActivity(rows) {
  const cards = new Map(),
    current = new Map(),
    calls = new Map();
  const make = (key, row, type = "step") => {
    if (!cards.has(key))
      cards.set(key, {
        key,
        type,
        seq: row.seq,
        runId: row.runId,
        revision: row.revision,
        time: row.time,
        text: "",
        thought: "",
        tools: [],
        images: [],
        status: "进行中",
        responseSeq: null,
      });
    return cards.get(key);
  };
  for (const r of [...rows].sort((a, b) => a.seq - b.seq)) {
    const b = r.body || {},
      run = r.runId || "manual",
      req = r.requestId || b.requestId;
    const kind = r.kind || "";
    if (
      (kind === "harness.started" ||
        kind === "harness.message.accepted" ||
        kind === "harness.message.applied") &&
      b.prompt
    ) {
      const c = make(`user:${b.requestId || r.requestId || run}`, r, "user");
      c.text = clipped(b.prompt);
      if (kind === "harness.started") c.mode = "start";
      else if (c.mode !== "start") c.mode = b.mode;
      c.status =
        { queue: "排队", interrupt: "打断", steer: "插话", start: "已发送" }[
          c.mode
        ] || "已发送";
      for (const image of [
        ...(r.images || []),
        ...(b.references || []).map((i) => ({
          ...i,
          url: "/artifacts/" + i.artifact,
          role: "reference",
          view: i.title || "参考图片",
        })),
      ]) {
        if (!c.images.some((i) => i.artifact === image.artifact))
          c.images.push(image);
      }
      continue;
    }
    if (
      kind === "provider.request" ||
      kind === "provider.reasoning_delta" ||
      kind === "provider.text_delta" ||
      kind === "provider.response"
    ) {
      const key = req
        ? `step:${run}:${req}`
        : current.get(run) || `step:${run}:${r.seq}`;
      current.set(run, key);
      const c = make(key, r);
      c.revision = r.revision;
      if (kind === "provider.reasoning_delta") c.thought += clipped(b.text);
      if (kind === "provider.text_delta") c.text += clipped(b.text);
      if (kind === "provider.response") {
        if (clipped(b.reasoning).length > c.thought.length)
          c.thought = b.reasoning;
        if (clipped(b.text).length >= c.text.length) c.text = b.text;
        c.responseSeq = r.seq;
        c.status = "已响应";
        c.seconds = b.seconds;
        c.usage = b.usage;
      }
      continue;
    }
    if (
      kind === "tool.request" ||
      kind === "tool.result" ||
      kind === "capture.completed"
    ) {
      let key =
        calls.get(r.callId) || current.get(run) || `operation:${run}:${r.seq}`;
      if (run === "manual") {
        if (kind === "tool.request")
          key = `operation:manual:${r.callId || r.seq}`;
        else key = calls.get(r.callId) || current.get(run) || key;
        current.set(run, key);
      }
      const c = make(key, r, run === "manual" ? "operation" : "step");
      if (run === "manual") c.status = "已记录";
      c.revision = r.revision;
      if (r.callId) calls.set(r.callId, key);
      if (kind !== "capture.completed") {
        const id = r.callId || `${b.name}:${r.seq}`;
        let t = c.tools.find((x) => x.id === id);
        if (!t) {
          t = {
            id,
            name: b.name,
            seq: r.seq,
            title: b.summary || toolLabel(b.name),
            status: "执行中",
          };
          c.tools.push(t);
        }
        if (kind === "tool.result") {
          const result = b.result || {};
          t.seq = r.seq;
          t.status = result.ok === false ? "失败" : "完成";
          t.error = clipped(result.error?.message) || clipped(result.error);
          t.seconds = Number(b.elapsedMs || 0) / 1000;
          t.revision = result.revision ?? r.revision;
        }
      }
      for (const image of r.images || [])
        if (!c.images.some((i) => i.artifact === image.artifact))
          c.images.push({ ...image, seq: r.seq });
      continue;
    }
    if (kind === "harness.finished") {
      for (const c of cards.values())
        if (c.runId === r.runId && c.type === "step")
          c.status =
            b.status === "completed"
              ? "已完成"
              : { failed: "已中断", stopped: "已停止", cancelled: "已取消" }[
                  b.status
                ] ||
                b.status ||
                "已结束";
      if (b.message || b.error) {
        const c = make(`finish:${run}:${r.seq}`, r, "result");
        c.text = clipped(b.message) || clipped(b.error);
        c.status = b.status === "completed" ? "创作完成" : "运行已停止";
      }
      continue;
    }
    if (
      kind === "cache.guard.blocked" ||
      kind === "compact.completed" ||
      kind === "provider.cancelled" ||
      kind === "harness.paused"
    ) {
      const c = make(`notice:${r.seq}`, r, "notice");
      c.text =
        clipped(b.message) ||
        clipped(b.reason) ||
        {
          "cache.guard.blocked": "缓存保护已暂停调用，请查看配置中的告警。",
          "compact.completed": "上下文已整理，创作记录保留。",
          "provider.cancelled": "当前模型请求已取消。",
          "harness.paused": "运行已暂停。",
        }[kind];
      c.status = "运行提示";
    }
  }
  return [...cards.values()].sort((a, b) => a.seq - b.seq);
}
