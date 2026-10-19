import test from "node:test";
import assert from "node:assert/strict";
import {captureSampling,viewPixelRatio} from "./render-settings.js";
test("capture supersampling respects output dimensions and GPU budgets",()=>{
 assert.throws(()=>captureSampling(2048,1024,2,1024),/设备/);
 assert.deepEqual(captureSampling(640,480,2),{factor:2,width:1280,height:960});
 assert.deepEqual(captureSampling(2048,2048,3,4096),{factor:2,width:4096,height:4096});
 assert.deepEqual(captureSampling(2048,1024,3,2048),{factor:1,width:2048,height:1024});
});
test("standard and high observation quality have predictable pixel ratios",()=>{
 assert.equal(viewPixelRatio("high",1),2);assert.equal(viewPixelRatio("standard",3),1.5);
});
