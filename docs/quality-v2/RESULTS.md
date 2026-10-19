# MorphForge 0.2.0 质量迭代记录

日期：2026-10-19。开发分支 `feat/harness-interaction-and-voxel-text`，起点为
`v0.1.0-baseline`。代码、字体及许可、文档、成功与失败证据均进入本地 Git；没有推送远端。

## 实现结果

- Harness 支持安全点插话、立即打断后同run续接、排队下一轮、暂停/恢复/停止。暂停冻结活跃时间；旧epoch的未提交工具不能迟到修改工程。完整工具批次中的取消结果仍与call ID配对，不执行不完整SSE参数。
- 中文system/tools优先，英文版本保持相同领域接口。实际迭代到system/tools **2.3**；借鉴官方DeepSeek Harness的消息收件箱、调用回放、取消与恢复机制，来源commit见[调研](../harness-v2/upstream-research.md)。
- 字体文字使用内置Noto Sans SC或只读系统字体，生成普通可编辑Box；支持三平面、正反方向、挤出、换行和间距。预览不改草稿；应用可撤销。模型复用规范化`applyTemplate`，不需重新抄写设置。
- 文字成功回执与scene原子持久化；跨重启、日志窗口淘汰、日志写入失败仍能重放。失败持久化回滚并清理未提交快照。缓存最多8个不可变字体快照，来源hash可追溯。
- 用户可保护分组，严格保护其Box ID、位置、尺寸和样式编码；覆盖、擦除、清空、变换、历史恢复或载入作品若改变受保护集合则整体拒绝。软件约束不增加Avatar参数。
- `finish_artwork`在提交守卫下完成检查点、关联当前版本真实截图、入库和完成摘要；成功后结束assistant turn，避免多一次模型请求仅生成结束语。失败不假标完成，pending插话/排队仍继续。
- 预算按实际Provider usage累计；本地估算仅用于提前收尾。无法提前精确得知下一次输入成本，门限最多可能超出一个完整响应，不是货币账单硬上限。

## 真实模型迭代

| 轮次 | 版本/设置 | 实际结果 | 判定与下一改动 |
|---|---|---|---|
| R0 | 0.1基线/high | 256 Box；16,384输出全部耗于reasoning | 失败；保留partial拒绝，加入有界incomplete恢复 |
| R1 | system/tools2，low，240k | 687 Box，三次截图与两次真实修正 | 预算耗尽、未入库；补阶段commit、实际预算提示和可定位尺寸错误 |
| R2 | 2.1，450k | 739 Box，插话/打断/compact/入库成功 | 仍失败：覆盖8个旧building Box，图片参数误用；引入硬保护和调用模板 |
| R2b | 2.2，450k | 766 Box，五个原组完全保留，工具零错误并commit | 估算预检误拦入库；按真实usage门限，新增显式收尾工具 |
| R2c | 2.3，none，150k | 3步骤、79,626 Token、约20秒；766 Box/八组全保留并有真实三视图入库 | 全部检查通过 |
| R3 | 2.3，none，350k，空场景 | 745 Box，真实修正、阶段commit、全部VRC坐标合规 | 模型字牌遮挡判断错误且预算耗尽；保留失败，接续指令修正 |
| R3c | 2.3，none，200k | 5步骤、110,828 Token、约22秒；791 Box，七个旧组全保留，正面文字与真实三视图入库 | 全部检查通过 |

各轮改变了设置或任务约束，不是单变量A/B实验；没有反复跑相同输入直到碰巧成功。
R2b在新隔离目录恢复R1原几何和首次compact前的Conversation，未把受损R2当新基准。
R2c/R3c是明确的连续输入修正/收尾，不能写成一次提示自动完成。

[全部输入与证据说明](../harness-v2/evals/README.md)；每轮保留case hash、版本、工具调用、错误、usage、场景、事件、三视图和冻结二进制hash。

图像已人工审阅：

- [云港正面](../harness-v2/evals/r2c-closeout/front.png)：16号真实体素“云港”、独立牌匾及双灯可辨，旧组未变；该艺术版仍含负Y，不直接发布VRC。
- [星港正面](../harness-v2/evals/r3c-front-text/front.png)：正面体素“星港”可读；791 Box全部满足VRC坐标范围。按无损修正要求保留了旧背面字组，未伪称清除。

## 自动与浏览器验证

最终统一命令、退出码和原始输出哈希见 [automated-checks.json](automated-checks.json)；逐项目标核对见 [COMPLETION-AUDIT.md](COMPLETION-AUDIT.md)。

- Linux `cargo test --workspace`：**74 passed，0 failed，0 ignored**。
- Workspace Clippy `-D warnings` 与Rust格式检查通过。
- 前端Node：**24 passed**；Chromium交互mock：**31 passed**。
- 真实Rust+Chromium文字三平面/缺字/预览：24项通过；文字应用、回执与撤销另有Windows原生59项报告覆盖。
- 真实Harness UI（loopback mock Provider）：17项通过。
- 真实保护专项：25项通过；发现的轮询竞争与空组显示问题已修复。
- 手工绘制、合并、擦除、undo、5k/16k、截图与Recipe回归：14项通过；16,384实例为7次绘制调用。
- 测试服务被重新编译替换的可变路径造成过一次Recipe子进程失败；已改固定二进制副本重跑通过，失败报告保留。

浏览器证据在[ui-v2](../ui-v2/README.md)与[手工回归](../workbench/ui-v2-regression-validation.json)。测试未向真实模型发送UI测试垃圾输入，自动OSC保持禁用。

## Avatar、Windows与样例

最终FullFT prefab与已保存上传场景都无`minminmean.supine`递归依赖；生成入口自动清理，SDK预算仍256/256，完整FT为161bit。原dirty Stage3保持原状，未Play/上传/启动VRChat；见[无睡眠审计](../avatars/README.md)。

Windows使用69个冻结构建输入，manifest与聚合hash见[source-freeze.json](source-freeze.json)。原生74项与桌面2项通过，NSIS正常构建。隔离Windows原生HTTP验证59项、桌面安装/启动/正常退出/卸载26项全部通过；实际HTTP嵌入的前端及OFL字节hash均与冻结输入相符，安装目录许可一致。详见[Windows验收](WINDOWS-V2.md)、[原生报告](windows-v2-native-final.json)与[桌面报告](desktop-v2-validation.json)。安装器13,613,538字节，SHA-256为`ce30b61609ff93c250840560d14ab4278709e7bbd37eacda63f736b4fccbef03`。

两份新作品已成功加入Windows默认作品库，包含三视图与独立原始run/Recipe/archive，旧工程与作品保留；见[library-seed.json](library-seed.json)。原始事件流单独归档，不把不同工程revision混进当前用户日志。

## 实际边界

软件闭环通过不等于任意艺术任务都能一次自动完成。真实测试仍出现数学/语法、空间遮挡判断、参数重抄与预算耗尽；保护、版本、预览指纹、检查点与连续输入使这些错误可观察和恢复。中文与拉丁文字已验证，复杂RTL/shaping暂不支持。VRC网络无远端ACK等既有限制保持，未以本轮软件测试代替真实多人验证。
