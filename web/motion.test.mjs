import { test } from "node:test";
import assert from "node:assert/strict";
import {
  keyframeAt,
  sampleTrack,
  transformPoint,
  poseBounds,
  quaternionXYZ,
  slerp,
} from "./motion.js";
const key = (
  time,
  translation = [0, 0, 0],
  rotation = [0, 0, 0],
  scale = [1, 1, 1],
) => ({ time, translation, rotation, scale });
const track = {
  group: "door",
  pivot: [2, 0, 0],
  interpolation: "linear",
  keys: [key(0), key(2, [4, 0, 0], [0, 90, 0], [2, 2, 2])],
};
test("linear translation/scale and clamped endpoints agree with keyframe semantics", () => {
  assert.deepEqual(sampleTrack(track, 1).translation, [2, 0, 0]);
  assert.deepEqual(sampleTrack(track, -1).scale, [1, 1, 1]);
  assert.deepEqual(sampleTrack(track, 3).scale, [2, 2, 2]);
});
test("pivoted rotation is applied in canonical voxel coordinates", () => {
  const p = transformPoint([3, 0, 0], track, sampleTrack(track, 2));
  assert.ok(Math.abs(p[0] - 6) < 1e-10);
  assert.ok(Math.abs(p[2] + 2) < 1e-10);
});
test("step interpolation switches exactly at a key and rotation takes shortest path", () => {
  assert.deepEqual(
    sampleTrack({ ...track, interpolation: "step" }, 1.99).translation,
    [0, 0, 0],
  );
  assert.deepEqual(
    sampleTrack({ ...track, interpolation: "step" }, 2).translation,
    [4, 0, 0],
  );
  const a = quaternionXYZ([0, 170, 0]),
    b = quaternionXYZ([0, -170, 0]),
    mid = slerp(a, b, 0.5);
  assert.ok(Math.abs(mid[1]) > 0.999);
});
test("animated bounds include all cuboid corners and triangle points; source stays unchanged", () => {
  const boxes = [{ id: 1, group: "door", min: [2, 0, 0], size: [1, 1, 1] }],
    triangles = [
      {
        id: 2,
        group: "still",
        vertices: [
          [0, 0, 0],
          [0, 1, 0],
          [1, 0, 0],
        ],
      },
    ],
    before = JSON.stringify(boxes);
  const result = poseBounds(boxes, triangles, { tracks: [track] }, 2);
  assert.ok(Math.abs(result[0].min[0] - 6) < 1e-9);
  assert.deepEqual(result[1].min, [0, 0, 0]);
  assert.equal(JSON.stringify(boxes), before);
});

test("inserting an intermediate key starts at the displayed interpolated pose", () => {
  const key = keyframeAt(track, 1);
  assert.ok(Math.abs(key.rotation[1] - 45) < 1e-8);
  assert.deepEqual(key.translation, [2, 0, 0]);
  const full = {
    ...track,
    keys: [
      { ...track.keys[0], rotation: [0, 0, 0] },
      { ...track.keys[1], rotation: [0, 0, 360] },
    ],
  };
  assert.deepEqual(keyframeAt(full, 2).rotation, [0, 0, 360]);
});
