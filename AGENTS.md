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

- 用户验收版保持在 `%LOCALAPPDATA%\MorphForge Desktop`，数据为 `%LOCALAPPDATA%\MorphForge`。用户可能持续使用，不因开发更新而关闭或覆盖它。
- 默认更新独立开发测试副本 `%LOCALAPPDATA%\MorphForge Preview`，使用 `tools/install_preview.ps1` 部署；桌面入口为“MorphForge - 开发测试副本”。数据、对话、日志及 WebView2 缓存独立。
- 副本以内容哈希保存不可变版本目录，更新只切换 `current.json`；已经运行的旧副本继续使用旧目录，用户关闭重开后进入新版本。不要覆盖仍在运行的 `morphforge.exe`，recipe 子进程依赖 `current_exe()`。
- 首次可复制当前作品、作品库和配置；后续保留副本自己的数据，不自动和验收版合并或双向同步。
- 仅在用户明确要求发布时创建 GitHub Release。
