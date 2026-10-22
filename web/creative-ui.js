import { keyframeAt } from "/motion.js";
import { VoxelRenderer } from "/renderer.js";
import { referenceImage, drawReference } from "/comparison.js";
function vector(text, n = 3) {
  const value = String(text)
    .split(/[,，\s]+/)
    .filter(Boolean)
    .map(Number);
  if (value.length !== n || !value.every(Number.isFinite))
    throw Error(`请输入${n}个有限数字，以逗号分隔`);
  return value;
}
function options(node, items, empty = "请选择") {
  const value = node.value;
  node.replaceChildren(
    new Option(empty, ""),
    ...items.map(([id, name]) => new Option(name, id)),
  );
  if ([...node.options].some((o) => o.value === value)) node.value = value;
}
export function setupCreativeUi({
  $,
  api,
  renderer,
  getState,
  getScene,
  getSelected,
  refresh,
  error,
  toast,
}) {
  document.body.insertAdjacentHTML(
    "beforeend",
    `
  <dialog id="comparisonDialog" class="creative-dialog" aria-labelledby="comparisonTitle">
    <div class="row"><h2 id="comparisonTitle">参考图对照工作台</h2><span class="grow"></span><button id="comparisonClose">关闭</button></div>
    <div class="row wrap"><label>参考图<select id="comparisonImage"></select></label><label>布局<select id="comparisonMode"><option value="split">并排</option><option value="overlay">叠加</option></select></label><label>书签<select id="comparisonBookmark"></select></label><button id="comparisonFromMain">采用主窗口视角</button><button id="comparisonFront">正交正面</button><button id="comparisonSide">正交侧面</button></div>
    <div class="comparison-stage"><canvas id="comparisonReference" aria-label="原始参考图"></canvas><div id="comparisonViewport"><canvas id="comparisonOverlay" aria-hidden="true"></canvas></div></div>
    <div class="creative-controls"><label>叠加透明度<input id="comparisonOpacity" type="range" min="0" max="1" step=".01" value=".5"></label><label>参考缩放<input id="comparisonZoom" type="number" min=".1" max="10" step=".1" value="1"></label><label>参考偏移 X,Y<input id="comparisonOffset" value="0, 0"></label><label>裁剪 x,y,w,h<input id="comparisonCrop" value="0, 0, 1, 1"></label><label>书签名称<input id="comparisonName" value="正面对照"></label></div>
    <div class="row wrap"><button id="comparisonSave">保存当前相机与对照</button><button id="comparisonNew">另存新书签</button><button id="comparisonRemove">删除书签</button><button id="comparisonCapture">生成对照截图</button><a id="comparisonDownload" hidden download="reference-comparison.png">保存截图</a></div>
    <p id="comparisonStatus" class="caption">参考图仅用于对照，不会绘制到作品。右键旋转、中键平移、滚轮缩放作品视角；可上传图片后重新打开此窗口。</p><img id="comparisonResult" hidden alt="参考图与作品对照截图">
  </dialog>
  <dialog id="componentDialog" class="creative-dialog" aria-labelledby="componentTitle">
    <div class="row"><h2 id="componentTitle">可复用组件</h2><span class="grow"></span><button id="componentClose">关闭</button></div>
    <p class="caption">项目内共享给所有对话。选择已有方块/三角面或输入分组名称保存；默认原点为选区地面中心，放置后可继续逐块编辑。</p>
    <div class="creative-controls"><label>已有组件<select id="componentSelect"></select></label><label>组件名称<input id="componentName" placeholder="例如：拱形窗"></label><label>源分组（逗号分隔；留空用选区）<input id="componentGroups"></label><label>说明<input id="componentDescription" placeholder="用途与适合的位置"></label></div>
    <div class="row"><button id="componentSave">保存为新组件</button><button id="componentReplace">更新选中组件</button><button id="componentRemove">删除组件</button></div>
    <div id="componentViewport" class="creative-preview"></div>
    <div class="creative-controls"><label>放置原点 X,Y,Z<input id="componentOffset" value="0, 0, 0"></label><label>整数缩放<input id="componentScale" type="number" min="1" max="64" value="1"></label><label>旋转<select id="componentTurns"><option value="0">0°</option><option value="1">90°</option><option value="2">180°</option><option value="3">270°</option></select></label><label>镜像<select id="componentMirror"><option value="">无</option><option>x</option><option>y</option><option>z</option></select></label><label>目标分组<input id="componentGroup" value="component"></label><label>方块材质ID（可留空）<input id="componentStyle" type="number" min="0"></label><label>重复数量<input id="componentCount" type="number" min="1" max="128" value="1"></label><label>重复步进 X,Y,Z<input id="componentStep" value="8, 0, 0"></label></div>
    <div class="row"><button id="componentDryRun">试算放置</button><button id="componentPlace">放置组件（可撤销）</button></div><pre id="componentStatus" class="status"></pre>
  </dialog>
  `,
  );
  const panel = document.createElement("section");
  panel.id = "motionPanel";
  panel.className = "motion-panel";
  panel.hidden = true;
  panel.innerHTML = `<div class="row"><strong>分组动画</strong><span class="grow"></span><button id="motionStatic">返回静态编辑</button><button id="motionClose">关闭面板</button></div><div class="row wrap"><select id="motionClip" aria-label="动画片段"></select><input id="motionName" aria-label="动画名称" value="新动画"><button id="motionNew">新片段</button><button id="motionSave">保存</button><button id="motionRemove">删除</button></div>
  <div class="row wrap"><button id="motionPlay">播放</button><button id="motionPause">暂停</button><button id="motionStart">回到开头</button><label>时长<input id="motionDuration" type="number" value="4" min=".05" max="3600" step=".1"></label><label><input id="motionLoop" type="checkbox" checked>循环</label><output id="motionTimeLabel">0.00 s</output></div>
  <input id="motionTime" aria-label="动画时间" type="range" min="0" max="4" step=".01" value="0">
  <details open><summary>分组与关键帧</summary><div class="creative-controls"><label>目标分组<select id="motionGroup"></select></label><label>枢轴 X,Y,Z<input id="motionPivot" value="0, 0, 0"></label><label>平移 X,Y,Z<input id="motionTranslation" value="0, 0, 0"></label><label>旋转 XYZ（度）<input id="motionRotation" value="0, 0, 0"></label><label>缩放 X,Y,Z<input id="motionScale" value="1, 1, 1"></label><label>插值<select id="motionInterpolation"><option value="linear">线性 / 球面旋转</option><option value="step">阶梯</option></select></label></div>
  <div class="row wrap"><button id="motionPivotCenter">枢轴设为分组地面中心</button><button id="motionAddKey">添加 / 替换当前关键帧</button><button id="motionDeleteTrack">删除分组轨道</button></div><div id="motionKeys" class="motion-keys"></div></details>
  <div class="row"><button id="motionCapture">截图当前姿态</button><a id="motionExport" hidden download="animation.glb">导出动画 GLB</a></div><details><summary>Avatar 动画</summary><div class="row wrap"><label>速度<select id="motionAvatarSpeed"><option value="0.25">0.25×</option><option value="0.5">0.5×</option><option value="1" selected>1×</option><option value="2">2×</option></select></label><button id="motionAvatarCheck">检查可发布性</button><button id="motionAvatarPublish">发布到 Avatar</button><button id="motionAvatarResume">继续</button><button id="motionAvatarPause">暂停</button><button id="motionAvatarStop">停止</button><button id="motionAvatarSeek">定位到预览时间</button><button id="motionAvatarDetach">解除动画绑定</button></div><p class="caption">需支持动画的 Avatar；最多3组、每组4帧、均匀缩放。时长可用0.125、0.25、0.5、1、2、4、8、16、32、64秒。预览与Avatar播放分别控制。</p></details><p id="motionStatus" class="caption">预览不会发送OSC；保存后可检查并发布Avatar动画。</p>`;
  $("viewport").append(panel);
  const motionResize = new ResizeObserver(() => {
    renderer.previewInset = panel.hidden
      ? 0
      : panel.getBoundingClientRect().height + 36;
    renderer.resize();
  });
  motionResize.observe(panel);
  let data = { revision: 0, bookmarks: {}, components: [], clips: {} },
    projectId = "",
    creativeRevision = -1,
    sceneRevision = -1,
    compareRenderer = null,
    componentRenderer = null,
    comparisonSeq = 0,
    clip = { name: "新动画", duration: 4, looping: true, tracks: [] },
    playing = false,
    playStart = 0,
    playOffset = 0;
  const bound = () => ({
    projectId: getState().project.id,
    baseCreativeRevision: data.revision,
  });
  const run = (id, fn) => {
    $(id).onclick = async () => {
      if ($(id).disabled) return;
      $(id).disabled = true;
      try {
        await fn();
      } catch (e) {
        error(e);
      } finally {
        $(id).disabled = false;
      }
    };
  };
  const updateData = (next) => {
    data = next;
    creativeRevision = next.revision;
    options(
      $("comparisonBookmark"),
      Object.entries(next.bookmarks).map(([id, b]) => [id, b.name]),
      "未保存的视角",
    );
    options(
      $("componentSelect"),
      next.components.map((c) => [
        c.id,
        `${c.name} · ${c.boxCount}盒/${c.triangleCount}面`,
      ]),
      "选择组件",
    );
    options(
      $("motionClip"),
      Object.entries(next.clips).map(([id, c]) => [id, c.name]),
      "未保存的片段",
    );
  };
  async function mutate(domain, args) {
    const result = await api("creative/" + domain, { ...bound(), ...args });
    updateData(result);
    await refresh();
    return result;
  }
  function comparison() {
    return {
      imageId: $("comparisonImage").value,
      mode: $("comparisonMode").value,
      opacity: Number($("comparisonOpacity").value),
      zoom: Number($("comparisonZoom").value),
      offset: vector($("comparisonOffset").value, 2),
      crop: vector($("comparisonCrop").value, 4),
    };
  }
  async function drawComparison() {
    const generation = ++comparisonSeq,
      c = comparison();
    $("comparisonDialog").classList.toggle(
      "overlay-mode",
      c.mode === "overlay",
    );
    if (!c.imageId) return;
    const image = await referenceImage(c.imageId);
    if (generation !== comparisonSeq) return;
    for (const [id, isOverlay] of [
      ["comparisonReference", false],
      ["comparisonOverlay", true],
    ]) {
      const canvas = $(id),
        rect = canvas.parentElement.getBoundingClientRect(),
        w = isOverlay
          ? rect.width
          : Math.max(100, $("comparisonReference").clientWidth),
        h = isOverlay
          ? rect.height
          : Math.max(100, $("comparisonReference").clientHeight);
      canvas.width = Math.round(w * devicePixelRatio);
      canvas.height = Math.round(h * devicePixelRatio);
      const ctx = canvas.getContext("2d");
      ctx.scale(devicePixelRatio, devicePixelRatio);
      ctx.clearRect(0, 0, w, h);
      if (!isOverlay) {
        ctx.fillStyle = "#101923";
        ctx.fillRect(0, 0, w, h);
      }
      ctx.globalAlpha = isOverlay ? c.opacity : 1;
      drawReference(ctx, image, c, 0, 0, w, h);
    }
  }
  async function loadComparison() {
    const b = data.bookmarks[$("comparisonBookmark").value];
    if (!b) return;
    $("comparisonName").value = b.name;
    const c = b.comparison;
    if (c) {
      $("comparisonImage").value = c.imageId;
      $("comparisonMode").value = c.mode;
      $("comparisonOpacity").value = c.opacity;
      $("comparisonZoom").value = c.zoom;
      $("comparisonOffset").value = c.offset.join(", ");
      $("comparisonCrop").value = c.crop.join(", ");
    }
    compareRenderer.viewShot(
      b.shot,
      $("comparisonViewport").clientWidth,
      $("comparisonViewport").clientHeight,
    );
    await drawComparison();
  }
  run("comparisonOpen", async () => {
    await sync(getState());
    const images = await api("references");
    options(
      $("comparisonImage"),
      images.images.map((i) => [i.id, i.title]),
    );
    if (!$("comparisonImage").value) {
      const preferred = images.images.find((i) =>
        i.author?.includes("MorphForge"),
      );
      $("comparisonImage").value = preferred?.id || images.images[0]?.id || "";
    }
    $("comparisonDialog").showModal();
    compareRenderer ??= new VoxelRenderer($("comparisonViewport"));
    compareRenderer.renderer.setAnimationLoop(compareRenderer.animationLoop);
    await compareRenderer.load(getScene().scene, getScene().materials);
    compareRenderer.viewShot(
      renderer.cameraBookmark(),
      $("comparisonViewport").clientWidth,
      $("comparisonViewport").clientHeight,
    );
    await drawComparison();
  });
  $("comparisonClose").onclick = () => $("comparisonDialog").close();
  $("comparisonDialog").addEventListener("close", () =>
    compareRenderer?.renderer.setAnimationLoop(null),
  );
  for (const id of [
    "comparisonImage",
    "comparisonMode",
    "comparisonOpacity",
    "comparisonZoom",
    "comparisonOffset",
    "comparisonCrop",
  ])
    $(id)[$(id).type === "range" ? "oninput" : "onchange"] = () =>
      drawComparison().catch(error);
  $("comparisonBookmark").onchange = () => loadComparison().catch(error);
  $("comparisonFromMain").onclick = () =>
    compareRenderer.viewShot(
      renderer.cameraBookmark(),
      $("comparisonViewport").clientWidth,
      $("comparisonViewport").clientHeight,
    );
  for (const [id, yaw] of [
    ["comparisonFront", 0],
    ["comparisonSide", 90],
  ])
    $(id).onclick = () =>
      compareRenderer.viewShot(
        { projection: "orthographic", yaw, pitch: 0 },
        $("comparisonViewport").clientWidth,
        $("comparisonViewport").clientHeight,
      );
  async function saveBookmark(fresh = false) {
    const id =
      (!fresh && $("comparisonBookmark").value) || "ref-" + crypto.randomUUID();
    await mutate("references", {
      action: "save",
      id,
      bookmark: {
        name: $("comparisonName").value,
        shot: compareRenderer.cameraBookmark(),
        comparison: comparison(),
      },
    });
    $("comparisonBookmark").value = id;
    toast("相机与参考对照已保存，Harness可按书签ID复用");
  }
  run("comparisonSave", () => saveBookmark());
  run("comparisonNew", () => saveBookmark(true));
  run("comparisonRemove", async () => {
    if (!$("comparisonBookmark").value) throw Error("请先选择书签");
    await mutate("references", {
      action: "remove",
      id: $("comparisonBookmark").value,
    });
    toast("书签已删除");
  });
  run("comparisonCapture", async () => {
    toast("正在生成参考图对照截图…");
    const r = await api("harness/tool", {
      name: "capture_views",
      arguments: {
        shots: [{ ...compareRenderer.cameraBookmark(), id: "comparison" }],
        comparison: comparison(),
        width: 1280,
        height: 720,
      },
    });
    $("comparisonResult").src = r.images[0].url;
    $("comparisonResult").hidden = false;
    $("comparisonDownload").href = r.images[0].url;
    $("comparisonDownload").hidden = false;
    $("comparisonStatus").textContent =
      `已保存对照截图 · v${r.revision} · 参考图不属于生成几何`;
    toast("对照截图已生成");
  });
  const resize = new ResizeObserver(() => {
    if ($("comparisonDialog").open) drawComparison().catch(error);
  });
  resize.observe($("comparisonViewport"));
  async function inspectComponent() {
    const id = $("componentSelect").value;
    if (!id) {
      if (componentRenderer)
        await componentRenderer.load(
          { ...getScene().scene, boxes: [], triangles: [] },
          getScene().materials,
        );
      $("componentStatus").textContent =
        "请选择组件，或从选区/分组保存新组件。";
      return;
    }
    const r = await api("creative/components", {
      action: "inspect",
      id,
      projectId,
    });
    $("componentName").value = r.component.name;
    $("componentDescription").value = r.component.description;
    $("componentStatus").textContent =
      `局部坐标 · 原点为保存时的地面中心 · 源版本 ${r.component.sourceRevision}`;
    componentRenderer ??= new VoxelRenderer($("componentViewport"));
    await componentRenderer.load(r.component.scene, getScene().materials);
    componentRenderer.view();
  }
  run("componentOpen", async () => {
    await sync(getState());
    $("componentDialog").showModal();
    componentRenderer?.renderer.setAnimationLoop(
      componentRenderer.animationLoop,
    );
    if (data.components.length && !$("componentSelect").value)
      $("componentSelect").value = data.components[0].id;
    await inspectComponent();
  });
  $("componentClose").onclick = () => $("componentDialog").close();
  $("componentDialog").addEventListener("close", () =>
    componentRenderer?.renderer.setAnimationLoop(null),
  );
  $("componentSelect").onchange = () => inspectComponent().catch(error);
  async function saveComponent(replace) {
    const id = replace
      ? $("componentSelect").value
      : "part-" + crypto.randomUUID();
    if (!id) throw Error("请先选择组件");
    const groups = $("componentGroups")
      .value.split(/[,，]/)
      .map((v) => v.trim())
      .filter(Boolean);
    await mutate("components", {
      action: "save",
      id,
      name: $("componentName").value || "新组件",
      description: $("componentDescription").value,
      ids: groups.length ? [] : [...getSelected()],
      groups,
      baseRevision: getState().revision,
    });
    $("componentSelect").value = id;
    await inspectComponent();
    toast("组件已保存，可在当前对话重复使用");
  }
  run("componentSave", () => saveComponent(false));
  run("componentReplace", () => saveComponent(true));
  run("componentRemove", async () => {
    if (!$("componentSelect").value) throw Error("请先选择组件");
    await mutate("components", {
      action: "remove",
      id: $("componentSelect").value,
    });
    await inspectComponent();
    toast("组件已删除，已放置几何保留");
  });
  async function placeComponent(dryRun) {
    const id = $("componentSelect").value;
    if (!id) throw Error("请先选择组件");
    const count = Number($("componentCount").value);
    if (!Number.isInteger(count) || count < 1 || count > 128)
      throw Error("重复数量1..128");
    const offset = vector($("componentOffset").value),
      step = vector($("componentStep").value),
      scale = Number($("componentScale").value),
      group = $("componentGroup").value;
    const instances = Array.from({ length: count }, (_, i) => ({
      offset: offset.map((v, a) => v + step[a] * i),
      scale,
      turns: Number($("componentTurns").value),
      group: count === 1 ? group : group + "-" + (i + 1),
      ...($("componentMirror").value
        ? { mirror: $("componentMirror").value }
        : {}),
      ...($("componentStyle").value !== ""
        ? { styleId: Number($("componentStyle").value) }
        : {}),
    }));
    const r = await api("creative/components", {
      ...bound(),
      action: "place",
      id,
      instances,
      baseRevision: getState().revision,
      requestId: crypto.randomUUID(),
      dryRun,
    });
    $("componentStatus").textContent = JSON.stringify(r, null, 2);
    if (!dryRun) {
      await refresh(true);
      toast("组件已放置，可撤销");
    } else toast("试算完成，草稿未修改");
  }
  run("componentDryRun", () => placeComponent(true));
  run("componentPlace", () => placeComponent(false));
  function time() {
    return Number($("motionTime").value);
  }
  const dirty = () => {
    $("motionExport").hidden = true;
  };
  $("motionName").oninput = dirty;
  function pose() {
    for (const track of clip.tracks)
      if (!(track.group in (getState().stats.groups || {})))
        throw Error("动画目标分组已删除：" + track.group);
    if (!clip.tracks.length) throw Error("先添加至少一条分组关键帧");
    renderer.setAnimationPose(clip, time());
    $("motionTimeLabel").textContent = time().toFixed(2) + " s";
  }
  function stop() {
    playing = false;
    $("motionPlay").textContent = "播放";
  }
  function keys() {
    const track = clip.tracks.find((t) => t.group === $("motionGroup").value);
    $("motionKeys").replaceChildren();
    for (const key of track?.keys || []) {
      const row = document.createElement("div");
      row.className = "row";
      const view = document.createElement("button");
      view.textContent = `${key.time.toFixed(2)}s · 移 ${key.translation.join(",")} · 转 ${key.rotation.join(",")}`;
      view.onclick = () => {
        stop();
        $("motionTime").value = key.time;
        $("motionTranslation").value = key.translation.join(", ");
        $("motionRotation").value = key.rotation.join(", ");
        $("motionScale").value = key.scale.join(", ");
        pose();
      };
      const remove = document.createElement("button");
      remove.textContent = "删除";
      remove.onclick = () => {
        stop();
        dirty();
        track.keys = track.keys.filter((k) => k !== key);
        if (!track.keys.length)
          clip.tracks = clip.tracks.filter((t) => t !== track);
        keys();
        renderer.clearAnimationPose();
      };
      row.append(view, remove);
      $("motionKeys").append(row);
    }
    if (track) {
      $("motionPivot").value = track.pivot.join(", ");
      $("motionInterpolation").value = track.interpolation;
    }
  }
  function readPoseFields() {
    const track = clip.tracks.find((t) => t.group === $("motionGroup").value);
    const key = track
      ? keyframeAt(track, time())
      : { translation: [0, 0, 0], rotation: [0, 0, 0], scale: [1, 1, 1] };
    for (const [id, field] of [
      ["motionTranslation", "translation"],
      ["motionRotation", "rotation"],
      ["motionScale", "scale"],
    ])
      $(id).value = key[field].map((v) => Number(v.toFixed(6))).join(", ");
    $("motionTimeLabel").textContent = time().toFixed(2) + " s";
  }
  function updateClipFields() {
    $("motionName").value = clip.name;
    $("motionDuration").value = clip.duration;
    $("motionLoop").checked = clip.looping;
    $("motionTime").max = clip.duration;
    $("motionTime").value = 0;
    keys();
    readPoseFields();
  }
  function pickClip() {
    stop();
    renderer.clearAnimationPose();
    const found = data.clips[$("motionClip").value];
    if (found) clip = structuredClone(found);
    updateClipFields();
    $("motionExport").hidden = !found;
    if (found)
      $("motionExport").href =
        "/api/creative/animation/" +
        encodeURIComponent($("motionClip").value) +
        "/export";
  }
  run("animationOpen", async () => {
    await sync(getState());
    $("motionPanel").hidden = false;
    renderer.resize();
  });
  $("motionClip").onchange = pickClip;
  $("motionGroup").onchange = () => {
    keys();
    readPoseFields();
  };
  $("motionClose").onclick = () => {
    stop();
    renderer.clearAnimationPose();
    $("motionPanel").hidden = true;
    renderer.previewInset = 0;
    renderer.resize();
  };
  $("motionStatic").onclick = () => {
    stop();
    renderer.clearAnimationPose();
    $("motionStatus").textContent =
      "已返回静态作品，可以继续编辑；动画轨道保留。";
  };
  $("motionNew").onclick = () => {
    stop();
    renderer.clearAnimationPose();
    clip = { name: "新动画", duration: 4, looping: true, tracks: [] };
    $("motionClip").value = "";
    $("motionExport").hidden = true;
    updateClipFields();
  };
  $("motionTime").oninput = () => {
    stop();
    try {
      if (clip.tracks.length) pose();
      readPoseFields();
    } catch (e) {
      error(e);
    }
  };
  $("motionDuration").onchange = () => {
    const duration = Number($("motionDuration").value);
    if (!Number.isFinite(duration) || duration < 0.05 || duration > 3600)
      return error(Error("时长0.05..3600秒"));
    dirty();
    clip.duration = duration;
    $("motionTime").max = duration;
  };
  $("motionLoop").onchange = () => {
    dirty();
    clip.looping = $("motionLoop").checked;
  };
  run("motionPivotCenter", async () => {
    const group = $("motionGroup").value,
      scene = getScene().scene,
      boxes = scene.boxes.filter((b) => b.group === group),
      triangles = (scene.triangles || []).filter((t) => t.group === group),
      points = boxes
        .flatMap((b) => [b.min, b.min.map((v, a) => v + b.size[a])])
        .concat(triangles.flatMap((t) => t.vertices));
    if (!points.length) throw Error("请选择有几何的分组");
    const min = [0, 1, 2].map((a) => Math.min(...points.map((p) => p[a]))),
      max = [0, 1, 2].map((a) => Math.max(...points.map((p) => p[a])));
    $("motionPivot").value = [
      (min[0] + max[0]) / 2,
      min[1],
      (min[2] + max[2]) / 2,
    ].join(", ");
  });
  run("motionAddKey", async () => {
    stop();
    const group = $("motionGroup").value;
    if (!group) throw Error("请选择目标分组");
    const pivot = vector($("motionPivot").value),
      interpolation = $("motionInterpolation").value;
    const key = {
      time: time(),
      translation: vector($("motionTranslation").value),
      rotation: vector($("motionRotation").value),
      scale: vector($("motionScale").value),
    };
    if (key.scale.some((v) => v < 0.001 || v > 1000))
      throw Error("缩放0.001..1000");
    let track = clip.tracks.find((t) => t.group === group);
    if (!track) {
      if (clip.tracks.length >= 64) throw Error("最多64条轨道");
      track = { group, pivot, interpolation, keys: [] };
      clip.tracks.push(track);
    }
    if (
      track.keys.length >= 256 &&
      !track.keys.some((k) => Math.abs(k.time - key.time) < 0.0001)
    )
      throw Error("每轨道最多256帧");
    track.pivot = pivot;
    track.interpolation = interpolation;
    track.keys = track.keys.filter((k) => Math.abs(k.time - key.time) > 0.0001);
    track.keys.push(key);
    track.keys.sort((a, b) => a.time - b.time);
    dirty();
    keys();
    pose();
    $("motionStatus").textContent =
      `已设置 ${group} @ ${key.time.toFixed(2)}s；点击保存以持久化。`;
  });
  run("motionDeleteTrack", async () => {
    stop();
    dirty();
    clip.tracks = clip.tracks.filter((t) => t.group !== $("motionGroup").value);
    keys();
    renderer.clearAnimationPose();
  });
  run("motionSave", async () => {
    clip.name = $("motionName").value;
    clip.duration = Number($("motionDuration").value);
    clip.looping = $("motionLoop").checked;
    const id = $("motionClip").value || "clip-" + crypto.randomUUID();
    await mutate("animation", { action: "save", id, clip });
    $("motionClip").value = id;
    $("motionExport").href =
      "/api/creative/animation/" + encodeURIComponent(id) + "/export";
    $("motionExport").hidden = false;
    toast("动画已保存，可导出GLB；静态作品保持原状");
  });
  run("motionRemove", async () => {
    if (!$("motionClip").value) throw Error("请选择已保存片段");
    await mutate("animation", { action: "remove", id: $("motionClip").value });
    $("motionNew").click();
    toast("动画片段已删除");
  });
  async function avatarAnimation(action, extra = {}) {
    const id = $("motionClip").value;
    if (action === "publish" && (!id || $("motionExport").hidden))
      throw Error("请先保存当前动画片段");
    const result = await api("creative/animation", {
      ...bound(), action, id,
      ...(action === "publish" ? {speed: Number($("motionAvatarSpeed").value), position: time(), playing: true} : {}),
      ...extra,
    });
    if (result.dryRun) {
      $("motionStatus").textContent = `可发布：${Object.keys(result.groups).length}组，定义${result.definitionFrames}帧＋控制${result.controlFrames}帧。尚未验证远端接收。`;
    } else {
      const motion = result.motion;
      $("motionStatus").textContent = action === "detach" ? "已解除Avatar动画绑定，静态作品已加入发送队列。" : `Avatar目标：${["停止", "播放", "暂停"][motion.state]} · ${Number(motion.position).toFixed(2)}秒 · 待发送${motion.pending}帧（无远端ACK）`;
      await refresh();
    }
  }
  run("motionAvatarCheck", () => avatarAnimation("publish", {dryRun: true}));
  run("motionAvatarPublish", () => avatarAnimation("publish"));
  run("motionAvatarResume", () => avatarAnimation("resume"));
  run("motionAvatarPause", () => avatarAnimation("pause"));
  run("motionAvatarStop", () => avatarAnimation("stop"));
  run("motionAvatarSeek", () => avatarAnimation("seek", {position: time()}));
  run("motionAvatarDetach", () => avatarAnimation("detach"));
  $("motionPlay").onclick = () => {
    try {
      pose();
      playing = true;
      playOffset = time();
      playStart = performance.now() / 1000;
      $("motionPlay").textContent = "播放中";
    } catch (e) {
      error(e);
    }
  };
  $("motionPause").onclick = () => {
    stop();
    readPoseFields();
  };
  $("motionStart").onclick = () => {
    stop();
    $("motionTime").value = 0;
    try {
      if (clip.tracks.length) pose();
      readPoseFields();
    } catch (e) {
      error(e);
    }
  };
  renderer.onAnimationFrame = (now) => {
    if (!playing) return;
    let t = playOffset + now - playStart;
    if (t > clip.duration) {
      if (clip.looping) t %= clip.duration;
      else {
        t = clip.duration;
        stop();
      }
    }
    $("motionTime").value = t;
    renderer.setAnimationPose(clip, t);
    $("motionTimeLabel").textContent = t.toFixed(2) + " s";
  };
  run("motionCapture", async () => {
    const id = $("motionClip").value;
    if (!id || $("motionExport").hidden)
      throw Error("请先保存当前动画片段，再截图");
    const r = await api("harness/tool", {
      name: "capture_views",
      arguments: {
        animation: { clipId: id, time: time() },
        views: ["hero"],
        width: 960,
        height: 720,
      },
    });
    $("motionStatus").replaceChildren();
    const link = document.createElement("a");
    link.href = r.images[0].url;
    link.target = "_blank";
    link.textContent = `查看 ${time().toFixed(2)}s 姿态截图`;
    $("motionStatus").append(link);
    toast("动画姿态截图已保存到过程记录");
  });
  async function sync(state) {
    if (!state || !getScene()) return;
    const changed = projectId !== state.project.id;
    if (changed) {
      projectId = state.project.id;
      creativeRevision = -1;
      stop();
      renderer.clearAnimationPose();
      clip = { name: "新动画", duration: 4, looping: true, tracks: [] };
      $("comparisonDialog").close();
      $("componentDialog").close();
      $("motionPanel").hidden = true;
      renderer.previewInset = 0;
      renderer.resize();
      $("motionClip").value = "";
      $("comparisonBookmark").value = "";
      $("componentSelect").value = "";
      updateClipFields();
    }
    if (changed || sceneRevision !== state.revision) {
      if (!changed && sceneRevision >= 0) {
        stop();
        renderer.clearAnimationPose();
      }
      sceneRevision = state.revision;
      options(
        $("motionGroup"),
        Object.keys(state.stats.groups || {}).map((g) => [g, g]),
      );
      if (!$("motionGroup").value && $("motionGroup").options.length > 1)
        $("motionGroup").selectedIndex = 1;
    }
    if (creativeRevision !== state.creativeRevision) {
      const expected = state.project.id,
        result = await api("creative");
      if (getState()?.project?.id !== expected) return;
      updateData(result);
    }
  }
  return {
    sync,
    get data() {
      return data;
    },
    get clip() {
      return clip;
    },
  };
}
