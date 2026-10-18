export function buildTextRequest(values) {
  const text = String(values.text ?? "");
  const request = {
    text,
    fontId: String(values.fontId || "builtin:noto-sans-sc"),
    fontSize: clampInt(values.fontSize, 4, 512, 32),
    depth: clampInt(values.depth, 1, 256, 1),
    origin: [
      Number(values.origin?.[0] || 0),
      Number(values.origin?.[1] || 0),
      Number(values.origin?.[2] || 0),
    ],
    plane: ["xy", "xz", "yz"].includes(values.plane) ? values.plane : "xy",
    facing: values.facing === "negative" ? "negative" : "positive",
    align: ["left", "center", "right"].includes(values.align)
      ? values.align
      : "left",
    letterSpacing: clampInt(values.letterSpacing, -512, 512, 0),
    lineSpacing: clampInt(values.lineSpacing, -512, 512, 0),
    maxWidth:
      values.maxWidth === "" || values.maxWidth == null
        ? null
        : clampInt(values.maxWidth, 1, 1000000, 1),
    styleId: clampInt(values.styleId, 0, 4095, 1),
    tintId: clampInt(values.tintId, 0, 63, 0),
    motionPhase: clampInt(values.motionPhase, 0, 63, 0),
    group: String(values.group || "main"),
  };
  return request;
}

export function textRequestFingerprint(request) {
  return JSON.stringify(request);
}

export function previewBoxes(preview) {
  if (Array.isArray(preview?.boxes)) return preview.boxes;
  if (Array.isArray(preview?.scene?.boxes)) return preview.scene.boxes;
  if (Array.isArray(preview?.operations)) {
    return preview.operations
      .filter(
        (operation) =>
          operation?.op === "box" && operation.min && operation.size,
      )
      .map(({ op, ...box }) => box);
  }
  return [];
}

export function previewMetrics(preview) {
  const layout = preview?.layout || {};
  const bounds = preview?.bounds || layout.bounds || null;
  const dimensions =
    bounds?.min && bounds?.max
      ? bounds.max.map((value, axis) => value - bounds.min[axis])
      : null;
  const missing = preview?.missingGlyphs || layout.missingGlyphs || [];
  const warnings = preview?.warnings || [];
  return {
    dimensions,
    bounds,
    boxCount: Number(
      preview?.boxCount ?? layout.boxCount ?? previewBoxes(preview).length,
    ),
    voxelCount: Number(preview?.voxelCount ?? layout.voxelCount ?? 0),
    missing: Array.isArray(missing) ? missing : [],
    warnings: Array.isArray(warnings) ? warnings : [],
    overCapacity: preview?.overCapacity === true || preview?.fits === false,
  };
}

export function previewErrorDetails(error) {
  const payload = error?.payload || error || {};
  const structured = payload.error;
  const missing =
    payload.missingGlyphs ??
    payload.missing_glyphs ??
    structured?.missingGlyphs ??
    structured?.missing_glyphs ??
    [];
  const code =
    error?.code ||
    payload.code ||
    structured?.code ||
    (typeof structured === "string" ? structured.split(":", 1)[0] : "");
  const message =
    (typeof structured === "string" ? structured : structured?.message) ||
    payload.message ||
    error?.message ||
    "文字预览失败";
  return {
    code,
    message,
    missingGlyphs: Array.isArray(missing) ? missing.map(String) : [],
    missingGlyphError:
      code === "FONT_MISSING_GLYPHS" ||
      String(message).includes("FONT_MISSING_GLYPHS"),
  };
}

export function previewCanApply(
  preview,
  sentLocalFingerprint,
  currentLocalFingerprint,
  targetBoxes = 16384,
  serverFingerprint = null,
) {
  if (
    !preview ||
    !sentLocalFingerprint ||
    sentLocalFingerprint !== currentLocalFingerprint
  )
    return false;
  if (
    serverFingerprint !== null &&
    !/^[0-9a-f]{64}$/i.test(String(serverFingerprint))
  )
    return false;
  const metrics = previewMetrics(preview);
  const capacity =
    targetBoxes == null || targetBoxes === "" ? 16384 : Number(targetBoxes);
  if (!Number.isFinite(capacity) || capacity <= 0) return false;
  return (
    metrics.boxCount > 0 &&
    previewBoxes(preview).length > 0 &&
    metrics.boxCount <= capacity &&
    metrics.missing.length === 0 &&
    !metrics.overCapacity
  );
}

export function formatPreviewError(error) {
  const details = previewErrorDetails(error);
  if (details.missingGlyphError) {
    const missing = details.missingGlyphs.length
      ? "缺字：" + details.missingGlyphs.join(" ")
      : details.message.replace(/^FONT_MISSING_GLYPHS\s*:?\s*/, "缺字：");
    return `${missing}\n请换用支持这些字符的字体后再应用。`;
  }
  return details.message;
}

export function normalizeApplyResult(value, expectedRequestId = "") {
  if (!value || value.ok !== true) {
    const failure = new Error(
      typeof value?.error === "string" ? value.error : "文字应用失败",
    );
    failure.payload = value;
    throw failure;
  }
  if (expectedRequestId && value.requestId !== expectedRequestId) {
    throw new Error("文字提交状态校验失败，请刷新后重新预览。");
  }
  const revision = Number(value.revision ?? value.scene?.revision);
  if (!Number.isSafeInteger(revision) || revision < 0) {
    throw new Error("文字已提交，但没有收到有效的工程版本，请刷新确认。");
  }
  if (value.scene && Number(value.scene.revision) !== revision) {
    throw new Error("文字提交后的工程版本不一致，请刷新确认。");
  }
  return {
    revision,
    scene: value.scene || null,
    stats: value.stats || null,
    font: value.font || null,
    layout: value.layout || null,
    requestId: value.requestId || "",
    requestFingerprint: value.requestFingerprint || "",
    warning: typeof value.warning === "string" ? value.warning : "",
  };
}

function clampInt(value, min, max, fallback) {
  const number = Math.floor(Number(value));
  return Number.isFinite(number)
    ? Math.min(max, Math.max(min, number))
    : fallback;
}
