import {t as uiText, bindText} from "./i18n.js";
const count = (value) => new Intl.NumberFormat("zh-CN").format(value || 0);

function tokens(stats = {}) {
  if (stats.requests && !stats.reportedRequests) return uiText("未报告");
  return `${stats.unreportedRequests ? "≥" : ""}${count(stats.totalTokens)} Token`;
}

export function usageDisplay(usage = {}) {
  const total = usage.total || {};
  const rate = total.prefixHitRate;
  const prefixKnown = typeof rate === "number" && Number.isFinite(rate);
  return {
    tokens: uiText("API累计 {0}", tokens(total)),
    run: usage.run ? uiText("本轮 {0}", tokens(usage.run)) : "",
    prefix: prefixKnown
      ? `Prefix ${(rate * 100).toFixed(1)}%`
      : uiText("Prefix 未报告"),
    tokenTitle: uiText("当前数据目录累计上游用量，包含上下文整理请求。已报告输入 {0} / 输出 {1} Token；{2} 次请求有总用量，{3} 次未报告。这里只统计上游usage，不混入本地估算；中断请求可能仍有未报告的费用。", count(total.inputTokens), count(total.outputTokens), count(total.reportedRequests), count(total.unreportedRequests)),
    prefixTitle: prefixKnown
      ? uiText("加权前缀命中率：缓存命中 {0} ÷ 已报告缓存信息的输入 {1} Token。覆盖 {2} / {3} 次请求；不平均各次百分比。", count(total.cachedInputTokens), count(total.cacheReportedInputTokens), count(total.cacheReportedRequests), count(total.requests))
      : uiText("上游未报告有效缓存输入用量；不将缺失数据当作0%命中。"),
  };
}

export function contextUsageDisplay(meter = {}) {
  const used=meter.usedTokens,window=meter.windowTokens;
  if(!Number.isFinite(used)||used<0||!Number.isFinite(window)||window<=0)
    return {text:uiText("上下文 —"),title:uiText("尚无有效上下文窗口信息。"),level:"unknown"};
  const percent=used/window*100,calibrated=meter.source==="upstream_input_plus_estimated_growth";
  return {
    text:uiText("上下文 ≈{0}k / {1}k · {2}%", (used/1000).toFixed(1), (window/1000).toFixed(1), percent.toFixed(1)),
    title:uiText("当前会话上下文约 {0} / {1} Token，1k=1000 Token。{2}图片和tokenizer可能影响精度；生成中的输出在完整响应后更新。窗口采用配置与上游限制的较小值。不是累计计费用量，不把预留的最大输出当作已用上下文。", count(used), count(window), calibrated?uiText("以上次上游输入 {0} Token 校准，并估算此后新增历史。", count(meter.upstreamInputTokens)):uiText("当前为本地估算，含系统说明、工具及消息；尚无适用的上游输入校准。")),
    level:percent>=100?"over":percent>=90?"near":"normal",
  };
}

export function supportsFileUpload(base, api) {
  try {
    const url = new URL(base);
    return (
      api === "responses" &&
      url.protocol === "https:" &&
      url.hostname === "api.deepseek.com" &&
      (!url.port || url.port === "443")
    );
  } catch {
    return false;
  }
}
