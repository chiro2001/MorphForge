# MorphForge 协作说明

- 使用简体中文回复用户。
- Unity 工程位于 `192.168.101.5` 的 `D:\Projects\VRChatProjects2\MorphForge`；本目录保存访问工具、源脚本与验证记录。
- 操作前阅读 [UNITY-GUIDE.md](UNITY-GUIDE.md)，使用 [.agents/skills/unity-pipeline/SKILL.md](.agents/skills/unity-pipeline/SKILL.md)。
- `python3 tools/unity_remote.py <命令>` 始终显式选择 MorphForge；不要省略项目路径去操作其他 Editor。
- 临时文件使用 Linux `~/tmp/morphforge/` 和 Windows 用户目录 `tmp\morphforge\`。
- Pipeline 复用 Roadster 的 Unity 2022 适配包，先阅读其 `Documentation~/unity2022-compatibility.md`，不能用未适配的注册表包直接覆盖。
- 创建或修改 Unity 序列化资源通过 Pipeline；批量脚本放 `AgentScripts/`，用 `run_script` 调用。
- Nemesis 使用用户已有资源，原模型与商业资产留在 Windows 工程，不公开发布。
- 当前范围是 Avatar + OSC 几何显示方案；研究文档见相邻 `../vrc-ai-world/`。2026-10-22本轮已授权实现语音识别，范围以docs/text-animation/REQUIREMENTS.md为准。
- 不自动上传 Avatar、启动真实 VRChat 客户端或改动其他项目场景。

## 本轮已授权范围

- 2026-10-25 用户要求参考 TestlaRoadster 准备 BOOTH 发布包。已使用原始 Desktop0.27.0 / Avatar1.4.0 文件整理三语指南、三语商品文案、7张实际编辑器素材排版图、独立许可与可重建发布脚本。买家ZIP约36.9MiB，整套PublisherKit约75.4MiB；41项包装与5项离线Chromium检查通过，Windows复制后55文件逐项核对。目录为Releases/MorphForge-0.27.0，结果见docs/releases/0.27.0/。额外隔离Unity导入因授权连接/启动阶段未就绪而未执行，不能当作通过；保留原30项开发工程Unity/SDK记录，临时Editor/任务已清理。售价0円仅草案，最终商品条款、联系方式和人工实机复验待确认。未上传BOOTH、未更新在线Avatar、未推送或创建GitHub Release。

- 2026-10-25 用户要求 Desktop 与 Unity 安装插件支持简体中文、日文、英文。Desktop 界面默认跟随系统，可即时切换并保存在独立 ui-settings.json，与提示词、识别及字幕翻译语言独立；主窗口/Mini 同步。Unity 所选语言用于插件界面与 Avatar 菜单，已有模块可直接更新菜单；每个安装实例使用独立菜单资产，不修改公共模板或其他 Avatar。Desktop 0.27.0 / Avatar 插件 1.4.0；320 项 Rust、67 项前端、24 项 Chromium、19 项 Windows 原生、3 项原生重启和 30 项 Unity/SDK 验证通过。主安装已升级启动，14 项安装与 8 项主程序检查通过，14,025 个数据文件安装前后摘要一致；备份在 E:\tmp\morphforge\main-backup-v0270-localization-final。见 docs/localization-20261025/RESULTS.md。本轮未上传头像、不推送或创建 Release。

- 2026-10-24用户要求将本人预览放入MorphForge子菜单、修正正面取景，并自主进行性能消融；允许参考VRCX/头像选择API切换当前头像及读取实机帧率。Avatar1.3.2已更新同一私人头像v13/file11，三项服务端分析通过；SDK152层、1926参数、254bit。本人预览位于MorphForge子菜单、默认+Z正视；普通空闲状态使用稀疏Clip，完整Direct输出保留WD On，材质绑定3132→1656。交互式Editor空载6.54→5.46ms、三组动画11.28→9.04ms；492值/24位置和106次连续GPU对比通过，文字15项、点线、动画、本人预览及公共安装验证通过。通过OSC/avatar/change实机换装并恢复原作品/flow-loop；未重启串流。实际FPS对照仍待确认可读的性能显示；SteamVR新客户端握手失败，用户尚未授权重启VRChat。见docs/performance-preview-20261024/RESULTS.md。Desktop保持0.26.1，未推送/建Release。

- 2026-10-24Desktop0.26.1已升级主安装：用户要求唯一动画默认直接选择；载入/重启自动选中并随主发布，模型revision16/API保留，发送关闭。相关Rust9/前端62/Chromium16/Windows原生16/安装14/主程序8项通过。头像缓存消除120个WD Off直接接收状态及234个双层写入字，参数与同步预算不增加；同一私人Avatar上传v12/file10，三项分析通过，接收能力9，公共包1.3.1。SDK后492值/24位置及三轮102次GPU比较通过，实机视觉待用户重载v9头像复验。见`docs/default-motion/RESULTS.md`、`docs/motion-cache-20261024/RESULTS.md`。未推送/建Release。

- 2026-10-24Desktop0.26.0已交付主安装：主界面发布模型与所选动画，两个片段选择框同步，未保存/不支持/版本冲突明确失败。317项Rust、62项前端、Chromium13项、Windows原生13项、安装14项及主程序8项通过。等待用户建模及发送完成后正常升级，当前新项目revision16与API连接保留，恢复flow-loop选择；发送默认关闭。结果`docs/model-motion-publish/RESULTS.md`，未改Unity或推送/发Release。

- 2026-10-24用户确认主界面“发布当前版本”需要同时发布所选动画。实现主界面片段选择、模型与动画一次校验发布及明确排队提示；允许按既定约定验证、提交及升级主安装。实机风车动画已由用户确认转动，之前无动画是仅执行了静态模型发布。

- 2026-10-23双语字幕已交付Desktop0.25.0主安装：同框原文+译文，新建默认四行128×40/字号8，各保留最近最多两行；等待翻译时原文继续刷新，补播使用最终双语。保留用户已开启的翻译设置；当前项目旧字幕框通过事务34→35改为四行固定字号8，文字、位置、尺寸及模型几何保留。316项Rust、62项前端、16项浏览器、17项Windows原生通过；详情`docs/bilingual-captions/RESULTS.md`。

- 2026-10-23用户调整字幕翻译：头像字幕必须同时保留原文和译文；默认实时字幕改为四行，分配原文/译文各两行，保留最近内容并保持字号，原文在等待译文时继续实时更新。允许按既定约定完成构建、验证、提交和主安装升级。

- 2026-10-23实时字幕自动翻译已交付Desktop0.24.0主安装：中/日/英，复用大模型连接或指定独立模型，默认关闭；原项目revision51及API连接保留。160项Rust、62项前端、13项浏览器、14项Windows原生验证通过，全部用本地模拟LLM。Avatar继续1.3.0/v11，详情`docs/subtitle-translation/RESULTS.md`。未推送或建Release。

- 2026-10-23用户追加实时字幕自动翻译：大模型API调用，目标语言中文/日文/英文；复用现有连接，默认关闭，保留原文。允许正常版本递增、验证、提交及按既定约定升级主安装；不调用付费模型作自动验收、不输出凭据、不推送或建Release。

- 2026-10-23本轮优化已交付：Desktop0.23.0主安装、公共Avatar1.3.0，同一私人Avatar v11/file9三项服务端分析通过。主项目异常字幕通过事务50→51，腾讯凭据已配置（未开启云识别），实机待用户复验。证据与复验步骤见`docs/optimization-20261023/RESULTS.md`。未推送或建Release。

- 2026-10-23：用户明确要求开始本轮全部验收问题优化并创建持续目标，直到完成。包括头像空载性能、文字空白闪烁、原生点线、Avatar连续动画、字幕/文本预设与位置、多行字幕、混合显示、提交判重与补播、编辑器几何闪烁、语音建模触发、腾讯云凭据迁移。允许更新现有在线头像验证服务端接受；此前等待“开始修复”的限制已解除。执行清单见 `docs/optimization-20261023/PLAN.md`；不输出凭据，不自动启动VRChat，不创建Release。

- 2026-10-22：用户报告 MF6 Avatar 在线服务端检查失败。已在同一私有 Avatar ID 上进行单项对照，确认内部参数规模触发问题；完整修复版已上传为 Avatar v10 / file v8，generic/standard/security 均200且success=true。FX内部参数2794→1926，网络254/256bit及240bit几何直同步不变；4×128槽文字、3组动画、预览与16K几何保留。公共Avatar包1.2.1，Desktop仍0.22.0。源场景恢复干净，原人物Prefab依赖未变，未启动VRChat或推送远端；证据见`docs/mf6-security-20261022/RESULTS.md`。

- 2026-10-22：用户明确要求“构建并上传当前 Unity 头像”。已通过公共VRChat SDK更新当前MF6场景绑定的既有私有Avatar，线上版本5→6，ID、名称和可见性保持一致，pendingUpload=false。同步预算254/256bit，几何240bit保持直接同步。上传入口`AgentScripts/AvatarUpload20261022.cs`，验证见`docs/avatar-upload-20261022/RESULTS.md`；Unity已恢复干净源场景，原Prefab依赖哈希未变，SDK临时状态另存Windows验证快照。未启动真实VRChat客户端，未创建GitHub Release。

- 2026-10-22：用户新增快速演示显示需求，已实现点/点集、可调粗细线段/折线、箭头、圆环、坐标轴与线框盒。复用普通三角面和现有Avatar 1.2.0协议；Desktop 0.22.0已按默认主安装约定备份、安装并启动，数据校验一致。备份在`E:\tmp\morphforge\main-backup-v022-drawing`，验证见`docs/drawing-primitives/RESULTS.md`。本轮未发Release、未上传Avatar。点线演示源码与交付记录纳入本轮Git提交。

- 2026-10-22：用户要求创建持续目标并完整实现文字/语音/动画/工作台mini与悬浮窗/定位/本人预览等本轮需求；**暂不实现SteamVR HUD，不自动创建GitHub Release**。在隔离副本开展Windows Cable和GUI验证，保护用户正在运行的任务；版本可递增。

- 2026-10-21用户选择并要求开始实现发布优化、Avatar能力识别、连续区域编码、多实例共享库、旧协议清理，以及闪烁修复更新到真实Avatar；包含本项目Avatar的SDK构建上传。执行ACK明确排除。原版Minecraft资源仅在gamepc私人路径使用，不放公共仓库。

## gamepc 软件更新约定

- 2026-10-21本轮用户明确要求将0.16.0正式安装包替换gamepc主程序并发布GitHub Release；允许在检查任务状态后正常关闭主窗口、备份并安装。测试副本和私人MC资源继续保留。C盘余量有限时，使用`install_main.ps1 -BackupRoot`将完整备份放E盘任务目录。
- 远程默认使用`gpu5080` SSH别名以继承身份和连接复用配置；可用`MORPHFORGE_SSH_HOST`或`--host`覆盖。

- 用户于2026-10-21本轮明确要求“现在替换主程序版本”：已将验收通过的0.15.3通过正式NSIS安装包升级至主安装并启动，更新前完整备份。独立开发副本继续保留。下条副本隔离约定描述此前验收阶段，不阻止本轮已授权的主程序替换。
- NSIS打包可能修改桌面shell的安装包类型标记；安装校验的shell哈希应取自独立解包的NSIS有效载荷，不能直接使用portable版或打包后已恢复的target文件。

- 用户于2026-10-21要求：当前使用主程序期间，更新和测试使用额外副本及隔离数据；本次不关闭、覆盖或切换用户正在使用的主安装，不复制其活动任务到测试环境。
- 用户于2026-10-20明确改为：后续默认更新**主安装** `%LOCALAPPDATA%\MorphForge Desktop`，原数据 `%LOCALAPPDATA%\MorphForge` 保留。更新前备份程序和数据，使用正式Windows安装包，并验证实际安装版本及桌面入口。
- 正在运行时先检查是否有AI任务和未完成操作；不覆盖运行中的核心可执行文件（Recipe依赖current_exe）。可在授权的更新范围内正常关闭程序后安装，避免强制结束任务。
- 原独立开发测试副本 `%LOCALAPPDATA%\MorphForge Preview` 与其Data/Profile继续保留；不自动复制或合并主安装与开发副本的数据。
- 自动GUI验收使用临时隔离数据/浏览器配置，先校验项目ID、版本和内嵌源码哈希，测试窗口标记为自动验收，不能假设用户没有操作过已有窗口。
- 仅在用户明确要求发布时创建 GitHub Release。
