# MorphForge 共享前端 v2

更新时间：2026-10-19。范围是 Harness 插话交互、字体体素文字面板和前端结构整理。本轮不启动 Unity、VRChat、OSC 或部署 Windows。

## Harness 消息交互

前端调用 `POST /api/harness/message`：

```json
{
  "prompt": "增加一个门",
  "mode": "steer",
  "requestId": "uuid",
  "runId": "active-run-id"
}
```

`mode` 有三条明确路径：

| 模式        | 界面动作          | 语义                                            |
| ----------- | ----------------- | ----------------------------------------------- |
| `steer`     | 插话 · 下一安全点 | 保留当前请求，在完整工具结果后插入              |
| `interrupt` | 打断并继续        | 取消当前模型请求或未提交工具，在同一 run 内接续 |
| `queue`     | 排队下一轮        | 当前本轮结束后，在同一上下文中开始下一轮        |

空闲时主按钮为“开始创作”。运行中根据选择显示“插话 · 下一安全点”“打断并继续”或“排队下一轮”。运行控制中的“暂停”发送空 `prompt` 的 `interrupt`，及时取消在途请求、冻结后续执行与预算并保持暂停；“继续”和“停止本轮”分别使用 `/api/harness/control` 的 `resume` 和 `stop`。`control.pause` 与空 `interrupt` 是同义兼容入口；只有带文本的 `message mode=interrupt` 才会取消后在同一 run 续接。

ACK 的精确形状为：

```json
{
  "ok": true,
  "accepted": true,
  "requestId": "uuid",
  "runId": "active-run-id",
  "mode": "steer",
  "epoch": 4,
  "phase": "running",
  "queuePosition": 1
}
```

`queuePosition` 只在该消息确实进入对应收件箱时有意义。界面把内部状态转换成：

- `sending`：发送中；
- `accepted`：已接收；
- `applied`：已应用；
- `cancelled`：已取消；
- `rejected` / `failed`：拒绝或失败，保留输入并显示原因。

客户端按 `requestId` 去重。409、`RUN_CONFLICT` 和错误字符串中的同名标记会显示为“运行已变化”；不会清空用户输入。网络错误、429 和 5xx 会对同一 payload 与同一 `requestId` 自动重试一次，避免响应丢失后的重复提交。双击由同步的在途锁抑制；快速连续发送在上一 ACK 返回后可继续。

`state.harness.interaction` 读取：

```json
{
  "phase": "running",
  "epoch": 4,
  "activeRequestId": null,
  "paused": false,
  "queue": [
    {
      "requestId": "uuid",
      "runId": "active-run-id",
      "mode": "steer",
      "acceptedAt": "2026-10-19T...",
      "epoch": 4
    }
  ]
}
```

没有新字段时兼容旧 `run.status` 与 `paused`。事件流处理 `harness.message.accepted/applied/cancelled/rejected/failed`。`runId/requestId/epoch` 只用于去重、状态恢复和运行记录，不显示在普通运行状态文本中。

聊天记录按 `runId + epoch + requestId` 去重，用户插话和排队消息使用独立条目。epoch 在有效打断后变化时，上一 epoch 的 assistant 片段保留并标注“已打断 · 保留当前片段”。聊天、待处理消息和输入草稿保存在浏览器 `localStorage`；刷新后不会重复插入或丢失未发送输入。只有用户原本位于聊天底部时才自动滚动。

成功运行的最终说明由 `harness.finished` 事件携带，不额外请求模型：事件 `data` 是 Run 的 camelCase JSON，`status="completed"`，`message` 是 `finish_artwork` 的 summary。前端按事件和 `runId` 写入一条“完成总结”，保留此前 assistant 片段；同一 summary 已存在时不重复插入。重复事件、状态轮询和刷新都会走精确文字去重。

界面证据见 [完成总结截图](ui-v2-mock-complete-summary.png)：上方保留此前的 assistant 片段，下方只新增一条标注“完成总结”的最终说明。

## 分组保护

左侧每个分组现在有两个独立控件：

- 可见性勾选框仍只控制本机预览；
- “保护 / 已保护”按钮保存软件编辑约束。

切换保护状态调用：

```json
POST /api/project
{
  "protectedGroups": ["主楼", "钟楼"],
  "baseRevision": 42
}
```

状态从 `state.project.protectedGroups` 读取。分组列表取 `stats.groups` 与实际保存的保护列表并集，因此尚未包含任何 Box 的未来受保护组也显示为 `0`，仍可解除保护；否则会出现“看不到锁却无法新建”的死锁。保护列表会去重并持久化；修改工程名称和固定要求时也会携带当前 `baseRevision`。保护是 Harness/编辑器侧约束，不增加 Avatar 同步参数或消耗 Avatar bit。

保护后，任何涉及该组的绘制、擦除或变换应由后端返回 HTTP 409，错误以 `PROTECTED_GROUP_CONFLICT` 开头。界面会显示“目标分组已锁定，不能绘制、擦除或变换。请先在左侧解除保护。”，并保留尚未发送的输入和当前消息草稿。解锁后即可继续编辑。

