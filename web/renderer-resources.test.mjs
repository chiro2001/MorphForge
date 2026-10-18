import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import test from 'node:test';

const source=readFileSync(new URL('./renderer.js',import.meta.url),'utf8');
const start=source.indexOf('export const MAX_RESOURCE_FRAMES');
const end=source.indexOf('export class VoxelRenderer');
assert.ok(start>=0&&end>start,'renderer resource helpers must remain extractable');
const helpers=new Function(
  `${source.slice(start,end).replaceAll('export ','')}
   return {MAX_RESOURCE_FRAMES,resourceAtlasLayout,resourceFrame};`
)();

test('material frame count overrides a static source image',()=>{
  assert.deepEqual(helpers.resourceAtlasLayout(16,16,17),{frames:17,resolution:16});
});

test('animated source strips remain visible without material metadata',()=>{
  assert.deepEqual(helpers.resourceAtlasLayout(16,64,0),{frames:4,resolution:16});
});

test('explicit single-frame material keeps a tall static image static',()=>{
  assert.deepEqual(helpers.resourceAtlasLayout(16,64,1),{frames:1,resolution:16});
});

test('large timelines reduce atlas resolution to stay inside WebGL limits',()=>{
  const layout=helpers.resourceAtlasLayout(128,128,1024,16384);
  assert.equal(layout.frames,1024);
  assert.equal(layout.resolution,16);
  assert.ok(layout.resolution*6<=16384);
  assert.ok(layout.resolution*layout.frames<=16384);
});

test('frame selection is cyclic and honors material frame time',()=>{
  assert.equal(helpers.resourceFrame(6,.05,0),0);
  assert.equal(helpers.resourceFrame(6,.05,.049),0);
  assert.equal(helpers.resourceFrame(6,.05,.05),1);
  assert.equal(helpers.resourceFrame(6,.05,.39),1);
  assert.equal(helpers.resourceFrame(6,.05,.30),0);
});
