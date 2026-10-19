# 兼容 API、创作预设与 Goal

MorphForge 的编辑器、作品库与 Harness 使用同一个工作台。模型连接可以选择 Chat Completions 或 Responses；VRC 发布仍是独立的可选输出，不需要连接 VRChat 即可创作。

## 连接兼容服务

从顶部「配置」进入独立配置页，设置 Base URL、协议、模型名和本机 Key。Base URL 可以带 `/v1` 等前缀；误粘贴 `/responses` 或 `/chat/completions` 会自动移除该尾缀。模型名直接输入，不限制为 DeepSeek。模型必须支持函数工具调用。

默认通过内嵌 PNG 发送截图：Chat 使用 `image_url.url`，Responses 使用 `input_image.image_url`；均为 data URL，不调用 `/files`。只有DeepSeek官方HTTPS Responses接口可选择「Files引用」；其余地址和Chat接口强制inline，兼容旧配置时也不调用上传通道。文本模型可以关闭「向模型发送真实截图」；关闭后也不会继续附带历史图像，截图及作品仍在本机保存。无视觉能力时不能保证模型对渲染结果的视觉判断。

可关闭推理强度参数、切换流式/普通 JSON 响应，Chat 输出上限字段可选 `max_tokens` 或 `max_completion_tokens`。Chat 流式请求附带 `stream_options.include_usage`；不支持此字段的服务可切换普通 JSON。兼容服务的具体扩展不统一，本版本验证的是协议路径，不保证每个平台与模型均已实测。

更换地址时不会继承旧 Key；空 Key 仅在地址不变时保留原值。运行中和暂停期间不能更换连接，先停止当前运行即可。连接存于数据目录的 `provider-connection.json`，Unix 权限0600，Windows沿用用户数据目录ACL；状态接口、浏览器存储和诊断包不含 Key。请求/响应日志隐藏已知 Key 和内嵌图片正文，通过本地 artifact 定位真实图片。配置文件本身包含本机 Key，不应分享。

环境变量：`MORPHFORGE_API_BASE_URL`、`MORPHFORGE_API_TYPE=chat|responses`、`MORPHFORGE_API_KEY`。未创建本机连接文件时才读取环境；保留旧 DeepSeek 环境配置，但 `DEEPSEEK_API_KEY` 只隐式用于官方 DeepSeek host，不会随自定义地址发送。配置也可从原有 `~/litellm/.env` 读取；已有连接文件优先。

## 三档预设

选择预设时应用以下默认值，随后可逐项修改；语言切换同步使用该档的中文或英文提示词。旧设置载入时保留原预算，不自动切到高消耗档。

| 预设 | 创作优先级 | 最大步骤 | 累计Token上限 | 活动时间上限 | 每步输出上限 | 上下文窗口 / compact阈值 | Goal默认 |
|---|---|---:|---:|---:|---:|---:|---|
| 快速草图 `sketch` | 轮廓、比例、大形、少量材质 | 12 | 120,000 | 600秒 | 8,192 | 65,536 / 40,000 | 关闭 |
| 标准创作 `standard` | 结构、材质分区、有限细节 | 24 | 250,000 | 900秒 | 16,384 | 131,072 / 60,000 | 关闭 |
| 精细作品 `detailed` | 分阶段细化、视觉焦点、多轮修正 | 128 | 2,000,000 | 7,200秒 | 32,768 | 262,144 / 180,000 | 开启，3轮复核，最多12次自动续作 |

这些是软件允许的消耗上限，**不是耗时、账单、质量或模型能力承诺**。服务的实际上下文/输出限制可能更小，请相应调低。大上下文会因每步重发历史增加累计输入费用；提高预算不意味着必须耗尽。可用额外细化要求与验收要求引导重点，避免仅增加 Box 数量。

预设提示词在 `crates/morphforge-app/src/harness/presets/`，中文优先，中英各三档。修改预设会重置其预算和 Goal 默认项；模型名、连接、语言、额外要求与是否发送图片保持用户设置。

## 应用内 Goal

Goal 是有预算的创作续作机制。输入目标后，模型先搭建、调用真实 `capture_views`、根据图像修改，再用 `finish_artwork` 保存。普通文字结束但目标尚未通过保存时，Harness 自动提醒续作，共用同一运行的步骤、Token、活动时间限制。