契约 mock 已覆盖保护状态独立于可见性、409 拒绝、中文提示、输入保留和解锁后可编辑。[ui-v2-mock-protected-group.png](ui-v2-mock-protected-group.png) 展示锁定状态，机器记录见 [group-protection-mock.json](group-protection-mock.json)。

真实 Rust R2b 后端专项见 [group-protection-real-validation.json](group-protection-real-validation.json)，25 项通过：

- 锁定新增 revision，`protectedGroups` 持久化并在刷新后恢复；
- 绘制、擦除、变换分别返回 409 `PROTECTED_GROUP_CONFLICT`，revision 与几何保持不变，中文说明出现且未发送 Prompt 保留；
- 后端允许保护尚未有 Box 的未来组，首次写入被拒绝；
- 新前端将未来空组显示为 `0`，可从 UI 解锁，解锁后绘制成功；
- 全程 OSC 关闭，未产生 provider 或 Harness 消息事件。

锁定与解锁截图：[locked](group-protection-real-locked.png)、[unlocked](group-protection-real-unlocked.png)。这次测试把当前 `web/` 通过同源测试代理连接到冻结 R2b 后端，覆盖了真实 API 硬约束和当前前端；最终安装包仍需用这份前端重新构建后做一次薄烟测。

首次真实运行时发现：`/api/project` 已成功后，轮询恰好占用 `refresh()` 的互斥标志，界面可能误报“保护未保存”。现在保护设置直接校验并采用 POST 响应，再刷新；失败证据保留在 [polling-race-validation.json](group-protection-real-polling-race-validation.json)。冻结 R2b 内置旧前端导致的未来空组不可见证据另存为 [embedded-old-ui-failure-validation.json](group-protection-real-embedded-old-ui-failure-validation.json)。

## 字体体素文字

“字体文字”页发送：

```json
{
  "text": "中文\nABC",
  "fontId": "builtin:noto-sans-sc",
  "fontSize": 8,
  "depth": 2,
  "origin": [0, 0, 0],
  "plane": "xy",
  "facing": "positive",
  "align": "left",
  "letterSpacing": 0,
  "lineSpacing": 0,
  "maxWidth": null,
  "styleId": 1,
  "tintId": 0,
  "motionPhase": 0,
  "group": "text"
}
```

`origin` 是最终排版 bbox 的最小角。`positive/negative`、平面和可读方向全部由后端以 canonical Box 解释；前端不再翻 X/Y/Z。Three 渲染继续只使用已验证的 `(x,y,-z)` 边界转换。

`GET /api/fonts` 返回 `fonts`，内置字体为 `builtin:noto-sans-sc`。文字预览调用 `POST /api/text/preview`，响应中的 `operations` 与其他几何 API 一样是 `{op:"box",min,size,...}`。预览放在独立的半透明 `InstancedMesh` 层，不进入主 `boxes`、拾取、选择或正式截图，也不改变草稿 revision 或用户相机。

输入变化后等待 320 ms 再预览。预览请求带独立的 `requestId`；服务端回显相同 ID 并返回 serde 规范化后的 SHA-256 `requestFingerprint`。前端同时保留“当时发送的完整本地请求串”和“服务端指纹”，不拿浏览器 `JSON.stringify` 直接与 SHA 比较。每个请求递增序号，并比较本地发送串与响应时表单串；旧响应会被丢弃。只有当前成功预览、本地请求串未变化、服务端指纹有效、没有缺字且 Box 数不超过当前目标容量时才启用“应用到草稿”。

`POST /api/text/apply` 在预览字段外加入 `baseRevision`、新的 `requestId`，以及预览回显的 `previewRequestId` 与服务端 `previewFingerprint`。服务端应在提交前再次校验指纹；它作为一次普通几何事务提交，因此可用现有撤销恢复。版本冲突、缺字、超容量、预览 ID/指纹不匹配或响应缺少 revision 时不修改草稿。缺字当前由字体层快速失败为 `FONT_MISSING_GLYPHS`，前端兼容顶层字符串和可选的 `missingGlyphs` 列表。

## 文件结构

| 文件                | 职责                                             |
| ------------------- | ------------------------------------------------ |
| `web/studio.html`   | 可读的共享工作台结构；四个右侧页签               |
| `web/studio.js`     | 现有编辑器接线、状态轮询、Harness 与文字流程编排 |
| `web/harness-ui.js` | 交互状态归一化、按钮语义、ACK/事件状态、消息去重 |
| `web/text-ui.js`    | 文字请求、预览操作归一化、容量/缺字应用条件      |
| `web/workbench.css` | 工作台、消息状态、文字面板布局                   |
| `web/renderer.js`   | 原坐标/材质/拾取逻辑；新增独立文字预览层         |

所有 Three.js 与字体资源仍是本地 vendor/后端资源，不依赖 CDN。

## 验证

纯函数测试：

```bash
node --test web/harness-ui.test.mjs web/text-ui.test.mjs web/renderer-resources.test.mjs
```

真实 Chromium 契约流程：

