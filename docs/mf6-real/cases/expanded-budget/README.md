# 放宽预算后的续作案例

根据2026-10-20用户要求，将真实建模评测单独设置的累计预算放宽到每件8,000,000 Token、48个模型响应、7,200秒活动时间，每次输出上限393,216，仍为1,048,576上下文。最大值为允许的上限，不要求模型耗尽；累计输入包含重复读取的缓存历史。

软件本身的精细preset早已默认32,000,000累计Token、128步、6小时，本次无需提高产品默认值。此前30万/65万是评测者另外设置的边界；原始案例和失败报告不改写。

这两个JSON使用`continue=true`：在原本隔离的数据目录、同一模型和同一静态前缀上续作。保留官方严格缓存监控和实际usage统计，不重置对话、不重新从零生成，不降低上下文或输出容量，不启用OSC。先确认`GET /api/harness/state`没有缓存告警且存在上游实际cache hit，然后运行：

```bash
python3 tools/evaluate_harness.py \
  --url http://127.0.0.1:18901 \
  --case docs/mf6-real/cases/expanded-budget/reference-cottage.json \
  --output /path/to/isolated-evaluation/reference/expanded \
  --timeout 7300 --require-cache-hit --lifetime-token-limit 8400000
```

翠鸟使用18902及`polygon-kingfisher.json`，原目录已有用量更多，数据目录累计上限设为8,600,000。数据目录上限包含首轮测试，case预算仅覆盖本次run；上游actual usage可以在单个完整响应内越过预算，runner和后端都不会在达到上限后继续发起模型请求。

小屋要求检查未执行的模型源码修订，改善比例后重新截图、自动保存；翠鸟同时给予上轮真实拓扑诊断，要求源码诊断/修复并复核。新提示词只是追加用户消息，未更换system或工具schema。艺术质量、拓扑诊断、自动收尾和缓存命中分别记录，不用Token消耗或API成功代替质量判定。
