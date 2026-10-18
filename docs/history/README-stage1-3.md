# Stage1–3 历史入口与环境配置

本页保留早期 README 的原型记录。当前容量、Rust/AI 实现、安装包与 FullFT 预算以 [当前入口](../../README.md) 和 [实施状态](../plan/STATUS.md) 为准。

## 历史原型证据

以下为 Stage1–3 过程记录：其中 64 Box、MF2/88 bit、旧 Python 8765 编辑器及旧预算均不是当前产品规格。历史链接和测试记录保留供追溯。

Avatar + OSC 渐进造型项目。2026-10-18 已打通外部OSC→整数参数→Avatar FX→原生Mesh粒子显示，包括单位体素、长方体、擦除、替换和清空。旧版有用户现场客户端观察，当前新版以Unity Play Mode + Gesture Manager验证；AI后端和共享运动尚未实现。

**最新版本：[Stage3 定位球](../stage3/README.md) + [本机网页编辑器](../editor/README.md)**。2026-10-19 新增金色底部中心定位球、默认世界固定／Avatar跟随、左右手接近握拳抓放、0.05–1.0缩放，以及浏览器放置／擦除／长方体／撤销重做／OSC发送。完整Avatar为 `Assets/MorphForge/Stage3/Avatar.prefab`，场景为 `MorphForge_Stage3.unity`，合并后196/256 bit。Windows浏览器入口 `http://127.0.0.1:8765`，默认停发；上传新版后可连接本机9000。真实VR硬件和多人定位仍待确认。

前一版：[Stage2/MF2](../stage2/README.md)。已改为单次网络提交，增加一个本地待执行槽，修复Avatar旋转/平移时的纹理游动。主回归1404项断言、速率/姿态扩展186项断言及队列隔离、NDMF检查通过。0.25s/0.2s均按一条完整命令测试，详细范围见[验证状态](../VALIDATION.md)。新版场景为`MorphForge_Stage2.unity`，Avatar为`Assets/MorphForge/Stage2/Avatar.prefab`；3层、40个Clip、同步新增88bit。

Stage1已由用户在真实VRChat确认本机和其他玩家能显示方块，0.25s双阶段循环观察正常；MF2是新的接收协议和Shader，需要更新上传Avatar后生效。以下Stage1数字保留为历史基线。

项目入口：[统一设计](../DESIGN.md)、[验证状态与未完成项](../VALIDATION.md)、[新版操作与证据](../stage2/README.md)、[纹理修复对照](../stage2/texture-comparison.png)。历史[第一阶段操作与证据](../stage1/README.md)：主回归1371项断言、外部自动拆分→OSC集成14项断言、几何编译器5项单测通过。

Stage1测试场景为`Assets/MorphForge/Scenes/MorphForge_Stage1.unity`，固定2层Animator、39个Clip；新版Stage2为3层、40个Clip。一颗粒子保存一个盒体，当前容量64盒；11个同步Int新增88bit，NDMF合并后Avatar总177bit、剩79bit。最小外部编译器支持盒体覆盖和局部挖洞，完整Draft/Published与可配置共享通道仍在设计阶段。

Pipeline 已实际连通，基础场景已保存；Humanoid / Avatar Descriptor 有效，Missing Script、缺失依赖和材质 Shader 问题均为 0。见 [配置验收](../setup/README.md) 与 [验证报告](../setup/validation.json)。

历史结果：[第 1 / 4 项原型](../probe14/README.md)。39 项检查通过；外部输入产生四面体、屋顶和随机三角形，三组 GPU 图与 CPU 参考图逐像素一致，载体 CPU 顶点与资产哈希保持不变。范围为 Play Mode + Gesture Manager 原生 OSC。

[OSC / 多人同步能力估算](../channel-capacity/README.md)：当时 Avatar 原有功能占 89 bit，未加新通道时剩余 167 bit；按 0.2 秒有效批次间隔，分析共享通道、原始网格、参数化建模和动画的边界。数值是规划估算，不是多人吞吐实测。