- 复核以不同几何内容的真实截图计数；同一个 revision 的多视角或重复截图只算一轮。材质配置改变但几何与外观编码未变不增加轮数。要求3轮表示首次成形截图，再经过至少两次不同几何/外观编码修改并截图。
- `finish_artwork` 要求当前 revision 的真实截图、达到复核轮数，并提供 `goalAssessment` 逐项说明目标完成情况和限制；成功提交并入库后才标记 achieved。系统验证截图、修改及保存条件，美术质量由模型自评与用户判断。
- 达到自动续作次数上限或连续三次纯文字收尾没有几何/复核进展，会进入 blocked，保留草稿。触及预算会记录 budget_exhausted；调整预算并输入继续要求可在已有草稿上继续。不会自动重开预算。
- 支持原有安全点插话、立即打断、排队、暂停恢复。暂停冻结活动时间；停止后不继续调用。排队的下一项在当前轮次结束后处理。
- compact 保留独立 Goal 进度和验收标准，同时保留原始事件、工具调用、截图、Recipe、作品版本。重启恢复记录，但不会自动发起付费调用。
- 新工程或已完成目标收到新请求会建立新 Goal；同一未完成目标的续接/插话会追加要求。工作台显示状态、复核轮数、续作次数和自评。

上下文预算保留输出与系统/工具余量；自动 compact 同时参考本地估算与服务实际输入用量。达到窗口但无法进一步整理时明确停止并保留历史，不静默裁剪。服务报告 usage 时使用实际总量；缺失时使用带事件标记的本地估算，不将请求记为零消耗。

## HTTP 与日志

- `POST /api/provider/config`：连接配置。Key只写本机，响应只返回公开状态。
- `POST /api/harness/config`：`preset`、`model`、`contextWindowTokens`、`detailInstructions`、`sendImages`、`goalMode`、`goalMinReviewRounds`、`goalMaxContinuations`、`goalCriteria`，以及原预算字段。可先应用 preset 再用同请求显式覆盖个别字段。
- `GET /api/harness/state`：公开连接状态、设置、运行状态和 `context.goal`。
- 原 `message` / `control` / `tool` API保持，`finish_artwork`新增可选 `goalAssessment`（Goal模式必填）。

事件包含预设、提示词/工具版本、请求预算、实际usage或估算来源、Goal建立/续作/复核/完成及工具失败。`runs/conversation.json`使用本地图片 artifact 引用，发送前才按所选协议转换，避免将大段base64当文本上下文保存。换服务时丢弃不兼容的内部推理项；普通文本与工具结果继续沿用。

0.4.2开始，system及工具定义在设置不变时保持固定；预算、权威状态增量和重试说明追加到尚未发送的新输入/工具结果。参考图只在选择变更或compact后追加，历史截图裁剪仅发生在compact边界。取消参考追加说明，不删除普通历史中的旧图片。开启/关闭图片、变更模型/提示设置、重置上下文和compact属于可预期的前缀边界。

`harness.request.prepared`保留`completionPhase`、`completionReserveTokens`，新增`contextPolicy`、`contextEpoch`、`runtimePlacement`和`authorityChangedFields`。`provider.request.prefix`在图片和协议转换后比较实际出站消息，记录`appendOnly`、`sharedMessages`、`firstChangedMessage`和原因；只保存指纹，不把它当作服务端token统计。`compact.completed.visualContext`记录边界上的截图裁剪。收尾提醒不改变实际usage计费或预算规则。[透明透传与token前缀验证](quality-v4.2/RESULTS.md)。

## 验证范围

本轮验证记录在 [quality-v3](quality-v3/RESULTS.md)。新增协议测试使用本地可控服务，覆盖 Chat JSON/SSE、工具结果配对、图片顺序、截断保护和 usage。真实 Chromium 流程运行实际 Rust Recipe、双视角渲染与作品入库。已有作品截图来自先前真实 DeepSeek 运行，不冒充本轮三档预设的同题美术对比。

## 用量与缓存命中率

输入框旁显示当前数据目录累计、当前运行的上游Token用量及加权Prefix命中率，包含compact。统计只使用实际usage；未报告数据不当作0%，缺失用量可能显示已知下限。悬停可看输入/输出、缓存Token分子分母与覆盖请求数；重启后从日志恢复。详见 [0.3.1验证](quality-v3.1/RESULTS.md)。