```bash
chromedriver --port=9515
python3 tools/check_ui_v2.py --label ui-v2-mock
```

契约 mock 结果见 [ui-v2-mock-validation.json](ui-v2-mock-validation.json) 和截图 [ui-v2-mock-workbench.png](ui-v2-mock-workbench.png)，31 项通过，包含完成 summary 保留旧片段、重复事件和刷新去重。真实 Rust 服务与 mock provider 的结果见 [ui-v2-real-validation.json](ui-v2-real-validation.json)、对话截图 [ui-v2-real-chat.png](ui-v2-real-chat.png) 和最终工作台 [ui-v2-real-workbench.png](ui-v2-real-workbench.png)，17 项通过；该报告早于分组保护和完成 summary UI，不包含这两项测试。真实 UI 流程覆盖启动、快速插话、双击去重、带文本打断、暂停/继续、刷新保草稿、聊天去重、文字预览、缺字阻止应用和应用撤销；无 JavaScript/WebGL 运行时错误。缺字测试返回预期的 400 network 日志，报告单独列出该网络日志，不把它误报为脚本错误。Node 纯函数测试当前共 24 项通过。

机器可读汇总见 [real-validation-status.json](real-validation-status.json)。真实测试没有向付费模型发送请求。

该证据使用与实际路由同名的确定性 mock API，用来验证前端状态机和 DOM 行为；它不是 Rust Harness、真实 DeepSeek、字体栅格化或文字 apply 的全链路证明。根代理可在实际服务启动后运行：

```bash
python3 tools/check_ui_v2.py --url http://127.0.0.1:<port> --label ui-v2-real
```

真实全链路测试仍应单独检查服务端 ACK 幂等、409 runId、事件持久化、字体预览指纹与文字事务。

当前 `crates/morphforge-app/src/server.rs` 已接线 `fonts`、`text/preview` 和 `text/apply`；`harness/message` 仍由 Harness worker/root 的 WIP 接线。前端在消息路由缺席时保留旧 `harness/start` 的启动兼容，字体页会显示服务未就绪，不会伪装成已完成。文字 apply 的响应包含 `ok/revision/scene/stats/font/layout/requestFingerprint`，并接受 `previewRequestId` 与 `previewFingerprint`；前端已按该形状校验。

### 真实字体与渲染方向

`tools/check_text_ui_real.py` 只调用 state/scene/fonts/text preview，不访问 Harness、不触发模型。它在独立 UI 预览层上隐藏其他场景内容，比较预览前后的真实 WebGL 画布像素：

- 对 `xy/front`、`xz/top`、`yz/side` 分别检查 `positive` 文本的右向与下向投影；
- 将实际变化像素 bbox 与后端 canonical Box 的投影 bbox 比较；
- 用非对称字母 `L` 检查屏幕阅读坐标中的左上竖笔、右上空缺和底部横笔；水平或垂直镜像都无法通过该四区占用检查；
- 另用 `FL\n云港`、24 号体素字生成三平面可人工查看的截图；测试脚本显式让测试相机适配预览包围盒并留边，产品预览仍不移动用户相机；
- 保存三个平面的画布 PNG，并确认预览没有改变 revision；
- 直接检查 `FONT_MISSING_GLYPHS` 的 top `code`、字符串 `error` 和 `missingGlyphs`；
- `--apply` 为可选项，会用一次 apply 和 undo 验证真实事务，默认关闭。

先对契约 mock 自检：

```bash
python3 tools/check_text_ui_real.py --mock --label text-mock
```

真实服务结果见 [text-real-text-rust-validation.json](text-real-text-rust-validation.json)，26 项通过。`L` 在三平面的四区占用分别为：

| 平面 |  左上 | 右上 |  左下 |  右下 |
| ---- | ----: | ---: | ----: | ----: |
| XY   | 65.7% |   0% | 81.1% | 47.8% |
| XZ   | 66.7% |   0% | 83.2% | 49.5% |
| YZ   | 65.9% |   0% | 81.5% | 47.5% |

右上为零而底部横笔跨越左右，水平镜像和垂直镜像都会改变这个签名。真实 Noto Sans SC 字形截图：

- [XY：FL 与云港](text-real-text-rust-content-xy.png)
- [XZ：FL 与云港](text-real-text-rust-content-xz.png)
- [YZ：FL 与云港](text-real-text-rust-content-yz.png)

三张 24px 展示图都可读为从左到右的 `FL`，下一行“云港”保持正向且完整入画留边。真实方向测试的像素 bbox 与 canonical Box 投影覆盖率为 99.1% 至 99.5%。该测试还完成一次真实 apply 和 undo。mock 自检保留在 [text-real-text-mock-validation.json](text-real-text-mock-validation.json)，用于在没有服务时复现测试流程。

运行结束后真实测试实例为 revision 6、0 Box、Harness `completed`，没有停留在活动运行或未完成预览。

真实服务就绪后运行：

```bash
python3 tools/check_text_ui_real.py --url http://127.0.0.1:8781 --label text-rust
```

需要真实 apply/undo 时再加 `--apply`。该测试不调用 `/api/harness/message`。
