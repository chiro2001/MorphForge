import test from "node:test";
import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";
const source=await readFile(new URL("./mesh-ui.js",import.meta.url),"utf8");
const {integerPoint,rgb8}=await import("data:text/javascript;base64,"+Buffer.from(source).toString("base64"));
test("mesh editor preserves signed integer points and exact RGB8",()=>{
  assert.deepEqual(integerPoint("-128, 0, 127"),[-128,0,127]);assert.deepEqual(integerPoint("2 4 8","size",true),[2,4,8]);assert.deepEqual(rgb8("#Af037C"),[175,3,124]);
  for(const value of ["1,2","1,2,3,4","NaN,0,0","0.5,1,2","1000001,0,0",""])assert.throws(()=>integerPoint(value));
  assert.throws(()=>integerPoint("0,1,1","size",true));assert.throws(()=>rgb8("#abc"));
});
