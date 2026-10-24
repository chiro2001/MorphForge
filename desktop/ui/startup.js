(function () {
  "use strict";

  const card = document.querySelector(".card");
  const title = document.getElementById("title");
  const message = document.getElementById("message");
  const log = document.getElementById("log");
  const code = window.__MF_DESKTOP_LANGUAGE__ || navigator.language;
  const lang = code.startsWith('zh') ? 'zh-CN' : code.startsWith('ja') ? 'ja' : 'en';
  document.documentElement.lang = lang;
  const labels = {
    'zh-CN': ['正在启动本地核心', '正在启动 morphforge.exe，并等待本机 HTTP 服务就绪。', '启动日志：'],
    ja: ['ローカルコアを起動中', 'morphforge.exeを起動し、ローカルHTTPサービスの準備を待っています。', '起動ログ：'],
    en: ['Starting local core', 'Starting morphforge.exe and waiting for the local HTTP service.', 'Startup log: '],
  }[lang];
  title.textContent = labels[0]; message.textContent = labels[1];

  function render() {
    const state = window.__MF_DESKTOP_STATE__;
    if (!state) return;
    card.classList.toggle("error", state.phase === "error");
    title.textContent = state.title || "MorphForge";
    message.textContent = state.message || "";
    log.textContent = state.logPath ? labels[2] + state.logPath : "";
  }

  window.__MF_DESKTOP_RENDER__ = render;
  window.__MF_DESKTOP_SET_STATE__ = function (state) {
    window.__MF_DESKTOP_STATE__ = state;
    render();
  };
  render();
})();

