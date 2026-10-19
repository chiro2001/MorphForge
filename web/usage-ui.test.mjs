import test from 'node:test';
import assert from 'node:assert/strict';
import {usageDisplay,supportsFileUpload} from './usage-ui.js';

test('missing cache metadata is not shown as zero percent',()=>{
  const display=usageDisplay({total:{requests:1,reportedRequests:1,totalTokens:1000}});
  assert.equal(display.tokens,'API累计 1,000 Token');
  assert.equal(display.prefix,'Prefix 未报告');
});
test('prefix hit rate uses the backend weighted ratio and reports partial coverage',()=>{
  const display=usageDisplay({total:{requests:3,reportedRequests:2,unreportedRequests:1,totalTokens:1100,prefixHitRate:0.1,cacheReportedRequests:2,cacheReportedInputTokens:1000,cachedInputTokens:100},run:{requests:1,reportedRequests:1,totalTokens:200}});
  assert.equal(display.tokens,'API累计 ≥1,100 Token');assert.equal(display.prefix,'Prefix 10.0%');
  assert.equal(display.run,'本轮 200 Token');assert.match(display.prefixTitle,/2 \/ 3/);
  assert.equal(usageDisplay({total:{requests:1,reportedRequests:0}}).tokens,'API累计 未报告');
});
test('zero reported cache hit is a known zero, and Files is official-only',()=>{
  assert.equal(usageDisplay({total:{prefixHitRate:0}}).prefix,'Prefix 0.0%');
  assert.equal(supportsFileUpload('https://api.deepseek.com/v1','responses'),true);
  for(const base of ['https://api.deepseek.com.evil.test','http://api.deepseek.com','https://api.deepseek.com:1234','http://127.0.0.1:9001','bad']) assert.equal(supportsFileUpload(base,'responses'),false);
  assert.equal(supportsFileUpload('https://api.deepseek.com','chat'),false);
});
