# MorphForge 第二轮质量迭代

本轮以 Git 基线 `abc639c`（`v0.1.0-baseline`）开始，开发分支为 `feat/harness-interaction-and-voxel-text`。源代码、字体许可、接口和评测证据分阶段提交，安装器作为发布产物保留而不提交二进制；不自动推送远端。

## 交付目标

1. 对齐 DeepSeek 官方 Harness 的协议与上下文经验，记录调研 commit；保留本项目的体素领域工具和受限 Recipe。
2. 明确区分插话、打断、排队；暂停和恢复有明确的状态、取消边界、消息 ACK 和日志。不得执行不完整 SSE 工具调用或出现停止后的晚到几何提交。
3. 内置可分发中文字体及字体目录，通过字形覆盖栅格生成普通 Box。人工编辑器、模型工具、Recipe 使用同一实现，支持预览、事务提交、撤销及来源记录。
4. 提示词与工具描述优先中文，提供英文版本；记录 prompt/tool/SDK 版本以便评测比较。
5. 多轮真实模型任务必须留存截图与问题分析，再调整提示词/工具。不能仅把重复跑同一配置算作迭代。
6. Windows 原生与 Tauri 重新构建，补齐运行指南及合理测试。

## 核心接口

- `POST /api/harness/message`：`mode=steer|interrupt|queue`；支持 requestId 幂等与 runId 冲突检查。
- `GET /api/fonts`：字体元数据及稳定 ID，不暴露系统文件路径。
- `POST /api/text/preview`：纯预览，返回字形尺寸、普通 Box 操作和指纹，不修改草稿。
- `POST /api/text/apply`：带 baseRevision/requestId 的一次事务，支持撤销。
- `POST /api/project`：单独提交 `protectedGroups` 与 `baseRevision`；变更保护策略生成新revision，旧客户端不能覆盖新策略。
- 模型 `voxel_text.preview` 返回规范化 `applyTemplate`，避免手工重抄坐标、group或对齐；apply通过稳定call ID幂等提交。
- 模型 `finish_artwork`：明确的收尾动作，同一工具完成当前版本commit、关联真实截图保存作品并给出完成摘要，避免再发一轮模型请求仅生成结束语。

`steer` 在下一安全点插入；`interrupt` 取消当前请求后接续；`queue` 在当前轮结束后执行。已提交的几何不因停止被隐式回滚，原始日志也不删除。

文字回执随scene在同一个`project.json`原子保存，不依赖UI事件的最近5000条窗口；日志失败、正常重启和材质目录变化不使同一成功请求重复生效。字体解析缓存最多8项，采用解析时不可变快照和实际内容hash。

保护约束比较受保护组的完整Box多重集合（含ID、位置、尺寸及样式编码），碰撞、擦除、重建、历史恢复或载入作品一旦改变该集合就整次拒绝。当前保护策略只由用户设置；历史/作品加载不能偷偷解除。它是软件工程约束，不增加Avatar同步参数。

## 评测方法

基线由独立数据目录和冻结的0.1.0二进制运行，随后使用相同任务和新中文工具说明对比。后续轮次加入体素文字、快速插话、打断、暂停、compact后继续与已有几何保护。每轮记录：

- Git与prompt/tool版本、模型设置、预算、输入任务；
- 工具调用/错误、几何版本与数量、艺术库保存；
- hero/front/top截图和实际人工图像审阅；
- 消息接受/应用顺序、中断是否阻止晚到提交；
- 本轮问题、具体改动、下轮检查项。

`tools/evaluate_harness.py`仅连接显式loopback测试实例，保持OSC禁用；不会读取Key。凭证由后端读取已有本机配置。

## Avatar边界

最终FullFT模型不安装睡眠插件/Supine生成器；本轮重新核对最终依赖，256bit预算不计入不存在的睡眠参数节省。不改用户Unity未保存场景，不自动上传、不启动VRChat。现有真实OSC漏绘边界继续保留，不用软件质量迭代掩盖协议限制。
