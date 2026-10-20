import test from "node:test";
import assert from "node:assert/strict";
import {inverseTextureRotation} from "./texture-orientation.js";
test("24 material rotations form distinct proper orthogonal cube frames",()=>{
 const frames=new Set();
 for(let i=0;i<24;i++){
  const [a,b,c]=[[1,0,0],[0,1,0],[0,0,1]].map(v=>inverseTextureRotation(v,i));
  const cross=[a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]];
  assert.equal(cross.reduce((s,v,k)=>s+v*c[k],0),1);
  frames.add(JSON.stringify([a,b,c]));
 } assert.equal(frames.size,24);
});
test("log side variants map local top to the required world axis",()=>{
 assert.deepEqual(inverseTextureRotation([1,0,0],4).map(v=>v||0),[0,-1,0]);
 assert.deepEqual(inverseTextureRotation([0,0,1],8).map(v=>v||0),[0,1,0]);
});
