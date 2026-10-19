import test from 'node:test';
import assert from 'node:assert/strict';
import {selectedReferences,sourceLink} from './reference-ui.js';
test('reference source toggles can use library, current images, both or neither',()=>{
 const library=[{id:'builtin:chair'},{id:'local:a'}],input=[{id:'local:a'},{id:'local:b'}];
 assert.deepEqual(selectedReferences(library,input,false,false),[]);
 assert.deepEqual(selectedReferences(library,input,true,false).map(i=>i.id),['builtin:chair','local:a']);
 assert.deepEqual(selectedReferences(library,input,false,true).map(i=>i.id),['local:a','local:b']);
 assert.deepEqual(selectedReferences(library,input,true,true).map(i=>i.id),['builtin:chair','local:a','local:b']);
});
test('source links only accept HTTP or HTTPS',()=>{
 for(const s of ['javascript:alert(1)','file:///secret','data:text/html,hi','not url'])assert.equal(sourceLink(s),null);
 assert.equal(sourceLink('https://www.bilibili.com/opus/1'),'https://www.bilibili.com/opus/1');
});
