export function setupDesktopMenus({ getCellSize, getName, onError, onLayout }) {
  const $ = (id) => document.getElementById(id),
    bar = $("desktopMenu"),
    roots = [...bar.querySelectorAll(".app-menu")];
  const close = () =>
    bar.querySelectorAll("details[open]").forEach((n) => (n.open = false));
  const hints = {
    glb: "推荐用于Blender、Unity等：保留纯色、金属度、粗糙度、透明/裁切、发光和六面图片贴图。单位米，右手坐标，Y向上。",
    obj: "ZIP内含OBJ、MTL及贴图，解压后一起导入。保留纯色和六面图片；PBR/透明效果取决于导入软件。单位米，Y向上。",
    ply: "二进制PLY，保留顶点RGB与透明度；图片材质使用平均色，不保留贴图、PBR或动画。单位米，Y向上。",
    stl: "二进制STL，坐标按毫米写出，无颜色、材质或贴图；不自动保证可打印性。独立壳体和开放面会原样导出。",
  };
  const openExport = (format = "glb") => {
    $("modelExportFormat").value = format;
    $("modelExportCellSize").value = getCellSize();
    $("modelExportHint").textContent = hints[format];
    $("modelExportStatus").textContent = "";
    $("modelExportDialog").showModal();
  };
  for (const root of roots) {
    root.querySelector("summary").addEventListener("click", () => {
      for (const other of roots) if (other !== root) other.open = false;
    });
    root.querySelector("summary").addEventListener("pointerenter", () => {
      if (roots.some((n) => n.open)) {
        for (const other of roots) other.open = other === root;
      }
    });
  }
  document.addEventListener("pointerdown", (e) => {
    if (!bar.contains(e.target)) close();
  });
  bar.addEventListener("click", (e) => {
    const button = e.target.closest("button");
    if (!button) return;
    close();
    if (button.dataset.command) $(button.dataset.command)?.click();
    if (button.dataset.export) openExport(button.dataset.export);
  });
  bar.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      const root = e.target.closest(".app-menu");
      close();
      root?.querySelector("summary").focus();
      e.preventDefault();
      return;
    }
    const root = e.target.closest(".app-menu");
    if (!root) return;
    if (
      ["ArrowLeft", "ArrowRight"].includes(e.key) &&
      !e.target.closest(".menu-submenu")
    ) {
      e.preventDefault();
      const index =
        (roots.indexOf(root) +
          (e.key === "ArrowRight" ? 1 : -1) +
          roots.length) %
        roots.length;
      close();
      roots[index].open = true;
      roots[index].querySelector("summary").focus();
      return;
    }
    if (["ArrowDown", "ArrowUp", "Home", "End"].includes(e.key)) {
      e.preventDefault();
      root.open = true;
      const items = [
        ...root.querySelectorAll("button, .menu-submenu > summary"),
      ].filter((n) => n.getClientRects().length && !n.disabled);
      if (!items.length) return;
      const i = items.indexOf(e.target);
      const next =
        e.key === "Home"
          ? 0
          : e.key === "End"
            ? items.length - 1
            : (i + (e.key === "ArrowDown" ? 1 : -1) + items.length) %
              items.length;
      items[next].focus();
    }
  });
  document.addEventListener("keydown", (e) => {
    if (
      !(e.ctrlKey || e.metaKey) ||
      e.altKey ||
      document.querySelector("dialog[open]")
    )
      return;
    const key = e.key.toLowerCase();
    if (["n", "o", "s"].includes(key)) {
      e.preventDefault();
      close();
      if (key === "s" && e.shiftKey) openExport();
      else
        $(
          { n: "newProject", o: "importProject", s: "exportProject" }[key],
        ).click();
    }
  });
  $("toggleTools").onclick = () => {
    document.body.classList.toggle("hide-tools");
    onLayout();
  };
  $("toggleObserver").onclick = () => {
    document.body.classList.toggle("hide-observer");
    onLayout();
  };
  $("modelExportClose").onclick = () => $("modelExportDialog").close();
  $("modelExportFormat").onchange = () =>
    ($("modelExportHint").textContent = hints[$("modelExportFormat").value]);
  $("modelExportApply").onclick = async () => {
    const button = $("modelExportApply");
    button.disabled = true;
    button.setAttribute("aria-busy", "true");
    $("modelExportStatus").textContent = "正在导出…";
    try {
      const format = $("modelExportFormat").value,
        cellSize = Number($("modelExportCellSize").value);
      if (!Number.isFinite(cellSize) || cellSize < 0.000001 || cellSize > 1000)
        throw new Error("每格米数须为0.000001至1000");
      const r = await fetch(
        "/api/export/model?" +
          new URLSearchParams({ format, cellSize: String(cellSize) }),
      );
      if (!r.ok) {
        const v = await r.json();
        throw new Error(v.error || "导出失败");
      }
      const url = URL.createObjectURL(await r.blob()),
        a = document.createElement("a");
      a.href = url;
      a.download =
        getName().replace(/[<>:"/\\|?*\u0000-\u001f]/g, "_") +
        "." +
        (format === "obj" ? "obj.zip" : format);
      a.click();
      setTimeout(() => URL.revokeObjectURL(url), 30000);
      $("modelExportStatus").textContent = "导出完成，已交给下载管理器。";
    } catch (e) {
      $("modelExportStatus").textContent = e.message;
      onError(e);
    } finally {
      button.disabled = false;
      button.removeAttribute("aria-busy");
    }
  };
}
