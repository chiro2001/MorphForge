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
