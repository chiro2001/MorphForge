(function () {
  "use strict";

  const card = document.querySelector(".card");
  const title = document.getElementById("title");
  const message = document.getElementById("message");
  const log = document.getElementById("log");

  function render() {
    const state = window.__MF_DESKTOP_STATE__;
    if (!state) return;
    card.classList.toggle("error", state.phase === "error");
    title.textContent = state.title || "MorphForge";
    message.textContent = state.message || "";
    log.textContent = state.logPath ? `启动日志：${state.logPath}` : "";
  }

  window.__MF_DESKTOP_RENDER__ = render;
  window.__MF_DESKTOP_SET_STATE__ = function (state) {
    window.__MF_DESKTOP_STATE__ = state;
    render();
  };
  render();
})();

