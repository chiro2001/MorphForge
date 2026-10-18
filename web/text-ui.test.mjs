import assert from "node:assert/strict";
import test from "node:test";
import {
  buildTextRequest,
  formatPreviewError,
  normalizeApplyResult,
  previewBoxes,
  previewCanApply,
  previewErrorDetails,
  previewMetrics,
  textRequestFingerprint,
} from "./text-ui.js";

const values = {
  text: "中文\nABC",
  fontId: "builtin:noto-sans-sc",
  fontSize: 12,
  depth: 2,
  origin: [4, 5, 6],
  plane: "xz",
  facing: "negative",
  align: "center",
  letterSpacing: 1,
  lineSpacing: 2,
  maxWidth: "",
  styleId: 7,
  tintId: 3,
  motionPhase: 4,
  group: "标题",
};

test("text request uses the confirmed canonical fields without axis flipping", () => {
  const request = buildTextRequest(values);
  assert.deepEqual(request.origin, [4, 5, 6]);
  assert.equal(request.plane, "xz");
  assert.equal(request.facing, "negative");
  assert.equal(request.maxWidth, null);
  assert.equal(request.motionPhase, 4);
});

test("preview boxes come from canonical box operations", () => {
  const preview = {
    operations: [
      { op: "box", min: [1, 2, 3], size: [4, 5, 6], styleId: 7 },
      { op: "erase", min: [0, 0, 0], size: [1, 1, 1] },
    ],
  };
  assert.deepEqual(previewBoxes(preview), [
    { min: [1, 2, 3], size: [4, 5, 6], styleId: 7 },
  ]);
});

test("stale or invalid previews cannot be applied", () => {
  const request = buildTextRequest(values);
  const fingerprint = textRequestFingerprint(request);
  const preview = {
    operations: [{ op: "box", min: [0, 0, 0], size: [1, 1, 1] }],
    layout: {
      boxCount: 1,
      voxelCount: 1,
      bounds: { min: [0, 0, 0], max: [1, 1, 2] },
      missingGlyphs: [],
    },
  };
  assert.equal(previewMetrics(preview).dimensions.join(","), "1,1,2");
  assert.equal(previewCanApply(preview, fingerprint, fingerprint, 100), true);
  assert.equal(
    previewCanApply(preview, fingerprint, fingerprint + "x", 100),
    false,
  );
  assert.equal(
    previewCanApply(
      { ...preview, layout: { ...preview.layout, missingGlyphs: ["缺"] } },
      fingerprint,
      fingerprint,
      100,
    ),
    false,
  );
  assert.equal(previewCanApply(preview, fingerprint, fingerprint, 0), false);
  assert.equal(
    previewCanApply(preview, fingerprint, fingerprint, 100, "not-a-server-sha"),
    false,
  );
});

test("missing glyph errors remain actionable and block apply", () => {
  const error = {
    status: 400,
    message: "FONT_MISSING_GLYPHS: U+1F600",
    payload: {
      ok: false,
      error: "FONT_MISSING_GLYPHS: U+1F600",
      missingGlyphs: ["U+1F600"],
    },
  };
  const details = previewErrorDetails(error);
  assert.equal(details.missingGlyphError, true);
  assert.deepEqual(details.missingGlyphs, ["U+1F600"]);
  assert.match(formatPreviewError(error), /缺字.*U\+1F600/);
  assert.equal(previewCanApply(null, "fp", "fp", 100), false);
});

test("real apply response normalizes revision, scene and audit fields", () => {
  const requestId = "text-apply-request";
  const result = normalizeApplyResult(
    {
      ok: true,
      requestId,
      requestFingerprint: "a".repeat(64),
      revision: 12,
      scene: { revision: 12, boxes: [] },
      stats: { boxCount: 0 },
      font: { id: "builtin:noto-sans-sc" },
      layout: { boxCount: 3 },
    },
    requestId,
  );
  assert.equal(result.revision, 12);
  assert.equal(result.scene.revision, 12);
  assert.equal(result.layout.boxCount, 3);
  assert.throws(
    () =>
      normalizeApplyResult(
        {
          ok: true,
          requestId: "different",
          revision: 13,
          scene: { revision: 13 },
        },
        requestId,
      ),
    /状态校验/,
  );
  assert.throws(
    () =>
      normalizeApplyResult(
        { ok: true, requestId, revision: 12, scene: { revision: 11 } },
        requestId,
      ),
    /工程版本不一致/,
  );
});

test("typed missing glyph response from the HTTP layer is accepted", () => {
  const details = previewErrorDetails({
    status: 400,
    code: "FONT_MISSING_GLYPHS",
    payload: {
      ok: false,
      code: "FONT_MISSING_GLYPHS",
      error: "FONT_MISSING_GLYPHS:U+1F600",
      missingGlyphs: ["😀"],
    },
  });
  assert.equal(details.missingGlyphError, true);
  assert.deepEqual(details.missingGlyphs, ["😀"]);
});
