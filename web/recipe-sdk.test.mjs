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
test("colored indexed mesh and triangle strips preserve winding and transform frames", () => {
  const faces=run('mf.translate([10,2,3],()=>mf.mirror("x",()=>mf.triangleStrip([[0,0,0],[1,1,0],[2,0,0],[3,1,0]],{colors:["#ff0000",[0,255,0]],doubleSided:true})));');
  assert.equal(faces.length,2);
  assert.deepEqual(Array.from(faces[0].color),[255,0,0]);
  assert.deepEqual(Array.from(faces[1].color),[0,255,0]);
  assert.equal(faces[0].doubleSided,true);
  assert.ok(faces.every(f=>f.op==="triangle"&&f.group==="main"));
  const normals=faces.map(f=>{const [a,b,c]=f.vertices;return (b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0]);});
  assert.ok(normals.every(n=>n<0));
  assert.throws(()=>run('mf.mesh([[0,0,0]],[[0,1,2]])'),/indices/);
  assert.throws(()=>run('mf.triangle([0,0,0],[1,0,0],[2,0,0])'),/degenerate/);
});
test("wedge emits a closed eight-face prism with per-face colors",()=>{
  const faces=run('mf.group("roof",()=>mf.wedge([0,0,0],[10,6,8],{direction:"-z",color:"#aa6633"}));');
  assert.equal(faces.length,8);
  const edges=new Map();
  for(const face of faces){assert.equal(face.group,"roof");for(let i=0;i<3;i++){const edge=[face.vertices[i].join(),face.vertices[(i+1)%3].join()].sort().join("|");edges.set(edge,(edges.get(edge)||0)+1);}}
  assert.ok([...edges.values()].every(n=>n===2));
});
test("textured slopes preserve material and mirror shape as a single object",()=>{
 const ops=run('mf.translate([4,2,0],()=>mf.mirror("x",()=>mf.slope([1,0,0],[2,3,4],{style:"wood",direction:"+x"})));');
 assert.equal(ops.length,1);assert.equal(ops[0].shape,2);assert.equal(ops[0].styleId,2);assert.deepEqual(Array.from(ops[0].min),[1,2,0]);
 assert.throws(()=>run('mf.mirror("y",()=>mf.slope([0,0,0],[1,1,1]));'),/Y mirror/);
});