另有 [固定体素方案分析](../voxel-capacity/README.md)：比较单位立方体、共享样式、稀疏槽位和区域/合并表面传输，区分可寻址范围与实际可显示容量。

新增 [快速仿真、草稿发布与独占OSC调度设计](../simulation-publish/README.md)：两套场景空间、异步快照发布、单写入发送、队列合并与持续刷新，附配置和模型工具指令草案；尚未实现。

## 工程位置与访问

- Windows：`D:\Projects\VRChatProjects2\MorphForge`
- 主机：`192.168.101.5`
- Unity：`2022.3.22f1`
- Linux 工具目录：`/home/chiro/projects/vrc-projects/morphforge`

在本目录执行：

```bash
python3 tools/unity_remote.py editor_status
python3 tools/unity_remote.py set_autotick --enable true
python3 tools/deploy_sources.py
python3 tools/unity_remote.py run_script --file AgentScripts/SetupMorphForge.cs --entry SetupMorphForge.Run
```

工具固定选择 MorphForge，允许多个 Unity Editor 同时运行。需要启动 Editor 时：

```bash
python3 tools/remote.py --file tools/start_editor.ps1
```

启动器使用当前登录桌面的按需任务 `MorphForge-UnityEditor`，没有定时触发器；不会关闭或重启 Roadster。可选的 SSH 控制连接环境变量为 `MORPHFORGE_SSH_CONTROL`。

## 依赖来源

复用 `D:\Projects\VRChatProjects2\TeslaRoadster` 中的现有安装，保留资源 GUID 和 Pipeline 的 Unity 2022 适配：

| 依赖 | 版本 |
| --- | --- |
| VRChat Base / Avatars | 3.10.5，来自新工程原有 SDK |
| Unity Pipeline | 0.8.0-exp.1，Roadster 的 Unity 2022 本地适配 |
| lilToon | 2.3.4，含 Roadster 已有兼容修订 |
| Modular Avatar | 1.18.7 |
| NDMF | 1.14.8 |
| Supine | 5.1.0 |
| Gesture Manager | 3.9.9 |

Nemesis 资源从 Roadster 的 `Assets/Nemesis` 导入；基础资源为 `Assets/Nemesis/Nemesis.prefab`。只读查询过 Nemesis Prefab 集的实际包依赖，另安装 Gesture Manager 供后续 Editor 验证。

`tools/bootstrap.ps1` 是一次性导入脚本，发现已存在的目标会停止，避免重复覆盖。它保留原有 SDK，备份 VPM 清单，并记录源 Prefab 的 SHA-256。后续不要通过重新运行它来更新包。

## 基础检查

`SetupMorphForge.Run` 检查基础 Nemesis 的 Humanoid、Avatar Descriptor、Missing Script、材质 Shader 和依赖引用；通过后创建独立基础场景与预览。

- 基础场景：`Assets/MorphForge/Scenes/MorphForge_Nemesis.unity`
- Windows 导入记录：`Docs/Setup/bootstrap.json`
- Windows Unity 验证：`Docs/Setup/validation.json`
- Windows 预览图：`Docs/Setup/morphforge-nemesis.png`

基础场景使用 Prefab 实例，不修改源 Nemesis Prefab；新实例不继承上传 ID。脚本在 Play Mode 或编译期间停止；需要切换/创建场景时会保护已有未保存编辑，验证已加载场景则不要求重载。

实际检查结果以验证报告为准；这些准备不代表已经实现 OSC 几何、AI 建模或多人同步。

## 后续设计

既有 [项目研究](../../../vrc-ai-world/README.md)、[Avatar 路线](../../../vrc-ai-world/docs/06-avatar-osc.md) 和 [KipSeek 复用分析](../../../vrc-ai-world/docs/07-kipseek-reuse.md) 保留在相邻目录。

Unity 操作遵循 [UNITY-GUIDE.md](../../UNITY-GUIDE.md) 和 [Unity Pipeline 技能](../../.agents/skills/unity-pipeline/SKILL.md)。Nemesis 商业模型和完整包保留在用户 Windows 工程，不纳入公开发布。
