# MorphForge / 造形工坊

独立体素艺术工作台：手工编辑、兼容 API 的 AI 创作、作品库与多视角预览共用一个前端，Rust 核心可独立运行在 Windows。VRChat Avatar 是可选发布目标，不是创作和保存作品的前置条件。

- [用户指南](docs/USER-GUIDE.md)：安装、配置、编辑、AI、compact、作品库与 OSC 发布。
- [交付记录](docs/DELIVERY.md) · [实施状态及证据](docs/plan/STATUS.md) · [已确认设计](docs/plan/README.md) · [验收标准](docs/plan/ACCEPTANCE.md)。
- Windows **0.4.2 候选版**：`dist/windows/MorphForge Desktop_0.4.2_x64-setup.exe`；校验和在同目录。[前缀缓存修复验证](docs/quality-v4.2/RESULTS.md) · [参考图建模迭代](docs/quality-v4.1/RESULTS.md) · [0.2 历史验收](docs/quality-v2/RESULTS.md)。

GitHub私有仓库：`chiro2001/MorphForge`，默认分支`main`，开发分支与完整历史保留。
[获取源码和Windows克隆验证](docs/GIT-SOURCE.md)。

## 在 VRChat 中的预期体验

把 Avatar 变成一个随身的体素创作台：在电脑端输入文字、提供参考图或手工编辑，通过 OSC 把作品逐步呈现在身边。目标是让同房间的朋友一起看着建筑、雕像和文字搭起来，再根据聊天中的新想法继续修改。体素内容由 Avatar 显示，作品和创作上下文长期保存在外部工作台。

| 功能与玩法 | 怎么操作 | 房间内的预期效果 |
|---|---|---|
| AI 现场搭建 | 输入“搭一座有拱门和发光招牌的小屋”，按阶段发布 | 墙体、屋顶和装饰逐渐出现；AI 可在后台继续设计 |
| 看图做摆件 | 选择图库或上传参考图，生成角色、建筑或道具 | 把参考图的轮廓、配色和识别点近似为立体体素作品 |
| 朋友出题、持续改稿 | 接着输入“加条鲸尾”“把屋顶换成蓝色”，或中途插话 | 在已有作品上增删、擦除和换色，保留连续上下文 |
| 手工积木与沙盘 | 在共享编辑器中绘制长方体、挖洞、选择分组和调整位置 | 演示搭桥、修楼、挖门窗，也可手工修正 AI 作品 |
| 随身展示与房间摆放 | 用底部中心的金色定位球定位，切换世界固定/跟随 Avatar、缩放及保持水平 | 做桌面小摆件、身边的展示牌，或放大成场景沙盘；手势抓放用于搬动整体作品 |
| 立体文字 | 选择字体，把短句、名字或标语生成体素 | 展示由真实方块组成的招牌、祝福语和纪念牌 |
| 材质与作品切换 | 使用已打包的材质目录，加载作品库中的不同作品并发布 | 展示木石、金属、发光、云水表面或 Minecraft 风格；重复调用已有作品无需重新生成 |

例如，可以让朋友轮流出题做一个“房间吉祥物”，边看生成边提修改意见；也可以先准备一组建筑和雕像，现场切换成小展览。手工编辑与 AI 共用同一份作品，适合先让 AI 搭主体，再由操作者补细节。

一轮演示的操作顺序：

1. 切换到包含 MorphForge 模块的 Avatar，启用 VRChat OSC；工作台选择 `VRChat 9000`。
2. 在工作台选择参考图并输入需求，先查看草稿；开启发送后，手动发布当前版本，或启用“编辑后自动发布”。
3. 观察作品按发送进度搭建，继续输入需求或手工修改；定位球控制整体摆放、跟随模式和大小。
4. 保存到作品库，按需再次发布；需要恢复显示时使用“清空并重建”，也可配置周期全量刷新。

### 显示效果与当前边界

