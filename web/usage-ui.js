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
