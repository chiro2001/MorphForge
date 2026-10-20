const count = (value) => new Intl.NumberFormat("zh-CN").format(value || 0);

function tokens(stats = {}) {
  if (stats.requests && !stats.reportedRequests) return "未报告";
  return `${stats.unreportedRequests ? "≥" : ""}${count(stats.totalTokens)} Token`;
}

export function usageDisplay(usage = {}) {
  const total = usage.total || {};
  const rate = total.prefixHitRate;
  const prefixKnown = typeof rate === "number" && Number.isFinite(rate);
  return {
    tokens: `API累计 ${tokens(total)}`,
    run: usage.run ? `本轮 ${tokens(usage.run)}` : "",
    prefix: prefixKnown
      ? `Prefix ${(rate * 100).toFixed(1)}%`
      : "Prefix 未报告",
    tokenTitle: `当前数据目录累计上游用量，包含上下文整理请求。已报告输入 ${count(total.inputTokens)} / 输出 ${count(total.outputTokens)} Token；${count(total.reportedRequests)} 次请求有总用量，${count(total.unreportedRequests)} 次未报告。这里只统计上游usage，不混入本地估算；中断请求可能仍有未报告的费用。`,
    prefixTitle: prefixKnown
      ? `加权前缀命中率：缓存命中 ${count(total.cachedInputTokens)} ÷ 已报告缓存信息的输入 ${count(total.cacheReportedInputTokens)} Token。覆盖 ${count(total.cacheReportedRequests)} / ${count(total.requests)} 次请求；不平均各次百分比。`
      : "上游未报告有效缓存输入用量；不将缺失数据当作0%命中。",
  };
}

export function contextUsageDisplay(meter = {}) {
  const used=meter.usedTokens,window=meter.windowTokens;
  if(!Number.isFinite(used)||used<0||!Number.isFinite(window)||window<=0)
    return {text:"上下文 —",title:"尚无有效上下文窗口信息。",level:"unknown"};
  const percent=used/window*100,calibrated=meter.source==="upstream_input_plus_estimated_growth";
  return {
    text:`上下文 ≈${(used/1000).toFixed(1)}k / ${(window/1000).toFixed(1)}k · ${percent.toFixed(1)}%`,
    title:`当前会话上下文约 ${count(used)} / ${count(window)} Token，1k=1000 Token。${calibrated?`以上次上游输入 ${count(meter.upstreamInputTokens)} Token 校准，并估算此后新增历史。`:"当前为本地估算，含系统说明、工具及消息；尚无适用的上游输入校准。"}图片和tokenizer可能影响精度；生成中的输出在完整响应后更新。窗口采用配置与上游限制的较小值。不是累计计费用量，不把预留的最大输出当作已用上下文。`,
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
