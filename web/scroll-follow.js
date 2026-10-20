// Follow only while the reader is at the end. Never scroll an ancestor/page.
export class ScrollFollow {
  constructor(container, button, onChange = () => {}) {
    this.container = container;
    this.button = button;
    this.following = true;
    this.pending = 0;
    this.onChange = onChange;
    container.addEventListener(
      "scroll",
      () => {
        this.following = this.atEnd();
        if (this.following) this.pending = 0;
        this.update();
      },
      { passive: true },
    );
    const detach = () => {
      this.following = false;
      this.update();
    };
    container.addEventListener(
      "wheel",
      (e) => {
        if (e.deltaY < 0) detach();
      },
      { passive: true },
    );
    container.addEventListener("keydown", (e) => {
      if (["PageUp", "Home", "ArrowUp"].includes(e.key)) detach();
    });
    button?.addEventListener("click", () => this.latest());
  }
  atEnd() {
    const c = this.container;
    return c.scrollHeight - c.scrollTop - c.clientHeight < 4;
  }
  snapshot() {
    return {
      top: this.container.scrollTop,
      height: this.container.scrollHeight,
      following: this.following,
    };
  }
  restore(before, { older = false, added = 0 } = {}) {
    const c = this.container;
    if (older) {
      c.scrollTop = before.top + c.scrollHeight - before.height;
      this.following = false;
    } else if (before.following) c.scrollTop = c.scrollHeight;
    else {
      c.scrollTop = before.top;
      this.pending += added;
    }
    this.update();
  }
  latest() {
    this.following = true;
    this.pending = 0;
    this.container.scrollTop = this.container.scrollHeight;
    this.update();
  }
  update() {
    if (this.button) {
      this.button.hidden = this.following;
      this.button.textContent = this.pending
        ? `返回最新 · ${this.pending} 条更新`
        : "返回最新";
    }
    this.onChange(this.following);
  }
}
