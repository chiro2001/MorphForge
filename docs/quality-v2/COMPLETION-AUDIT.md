# 0.2.0 交付核对

日期：2026-10-19。本表按授权的质量迭代目标核对，不能代替用户正在进行的手工验收或真实VRChat多人验证。

| 要求 | 实际实现与直接证据 | 结论 |
|---|---|---|
| Git基线及阶段提交 | `v0.1.0-baseline`，字体、Avatar、Harness、共享前端、收尾工具及Windows构建分别提交；候选tag `v0.2.0-rc.1`及完整Git bundle包含本轮修复；最终`v0.2.0`等待客户端确认 | 源码候选交付完成，正式客户端验收待完成 |
| 最终模型不依赖睡眠插件 | `FullFTBuild.cs`生成NoSleep控制器、JointBuild前置拒绝；[prefab审计](../avatars/no-sleep-audit.json)、[已保存场景审计](../avatars/fullft-scene-no-sleep-audit.json)、[SDK预算](../avatars/final-budget-post-fix.json) | prefab/场景/生成入口均覆盖，161bit完整FT保留 |
| DeepSeek官方Harness对齐及独立Windows核心 | [固定上游commit调研](../harness-v2/upstream-research.md)，Responses有序output与call ID结果配对；Rust HTTP/CLI独立，Tauri壳仅管理生命周期；[Windows原生报告](windows-v2-native-final.json) | 实现与Windows二进制通过 |
| 插话、打断、排队、暂停恢复、compact与日志 | `provider_harness.rs`测试分别覆盖安全点/批次取消/过期提交/同run续接/暂停冻结/compact失败与保留；[真实UI17项](../ui-v2/ui-v2-real-validation.json)；[R2b真实交互](../harness-v2/evals/r2b-protected/report.json) | 功能通过；R2b整体仍失败于旧估算预算，失败记录未删 |
| 真字体生成体素及统一接入 | `text.rs`/`fonts.rs`生成普通Box，`mf.text`、`voxel_text`、HTTP与共享UI同实现；[真实三平面24项](../ui-v2/text-real-text-rust-validation.json)，[Windows预览/应用/回执/撤销](windows-v2-native-final.json)，`text_receipts.rs`跨重启与失败回滚测试 | 字形、方向、API、持久化与用户操作均有覆盖 |
| 中文优先与英文版本 | system/tools2.3双语，`harness.rs`默认zh-CN；Windows报告逐工具核对中英文参数schema相同、描述各自语言 | 已通过；字体复杂RTL/shaping明确不支持 |
| 多轮真实模型与提示词/工具迭代 | [各轮输入和评测](../harness-v2/evals/README.md)：R0/R1/R2/R2b/R3失败原因、2.0→2.3改动、事件/Recipe/图片/usage；R2c/R3c通过并入库；两份新样例含三视图已加入用户库 | 连续指导与恢复闭环已证明，未声称复杂任务一次自动完成 |
| Windows构建、Tauri、字体许可、文档及适当测试 | [统一命令和原始输出](automated-checks.json)、[Windows59项](windows-v2-native-final.json)、[安装/退出/卸载26项](desktop-v2-validation.json)，69输入冻结，HTTP实际嵌入资源与安装许可hash一致；[用户指南](../USER-GUIDE.md) | 交付检查完成；WindowsWebView按钮级自动化未实现，未伪称覆盖 |
| 用户数据/密钥/Unity边界 | [样例增量导入](library-seed.json)保留旧project；源代码和全部Git历史密钥检查无命中；初期软件验收保留dirty Stage3；后续用户明确允许关闭旧现场并授权SDK上传，只更新本项目私有诊断ID；未启动VRChat或向9000发包 | 会话授权边界保持；正常桌面程序保留运行 |

已停止本轮Linux测试服务及其截图子进程：[linux-cleanup.json](linux-cleanup.json)。Windows隔离验收任务、进程与临时安装已清理；用户请求手测后新安装并打开的正常0.2.0应用不属于待清理测试进程。

用户手工验收状态：已打开正常安装版0.2.0，确认Key配置、OSC关闭、无活动模型run；已给出手工编辑、字体文字、作品库第一轮操作。尚未收到结果，不记为通过。后续按用户反馈继续AI交互与可选OSC联调。

后续Avatar诊断已完成：原16384×1 metadata布局改256×64；补齐真实FullFT场景前缀识别及人物特效Pickup碰撞力修复。最终私有`MorphForge FullFT 16K`已上传，实际包逐项参数256/FT161、Store16384/Infinity、完整材质及零违规碰撞力；服务端三项终态200，见[最终通过摘要](../security-check-failure/final-fullft-passed.json)。

客户端实际加载、多人及硬件抓取仍待用户现场确认，[步骤](../security-check-failure/CLIENT-RETEST.md)。因此不把整个项目记为“客户端手工验收通过”，候选源码标签与自动/服务端完成范围分别记录。
