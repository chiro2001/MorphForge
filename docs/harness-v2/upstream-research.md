# Harness v2 上游调研

## 调研范围

只参考 DeepSeek 官方仓库 `https://github.com/deepseek-ai/deepseek-harness` 的源码和仓库文档，没有复制其实现代码，也不引入其通用 coding-agent 工具集。

调研固定在 commit `687f282e8625c6b8d11ff58211a90524be2e4317`（2026-10-17，`release-dsh-0.2.1-rc.2` 合并提交）。上游声明 MIT License，并将项目标为 developer preview、提醒会发生兼容性破坏变更。因此这里借鉴的是协议与生命周期原则，不把该 commit 当稳定依赖。

主要核对的上游文件：

- `packages/core/agent-loop/src/agent.ts`
- `packages/core/agent-loop/src/tool-calls.ts`
- `packages/core/agent-loop/src/inbox.ts`
- `packages/core/session/src/repair.ts`
- `packages/llm/llm/src/types.ts`
- `packages/llm/llm/src/message.ts`
- `packages/core/agent-loop/README.md`
- `packages/core/agent/README.zh.md`
- `docs/user/develop/basic/tool.zh.md`

## 消息、工具与回放

上游把普通 follow-up 与 step steering 分成两个收件箱目标：`followup` 进入 `next-turn`，`steer` 进入 `next-step`。步骤入口先消费 steering，再按需消费一个 queued turn；已接受的输入成为后续模型请求历史。取消默认清空待处理输入，`keepInbox` 才会在中断后保留它。

模型消息以有类型的内容块表示。assistant 内容可以同时包含可见文本、reasoning 和 tool-call；工具调用有稳定 call ID，工具结果引用相同 ID。上游将 assistant 消息、tool/call、tool/result 分开写入 session 日志，并把结果与 call 事件关联，因此 UI 回放和下一次模型请求能重建同一调用顺序。MorphForge 使用 Responses API 时保留完整 `response.output` 数组的原始顺序与结构，包括 reasoning、message 和 function_call；在所有 function_call 后逐个附加同 ID 的 function_call_output。不得把 reasoning 拼进用户可见文本，也不得只记录工具结果摘要而丢弃配对身份。

工具只在完整的模型响应被接受后派发。取消流时，上游会保存可回放的中断 assistant 前缀；若该步没有可接受内容，则只留下 attempt 记录。已开始的工具需要先收敛；未派发的 call 也会得到错误结果配对。恢复日志时，缺少结果的已开始 call 标成 outcome unknown，未开始的 call 标为 not started；这些结果提醒模型先确认外部状态，避免盲目重试有副作用的工具。MorphForge 对不完整 SSE 不执行任何工具；若响应已完整但工具批次被取消，则为批次中每个 call 记录成功或 cancelled 结果，再接收插话。

## 中断、暂停与安全边界

上游把“插话”和“中断”分开：steer 不抢断当前步骤，只在下一个 step 边界进入；follow-up 在当前 turn 完成后进入下一 turn；cancel 中止当前活动。MorphForge 进一步提供 `queue`、`steer`、`interrupt` 三种 API 模式：queue 在当前助手轮次完成后排入下一轮；steer 在完整响应和工具结果配对后、下一个模型请求前应用；interrupt 立即取消当前请求/未提交工具，并在同一 run 用新 epoch 续接。空文本 interrupt 作为暂停处理：先取消当前尝试，冻结 active-time 预算，等待 resume。

每个消息 requestId 只接受一次；重放相同 ID 返回原 ACK，不重复入队。可选 runId 必须仍指向当前 run，否则以 `RUN_CONFLICT` 拒绝。每次暂停、恢复或中断都改变 epoch；工具提交在同一锁序下检查 runId、epoch 与 cancellation token，避免接受 interrupt/stop 后再发生迟到提交。停掉的 run 保留已提交的几何、完整 assistant/tool 配对与事件记录。

## 提示词与领域工具

上游 prompt 将角色、系统边界和工作目录等上下文分开组装；tool schema 本身提供短而具体的 description 与受约束参数。对于 MorphForge，应保留自己的体素领域流程（先 `get_context`，用 `run_recipe` 批量构建，调用 `capture_views` 检查，按需 commit/publish），并补充字体预览/应用工具。不要照抄 shell、文件系统、浏览器、代码编辑、MCP 等无关工具。

中文为默认界面和模型交互语言，英文作为可选路由。Prompt 与 tools 分别使用独立版本号，并在 run、provider request、tool request 事件上记录语言与版本，供评测和回放确认当时模型看到的指令及 schema。紧凑 prompt 说明材质/几何限制、revision 规则、工具错误恢复及真实发布状态；不要编造 SDK 能力，也不要在每个 Box 操作后要求截图。

## MorphForge 落地对应

| 上游原则 | MorphForge 对应 |
| --- | --- |
| 有序 durable conversation | Responses `input` 完整保存 reasoning、message、function_call 与 function_call_output |
| next-step / next-turn inbox | `steer` / `queue` 两种不同的消息队列 |
| cancel signal / epoch | per-run `Cancellation` 与递增 epoch，工具提交前再次校验 |
| tool call/result pairing | 所有完整批次逐 call ID 追加输出；取消/跳过也写 cancelled 结果 |
| schema-driven domain tools | 仅保留体素建模、检查、截图、作品与文本转体素工具 |
| localized prompt surface | `language=zh-CN|en`，默认 `zh-CN`；独立 system prompt/tools 版本 |

