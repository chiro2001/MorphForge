import test from "node:test";
import assert from "node:assert/strict";
import vm from "node:vm";
import { readFile } from "node:fs/promises";
const sdk = await readFile(
  new URL("../crates/morphforge-app/src/recipe_sdk.js", import.meta.url),
  "utf8",
);
const run = (code) =>
  vm.runInNewContext(
    sdk + "\n" + code + "\nJSON.parse(JSON.stringify(ops));",
    { settings: { seed: 1, maxOps: 100000, styles: { wood: 2 } } },
    { timeout: 2000 },
  );
function cells(ops) {
  const out = new Set();
  for (const b of ops) {
    if (b.op !== "box") continue;
    for (let x = b.min[0]; x < b.min[0] + b.size[0]; x++)
      for (let y = b.min[1]; y < b.min[1] + b.size[1]; y++)
        for (let z = b.min[2]; z < b.min[2] + b.size[2]; z++)
          out.add([x, y, z].join());
  }
  return out;
}
test("sphere spans equal an independent cell-center oracle", () => {
  const actual = cells(run('mf.sphere([0,0,0],3,{style:"wood"});'));
  const oracle = new Set();
  for (let x = -3; x < 3; x++)
    for (let y = -3; y < 3; y++)
      for (let z = -3; z < 3; z++)
        if ((x + 0.5) ** 2 + (y + 0.5) ** 2 + (z + 0.5) ** 2 <= 9)
          oracle.add([x, y, z].join());
  assert.deepEqual(actual, oracle);
});
test("fractional ellipsoid parameters create integer grid boxes", () => {
  const ops = run("mf.ellipsoid([.5,2,0],[3.5,2,1.5]);");
  assert.ok(ops.length > 0);
  assert.ok(ops.every((b) => [...b.min, ...b.size].every(Number.isInteger)));
});
test("capsule/tube cover both endpoints and polygon extrusion has negative space", () => {
  const a = cells(run("mf.tube([[0,1,0],[8,1,0],[8,5,0]],1.1);"));
  assert.ok(a.has("0,1,0"));
  assert.ok(a.has("8,4,0"));
  const b = cells(
    run("mf.extrude([[0,0],[4,0],[4,1],[1,1],[1,4],[0,4]],[0,0,0],2);"),
  );
  assert.equal(b.size, 14);
  assert.ok(!b.has("3,3,0"));
});
test("replaceGroup removes only the named group and resource guards reject oversized fields", () => {
  const ops = run('mf.replaceGroup("head",()=>mf.sphere([0,0,0],2));');
  assert.equal(ops[0].op, "remove_group");
  assert.equal(ops[0].name, "head");
  assert.ok(ops.slice(1).every((b) => b.group === "head"));
  assert.throws(
    () => run("mf.voxelize({min:[0,0,0],max:[1000,1000,1000]},()=>true);"),
    /2000000/,
  );
});