- **空间与精度**：VRC 适配器使用 `256³` 逻辑空间，当前容量目标为 **16,384 Box**。Box 是长方体，一整片连续墙体可以合并成一个 Box；容量消耗取决于几何碎片和外观分区。默认格长 `0.25m`，整体显示支持 `0.05～1.0` 缩放，便于从沙盘缩成小摆件。
- **搭建节奏**：AI 在本机快速编辑草稿，网络队列按节拍发送已发布修改，两者可以并行。大作品首次搭建需要等待；优先用合并长方体减少命令量，现场演示适合先搭大形、再补细节。
- **多人观看与恢复**：设计目标是由创作者运行工作台，其他玩家通过 Avatar 参数同步观看。已实现手动重建和周期刷新；全量刷新会先清空再重建，晚加入者需要等待一次完整发送。OSC 没有远端 ACK，发送完成不能证明所有玩家都已显示完整。
- **外观与动画**：已有六面贴图、材质帧动画、金属/发光及云水表面表现。新 RGB 色板或资源包需先导出并更新 Avatar 材质目录，才能在 VRC 中对应显示。逐 Box 实体运动、骨骼动画和复杂物理属于后续扩展。
- **适配与验收**：当前面向 PC/PCVR、Very Poor 性能等级。几何、定位约束、容量和材质已有 Unity 原生验证，最终 FullFT 16K 包已通过服务端检查；该最终版本的真实客户端加载、多人同步和 VR 硬件抓放仍待现场验收。上表描述目标体验，实测范围以[验收记录](docs/plan/ACCEPTANCE.md)为准。

## 工作台与已有作品

![MorphForge 共享工作台](docs/media/workbench.webp)

连接与运行配置集中在独立页面：

![MorphForge 配置页](docs/media/settings.webp)

| 云上邮局 | 星港灯塔 |
|---|---|
| ![云上邮局体素作品](docs/media/cloud-post.webp) | ![星港灯塔及体素汉字](docs/media/starlight-harbor.webp) |

工作台截图为新版界面载入已有星港作品；下方为此前真实 DeepSeek 创作的渲染，包含由字体栅格化的可编辑体素汉字。作品过程、失败与修正见 [Harness 迭代记录](docs/harness-v2/evals/README.md)。展示图为压缩 WebP 副本，原始验收 PNG 保留。

## 图片参考

支持从内置图库或本次添加的图片生成体素，可分别关闭或组合使用，最多6张。内置5张项目自制结构参考与3张核实出处的鲸鱼娘参考，附作者、原帖和独立许可；个人图片留在本机图库，发送选中参考时才交给模型。续作和compact保留参考，取消后下一次输入更新，Harness作品记录来源。

![内置参考图库](docs/media/reference-library.webp)

[用法、素材出处与许可](docs/references/README.md)。复杂角色的比例、遮挡与配色仍需人工复核，参考图片不等于自动重建保证。

已支持精确RGB色板，缺少肤色、藏蓝等颜色时由模型创建并保存。0.4.2修复前缀缓存：普通请求只追加内容，system保持稳定，旧截图清理移到compact边界。以下为此前真实参考图生成、预算耗尽后接续保存的223 Box作品，配色与正面识别点已有改善，比例和裙摆仍有简化。

![参考图与RGB色板生成的鲸鱼娘](docs/media/whale-reference-palette.webp)

非官方体素二创；参考图作者ZipZipPipe、原角色上善无形，遵循CC BY-NC-SA 4.0。[原帖与署名](references/ATTRIBUTION.md) · [可直接导入的作品JSON](docs/quality-v4.1/whale-palette-finish/artwork.json) · [完整评测、失败与接续记录](docs/quality-v4.1/RESULTS.md)。

## 创作速度与精度

AI创作支持 **快速草图／标准创作／精细作品** 三档 preset，分别偏向轮廓速度、结构与细节平衡、多阶段细化。预设同时设置中英提示词和默认预算，仍可手动调整。精细档默认启用有界 Goal 续作，要求不同几何的真实截图复核；支持插话、打断、暂停和 compact。耗时和质量仍取决于任务及所选模型。

连接、预算和Goal等设置集中在独立配置页；输入框旁显示API累计Token、本轮Token和加权Prefix缓存命中率，上游未报告时明确标记。

