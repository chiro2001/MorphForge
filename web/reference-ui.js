export function selectedReferences(library, input, useLibrary, useInput) {
  return [
    ...new Map(
      [...(useLibrary ? library : []), ...(useInput ? input : [])].map(
        (item) => [item.id, item],
      ),
    ).values(),
  ];
}
export function sourceLink(value) {
  try {
    const url = new URL(value);
    return ["http:", "https:"].includes(url.protocol) ? url.href : null;
  } catch {
    return null;
  }
}
export class ReferencePicker {
  constructor({ api, error, onBusy, storageGet, storageSet }) {
    Object.assign(this, { api, error, onBusy, storageGet, storageSet });
    this.library = [];
    this.input = [];
    this.items = [];
    this.busy = false;
    this.scope = "";
    this.$ = (id) => document.getElementById(id);
    this.$("referenceOpen").onclick = () => this.open();
    this.$("referenceClose").onclick = () => this.$("referenceDialog").close();
    this.$("referenceUpload").onclick = () => this.$("referenceFiles").click();
    this.$("referenceFiles").onchange = async () => {
      const files = [...this.$("referenceFiles").files];
      this.$("referenceFiles").value = "";
      await this.upload(files);
    };
    for (const id of ["referenceSearch", "referenceCategory"])
      this.$(id).oninput = () => this.renderCatalog();
    for (const id of ["referenceUseLibrary", "referenceUseInput"])
      this.$(id).onchange = () => {
        if (this.selected().length > 6) {
          this.$(id).checked = false;
          this.error(new Error("最多选择6张参考图片"));
        }
        this.persist();
        this.render();
      };
    this.render();
  }
  setScope(scope, activeReferences = []) {
    if (this.scope === scope) return;
    this.scope = scope;
    const active = activeReferences.map((item) => ({
      ...item,
      previewUrl: "/artifacts/" + item.thumbnail,
    }));
    const saved = this.storageGet("morphforge.references." + scope, null) || {
      library: active.filter((item) => item.origin === "builtin"),
      input: active.filter((item) => item.origin !== "builtin"),
      useLibrary: active.some((item) => item.origin === "builtin"),
      useInput: active.some((item) => item.origin !== "builtin"),
    };
    const valid = (items) =>
      Array.isArray(items)
        ? items.filter((item) => typeof item?.id === "string").slice(0, 6)
        : [];
    this.library = valid(saved.library);
    this.input = valid(saved.input);
    this.$("referenceUseLibrary").checked = !!saved.useLibrary;
    this.$("referenceUseInput").checked = !!saved.useInput;
    this.render();
  }
  persist() {
    this.storageSet("morphforge.references." + this.scope, {
      library: this.library,
      input: this.input,
      useLibrary: this.$("referenceUseLibrary").checked,
      useInput: this.$("referenceUseInput").checked,
    });
  }
  selected() {
    return selectedReferences(
      this.library,
      this.input,
      this.$("referenceUseLibrary").checked,
      this.$("referenceUseInput").checked,
    );
  }
  render() {
    const selected = this.selected();
    const enabled = new Set(selected.map((item) => item.id));
    this.$("referenceUseLibrary").disabled = !this.library.length;
    this.$("referenceUseInput").disabled = !this.input.length;
    this.$("referenceSummary").textContent = this.busy
      ? `正在处理图片…（${selected.length}/6）`
      : selected.length
        ? `已选 ${selected.length}/6 张 · 下次输入生效`
        : "不使用参考图片";
    const container = this.$("referenceSelected");
    container.replaceChildren();
    for (const item of selectedReferences(
      this.library,
      this.input,
      true,
      true,
    )) {
      const chip = document.createElement("div");
      chip.className = "reference-chip";
      chip.classList.toggle("inactive", !enabled.has(item.id));
      const img = document.createElement("img");
      img.src = item.previewUrl || "/artifacts/" + item.thumbnail;
      img.alt = item.title;
      img.loading = "lazy";
      const title = document.createElement("span");
      title.textContent = item.title;
      title.title = `${item.author || ""} · ${item.license || ""}`;
      const remove = document.createElement("button");
      remove.textContent = "×";
      remove.setAttribute("aria-label", "取消参考 " + item.title);
      remove.onclick = () => {
        this.library = this.library.filter((i) => i.id !== item.id);
        this.input = this.input.filter((i) => i.id !== item.id);
        this.persist();
        this.render();
        this.renderCatalog();
      };
      chip.append(img, title, remove);
      container.append(chip);
    }
    this.$("referenceUpload").disabled = this.busy;
  }
  async open() {
    try {
      const value = await this.api("references");
      this.items = value.images || [];
      const select = this.$("referenceCategory");
      const category = select.value;
      select.replaceChildren(new Option("全部类别", ""));
      for (const c of new Set(this.items.map((item) => item.category)))
        select.add(new Option(c, c));
      if ([...select.options].some((item) => item.value === category))
        select.value = category;
      const sources = this.$("referenceSources");
      sources.replaceChildren();
      for (const source of value.sources || []) {
        const row = document.createElement("p");
        const link = sourceLink(source.url);
        if (link) {
          const a = document.createElement("a");
          a.href = link;
          a.target = "_blank";
          a.rel = "noreferrer";
          a.textContent = source.title;
          row.append(a);
        } else row.textContent = source.title;
        const note = document.createElement("span");
        note.textContent = " · " + source.note;
        row.append(note);
        sources.append(row);
      }
      this.renderCatalog();
      this.$("referenceDialog").showModal();
    } catch (error) {
      this.error(error);
    }
  }
  renderCatalog() {
    const container = this.$("referenceGrid");
    container.replaceChildren();
    const query = this.$("referenceSearch").value.toLowerCase();
    const category = this.$("referenceCategory").value;
    const items = this.items.filter(
      (item) =>
        (!category || item.category === category) &&
        `${item.title} ${item.category} ${item.author}`
          .toLowerCase()
          .includes(query),
    );
    this.$("referenceCatalogCount").textContent =
      `${items.length} 张 · 显示前96张，可搜索筛选`;
    for (const item of items.slice(0, 96)) {
      const card = document.createElement("article");
      card.className = "reference-card";
      card.dataset.referenceId = item.id;
      const img = document.createElement("img");
      img.src = item.previewUrl;
      img.alt = item.title;
      img.loading = "lazy";
      const label = document.createElement("label");
      const check = document.createElement("input");
      check.type = "checkbox";
      check.checked = this.library.some((i) => i.id === item.id);
      check.onchange = () => {
        const next = check.checked
          ? [...this.library.filter((i) => i.id !== item.id), item]
          : this.library.filter((i) => i.id !== item.id);
        if (
          selectedReferences(
            next,
            this.input,
            true,
            this.$("referenceUseInput").checked,
          ).length > 6
        ) {
          check.checked = false;
          return this.error(new Error("最多选择6张参考图片"));
        }
        this.library = next;
        this.$("referenceUseLibrary").checked = true;
        this.persist();
        this.render();
      };
      label.append(check, document.createTextNode(item.title));
      const credit = document.createElement("p");
      credit.className = "caption";
      credit.textContent = `${item.author} · ${item.license}`;
      const note = document.createElement("p");
      note.className = "caption";
      note.textContent = item.notes;
      card.append(img, label, credit, note);
      for (const [url, text] of [
        [item.sourceUrl, "来源"],
        [item.licenseUrl, "许可"],
      ]) {
        const href = sourceLink(url);
        if (href) {
          const a = document.createElement("a");
          a.href = href;
          a.target = "_blank";
          a.rel = "noreferrer";
          a.textContent = text;
          card.append(a, document.createTextNode(" "));
        }
      }
      if (item.origin === "local") {
        const remove = document.createElement("button");
        remove.textContent = "移出图库";
        remove.title = "移出图库；已关联运行的历史图片仍保留";
        remove.onclick = async () => {
          try {
            await this.api("references/remove", { id: item.id });
            this.items = this.items.filter((i) => i.id !== item.id);
            this.library = this.library.filter((i) => i.id !== item.id);
            this.input = this.input.filter((i) => i.id !== item.id);
            this.persist();
            this.render();
            this.renderCatalog();
          } catch (error) {
            this.error(error);
          }
        };
        card.append(remove);
      }
      container.append(card);
    }
  }
  async upload(files) {
    if (this.busy) return;
    this.busy = true;
    const originalScope = this.scope;
    this.onBusy();
    this.render();
    try {
      for (const file of files) {
        if (
          selectedReferences(
            this.library,
            this.input,
            this.$("referenceUseLibrary").checked,
            true,
          ).length >= 6
        )
          throw new Error("最多选择6张参考图片；已添加的图片保留");
        if (file.size > 8 * 1024 * 1024)
          throw new Error(`${file.name} 超过8MiB，请先缩小`);
        const form = new FormData();
        form.append("file", file);
        const response = await fetch("/api/references/upload", {
          method: "POST",
          body: form,
        });
        const result = await response.json();
        if (!response.ok) throw new Error(result.error || "图片上传失败");
        if (this.scope !== originalScope) break;
        this.input = [
          ...this.input.filter((item) => item.id !== result.id),
          result,
        ];
        this.$("referenceUseInput").checked = true;
        this.persist();
        this.render();
      }
    } catch (error) {
      this.error(error);
    } finally {
      this.busy = false;
      this.onBusy();
      this.render();
    }
  }
}
