# MorphForge 协作说明

- 使用简体中文回复用户。
- Unity 工程位于 `192.168.101.5` 的 `D:\Projects\VRChatProjects2\MorphForge`；本目录保存访问工具、源脚本与验证记录。
- 操作前阅读 [UNITY-GUIDE.md](UNITY-GUIDE.md)，使用 [.agents/skills/unity-pipeline/SKILL.md](.agents/skills/unity-pipeline/SKILL.md)。
- `python3 tools/unity_remote.py <命令>` 始终显式选择 MorphForge；不要省略项目路径去操作其他 Editor。
- 临时文件使用 Linux `~/tmp/morphforge/` 和 Windows 用户目录 `tmp\morphforge\`。
- Pipeline 复用 Roadster 的 Unity 2022 适配包，先阅读其 `Documentation~/unity2022-compatibility.md`，不能用未适配的注册表包直接覆盖。
- 创建或修改 Unity 序列化资源通过 Pipeline；批量脚本放 `AgentScripts/`，用 `run_script` 调用。
- Nemesis 使用用户已有资源，原模型与商业资产留在 Windows 工程，不公开发布。
- 当前范围是 Avatar + OSC 几何显示方案；研究文档见相邻 `../vrc-ai-world/`。暂不做语音识别。
- 不自动上传 Avatar、启动真实 VRChat 客户端或改动其他项目场景。

## gamepc 软件更新约定

- 用户于2026-10-21要求：当前使用主程序期间，更新和测试使用额外副本及隔离数据；本次不关闭、覆盖或切换用户正在使用的主安装，不复制其活动任务到测试环境。
- 用户于2026-10-20明确改为：后续默认更新**主安装** `%LOCALAPPDATA%\MorphForge Desktop`，原数据 `%LOCALAPPDATA%\MorphForge` 保留。更新前备份程序和数据，使用正式Windows安装包，并验证实际安装版本及桌面入口。
- 正在运行时先检查是否有AI任务和未完成操作；不覆盖运行中的核心可执行文件（Recipe依赖current_exe）。可在授权的更新范围内正常关闭程序后安装，避免强制结束任务。
- 原独立开发测试副本 `%LOCALAPPDATA%\MorphForge Preview` 与其Data/Profile继续保留；不自动复制或合并主安装与开发副本的数据。
- 自动GUI验收使用临时隔离数据/浏览器配置，先校验项目ID、版本和内嵌源码哈希，测试窗口标记为自动验收，不能假设用户没有操作过已有窗口。
- 仅在用户明确要求发布时创建 GitHub Release。