API连接可自定义 **Chat Completions 或 Responses** 服务、Base URL、模型名和 Key。非DeepSeek官方接口强制内嵌截图，无需图片上传或 file_id 接口；文本模型可关闭图片。连接与作品库独立于 VRC，可作为体素生成器、浏览器和编辑器使用。配置方法、三档预算与验证范围见 [使用说明](docs/PROVIDERS-PRESETS-GOALS.md)。

## 当前实现

共享工作台支持稀疏 Box 几何、精确切割、分组变换、撤销重做、版本快照和资源包材质。Harness 提供受限 Recipe、多视角截图、工具调用循环、插话、打断续接、排队、暂停恢复、详细日志和上下文 compact；compact 保留原始日志、图片、Recipe 与作品版本。提示词与工具说明优先中文，并提供英文版本。字体文字栅格化为普通可编辑 Box，支持预览与撤销；保护分组可锁住既有作品。真实 DeepSeek 多轮创作、交互、compact 和收尾入库已验证，失败与接续修正记录一并保留。

独立作品库已有铜灯塔及其续写版本，以及旧完整场景的精确转换：70,286 个实际占用体素无损合并为 7,459 Box。Minecraft Java 资源包支持常见整立方体、六面与动画纹理；当前云、水、金属等使用体素表面材质表现。

VRC 主档为 **MF3：56 bit 几何通道 + 14 bit 定位**。完整面捕固定预留 **161 bit**，其余本体功能和压缩承载预算 **25 bit**。MA 注入几何和定位模块，VRCFury 负责完整面捕及低频参数压缩；实际SDK联合构建已通过256/256bit，FT161与MF70全部直通；通过精简本体同步实现，无压缩carrier。最终 prefab、上传场景及生成入口均已排除睡眠插件依赖。已通过补发到达 16,384 个唯一 Box 及满池回收验证。显示池持久保存每个 Box 的位置、尺寸和外观编码，接收动画器保持固定规模，不为每个 Box 创建一组状态；真实 UDP 首遍缺失、最终材质及 FullFT 联合构建结果见实施状态。

默认回归只向9012发送，不向9000发包或启动VRChat。2026-10-19经用户明确授权，已上传并核验私有 **MorphForge FullFT 16K**：实际包总256bit、完整FT161、Store16384，服务端三项分析均200。修复了Catalog metadata布局、FullFT场景根名和人物特效Pickup碰撞力；[最终证据](docs/security-check-failure/FINAL-FULLFT.md)与[客户端复核步骤](docs/security-check-failure/CLIENT-RETEST.md)。

网络发布没有远端ACK；本机发送进度不能证明其他玩家已经显示。真实客户端加载、多人及VR手势仍待现场确认。源码候选版本`v0.2.0-rc.2`包含本轮修复，最终`v0.2.0`标签等待客户端确认。

## 开发入口

```bash
./tools/cargo_local.sh test --workspace
./tools/cargo_local.sh run -p morphforge-app -- serve --data /tmp/morphforge-dev --port 8766
```

Windows 打包入口为 `desktop/`，见 [桌面说明](desktop/README.md)。Unity 资源通过 [Unity 操作规范](UNITY-GUIDE.md) 和 Pipeline 构建，禁止手改序列化资产。

## Unity 工程

- Windows：`D:\Projects\VRChatProjects2\MorphForge`
- Unity：2022.3.22f1；通过 `tools/unity_remote.py` 显式选择工程。
- FullFT：`Assets/MorphForge/FullFT/Avatar.prefab` 与 `Assets/MorphForge/FullFT/MorphForge_FullFT.unity`；实际预算与API烟测见 [FullFT验收](docs/fullft/README.md)。
- Nemesis 商业资产和生成的完整 Avatar 只保留在用户 Windows 工程中。

## 历史原型

[Stage1–3 历史入口与环境配置](docs/history/README-stage1-3.md) 保留原型、依赖和旧验证链接。旧的 64 Box、MF2/88 bit、Python 8765 编辑器与基础 Avatar 预算均不是当前产品规格。
