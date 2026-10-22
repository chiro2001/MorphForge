import {test} from 'node:test';
import assert from 'node:assert/strict';
import {capsuleRayDistance as hit} from './drawing-math.js';
test('long segment picks near either endpoint and rejects beyond its cap',()=>{
  assert.equal(hit([0,49,-5],[0,0,1],100),4.5);
  assert.equal(hit([0,-49,-5],[0,0,1],100),4.5);
  assert.equal(hit([0,51,-5],[0,0,1],100),null);
});
test('end-on, cap, tangent and inside rays remain selectable',()=>{
  assert.equal(hit([0,-60,0],[0,1,0],100),9.5);
  assert.equal(hit([0,60,0],[0,-1,0],100),9.5);
  assert.equal(hit([.5,0,-5],[0,0,1],100),5);
  assert.equal(hit([0,0,0],[1,0,0],100),.5);
});
