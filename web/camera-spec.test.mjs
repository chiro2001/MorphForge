import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
const code = await readFile(
  new URL("./camera-spec.js", import.meta.url),
  "utf8",
);
const { resolveCamera, fitBounds } = await import(
  "data:text/javascript;base64," + Buffer.from(code).toString("base64")
);
test("front orthographic framing uses the selected asymmetric group", () => {
  const boxes = [
    { min: [-3, 0, 5], size: [6, 12, 2], group: "face" },
    { min: [-100, 0, 0], size: [1, 1, 1], group: "far" },
  ];
  const bounds = fitBounds(boxes, { group: "face" });
  const c = resolveCamera(
    bounds,
    { yaw: 0, pitch: 0, projection: "orthographic" },
    2,
  );
  assert.deepEqual(c.target, [0, 6, 6]);
  assert.ok(c.position[2] > 7);
  assert.equal(c.position[0], 0);
  assert.ok(c.orthographic.top > 6);
  assert.equal(c.orthographic.right, c.orthographic.top * 2);
});
test("top camera has a valid up vector and explicit camera is preserved", () => {
  const b = { min: [0, 0, 0], max: [10, 2, 20] };
  const top = resolveCamera(b, { pitch: 90, yaw: 0 });
  assert.ok(top.up.every(Number.isFinite));
  const explicit = resolveCamera(b, {
    position: [40, 5, 30],
    target: [0, 1, 2],
  });
  assert.deepEqual(explicit.position, [40, 5, 30]);
  assert.deepEqual(explicit.target, [0, 1, 2]);
});
test("missing group is an error instead of silently framing the whole scene", () =>
  assert.throws(() => fitBounds([], { group: "missing" })));
