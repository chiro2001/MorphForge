> 2026-10-21 正式发布：gamepc主安装已升级 **0.16.0**，原数据完整备份并校验保留，测试副本继续独立运行。GitHub正式Release为`v0.16.0`。[安装与SSH复查记录](release-v16/RESULTS.md)。下文保留此前交付记录。

> 2026-10-21 当前交付：开发测试副本 **0.16.0**，私人 **MorphForge MF6 16K v5**；三项服务端分析通过。自动发布成本比较、区域编码、能力/材质识别、多实例、原版MC材质、输入区类型/尺寸、进度条、作品删除、视角/树叶/抓握修复已实现。主安装0.15.3保持运行，MF2–MF5发送已停用。MC导入不再默认4格，现自动选择形状精度并检查目标空间。[最新用法、开销与实测](stream-v16/RESULTS.md)。下文为历史版本记录。

> 2026-10-20 当前桌面主安装：**0.15.0**。参考图对照、同对话可复用组件、分组动画/姿态截图/动画GLB，以及风车示例已实现并安装。当前MF6头像仍显示静态几何。GUI/Windows/Harness验证见[本轮结果](creative-v15/RESULTS.md)；下文保留历史交付。

> 2026-10-20 当前阶段：**0.13 / MF6 16K v3** 已交付。0.11、0.12、0.13均已分阶段安装Preview；统一240bit、纯色三角面／单对象斜面、Minecraft建筑与状态材质已实现，头像三项分析通过。完整设计与实测以[MF6验证记录](mf6/RESULTS.md)为准。下文为保留的历史交付。

> 2026-10-20：开发副本0.10.0增加参考图像素/缩放/体素平面工具、连续循环补播与更新顺序；继续兼容MF5 v15。当前Unity仅加载一个Avatar。碰撞、三角面和4×60通道是研究候选，尚未部署。[本轮结果](image-stream/RESULTS.md)。

> 2026-10-20：当前开发交付为 **0.9.0 / MorphForge MF5 16K v15**，旧5%重定义为新100%、1%～1000%微缩、实时材质与48种原创内置纹理、双池配对发送和补播。16K第二轮首遍完整通过，首轮损耗仍保留；服务端三项分析通过，开发副本已安装，未发Release。[最新结果](mf5/RESULTS.md)。

> 2026-10-20：当前无面捕开发交付已升级为 **MF4 0.8 / MorphForge MF4 16K v13**，254 bit（三路几何240＋定位14），安全分析三项通过。软件更新开发副本，旧验收安装保留。见 [本轮结果](mf4/RESULTS.md) 与 [协议设计](mf4/DESIGN.md)。下文保留此前 FullFT 交付记录。

# MorphForge 交付记录

开发测试副本已部署 **0.7.0**，包含画质/材质/定位与透明表面完善，仅更新副本，原验收版保持运行；未新增 Release。[本轮验收](quality-v7/RESULTS.md)。下方0.6.0为之前公开候选版记录。

当前软件候选为 **0.6.0**：创作时间轴、常驻缓存/前缀保护、严格输入预算预留、执行器崩溃暂停；自由/局部/正交截图、绘制+截图、持久源码/patch与曲面助手。1M/384K、6份离线作品及既有参考图库保留。安装器 `dist/windows/MorphForge Desktop_0.6.0_x64-setup.exe`，校验和在同目录。[本轮真实失败、修复和验收](quality-v6/RESULTS.md)、[用法](HARNESS-QUALITY-TOOLS.md)。尚未完成与dsh的同条件美术对照；不把工程通过当作质量追平。旧交付证据保留在后文。

## 0.2.0 历史交付

日期：2026-10-19。Rust独立核心、共享体素工作台、DeepSeek Harness、可选MF3发布适配器及Windows Tauri安装器已交付。GitHub私有仓库为`chiro2001/MorphForge`，默认`main`，保留开发分支`feat/harness-interaction-and-voxel-text`，基线tag为 `v0.1.0-baseline`，源码候选tag为 `v0.2.0-rc.2`，最终 `v0.2.0` 等待客户端现场确认；代码与历史已同步GitHub私有仓库。

## 安装与使用

Windows x64安装器：

```text
D:\Projects\VRChatProjects2\MorphForge\dist\windows\MorphForge Desktop_0.2.0_x64-setup.exe
```

大小13,613,538字节，SHA-256：

```text
CE30B61609FF93C250840560D14AB4278709E7BBD37EACDA63F736B4FCCBEF03
```

Linux工作区有相同副本 `dist/windows/`，该目录保留安装器校验清单；完整Git历史bundle为 `dist/windows/MorphForge-0.2.0-rc.2.bundle`，附独立SHA-256；包含本轮Avatar修复及实际上传证据。安装器未代码签名。

用户数据位于 `%LOCALAPPDATA%\MorphForge`。现有DeepSeek配置保留，API Key不进入源码、安装器或Git bundle。新增两份带三视图的样例作品与独立原始运行档案，未覆盖当前工程及原有三份作品。

- [用户指南](USER-GUIDE.md)：安装、编辑、文字、AI交互、compact、作品库与发布。
- [本轮质量结果](quality-v2/RESULTS.md)：实现、测试、失败记录及使用边界。
- [Windows 0.2验收](quality-v2/WINDOWS-V2.md)：原生、安装、实际嵌入资源与清理。
- [实施状态](plan/STATUS.md) · [设计方案](plan/README.md) · [0.1.0历史交付](history/DELIVERY-0.1.0.md)。

## 本版能力

工作台可独立于Unity/VRChat运行，统一手工编辑、模型生成、作品浏览、材质、快照和撤销重做。Rust核心可独立运行在Windows；Tauri只负责桌面窗口与进程生命周期。

Harness支持安全点插话、立即打断后同run续接、排队下一轮、暂停/恢复/停止，以及保留原始日志与图片的compact。取消、工具调用配对、幂等与过期提交都有守卫。提示词和工具说明提供中英文，优先中文；版本迭代到2.3。`finish_artwork`完成提交、当前版本截图关联、入库和完成摘要。

字体文字由内置Noto Sans SC或只读系统字体生成普通体素Box，支持三平面、排版、挤出、预览、应用和撤销。应用回执跨重启持久化。用户可保护分组，几何修改、历史恢复及作品载入都不能隐式破坏受保护内容；此能力不增加Avatar同步参数。

## 验证与模型表现

Linux核心74项、Clippy与格式检查通过，Node24项、浏览器mock31项通过。真实字体预览24项、Harness UI17项、保护25项、手工5k/16k回归14项通过；16,384个实例为7次绘制调用。Windows原生核心74项、桌面2项通过；本版真实安装、窗口启动、实际嵌入资源哈希、正常退出与卸载均已验证。69个冻结构建输入与最终产物对应。

真实DeepSeek评测保留R0至R3c的全部失败与修正。R2c完成766 Box作品收尾、R3c完成791 Box作品的正面体素中文修正与三视图入库；旧组全部保留。[原始评测](harness-v2/evals/README.md)含输入、事件、Token用量、场景和图片。复杂作品仍可能发生空间判断错误或预算耗尽，本次证明的是可观察、可打断、可修正并完成入库的连续创作闭环。

## Avatar、预算与既有限制

最终资源入口：

```text
Assets/MorphForge/FullFT/Avatar.prefab
Assets/MorphForge/FullFT/MorphForge_FullFT.unity
```

实际SDK联合构建为 **256 / 256 bit**：

| 项目 | bit |
|---|---:|
| 完整面捕112项（105 Bool、7 Float） | 161 |
| MF3几何（6数据Int + Ctl） | 56 |
| 定位、缩放、水平与抓放 | 14 |
| 必要面捕控制Bool | 8 |
| 其余本体 | 17 |

MA注入几何与定位，VRCFury集成完整面捕，通过精简本体同步实现，无压缩carrier。最终prefab、上传场景与生成入口均已排除睡眠插件依赖；[资源审计](avatars/README.md)给出递归依赖与哈希证据。Nemesis商业资产只留在Windows工程。

此前16,384 Box真实UDP首遍缺192个，0.5秒补发后达到16,384个唯一Box；满池回收通过。最终FullFT协议166项、材质像素40项证据见[实施状态](plan/STATUS.md)。无远端ACK，不能将本机发送完成等同其他玩家显示完成；本轮没有把软件测试当作真实多人验证。

当前为PC/PCVR、Very Poor定位。云水是表面近似，Minecraft仅支持常见Java立方体、六面和动画纹理；高级折射、物理体积云、任意异形模型及实体运动扩展没有宣称实现。文字暂不支持复杂RTL/shaping。Token预算按Provider实际usage门限计量，最多可能多出一个完整响应，不能作为账单硬上限。

用户明确授权本轮SDK上传/更新私有诊断Avatar和窗口操作，并允许不保存旧场景。已完成上述授权内的实际上传与服务端检查；没有启动VRChat或向9000发包。原Stage3文件与Avatar ID保持不变。FullFT源场景和独立诊断场景已保存根名修复，最终FullFT副本已保存Pickup粒子碰撞力修复；商业Nemesis源未改。

## Avatar安全检查已修复并通过服务端复核

原Stage3的`AssetBundleFailedServerSideChecks`由原16384×1 Catalog metadata布局关联触发；改成完整256×64后，同一私有诊断Avatar在64、4096、16384容量都通过服务端分析。生产生成器保留全部4096样式、数据字节和资源GUID。

完整FullFT验证额外修复了场景根名未匹配Profile和人物RailGun粒子向Pickup施加碰撞力的问题。实际场景SDK/Play、16k显示、无睡眠审计通过后，最终Avatar已更新为 **MorphForge FullFT 16K**：

- ID：`avtr_df2ff222-b6ff-47fb-80be-e060868596a2`，private，Avatar v10/file v8，pending=false。
- 实际包11,072,116字节；总256bit、FT112项/161bit全部direct、控制8、几何56、定位14。
- Store16384/Infinity、Pump1；其它人物特效151套/41750容量单列。
- generic/standard/security三项终态200，VeryPoor；Pickup/Item碰撞力违规0。

[最终通过证据](security-check-failure/final-fullft-passed.json) · [逐项实包审计](security-check-failure/final-fullft-vrca.json) · [完整说明](security-check-failure/FINAL-FULLFT.md)。

真实客户端加载、多人同步与硬件抓取尚待用户现场确认，不能由服务端200代替。
[最小复核步骤](security-check-failure/CLIENT-RETEST.md)。因此源码先标记候选版本`v0.2.0-rc.2`，最终标签留待该项确认；桌面0.2.0安装器不变，Unity修复不属于69个软件构建输入。

## GitHub与Windows源码复核

源码获取方式见[GIT-SOURCE](GIT-SOURCE.md)。RC1交付复核发现Windows Git的
`core.autocrlf=true`会改变冻结构建输入；已通过`.gitattributes`统一代码LF并保留
报告原始字节。修正后的独立Windows克隆中，69项输入和安装器冻结清单完全一致，
[验证报告](quality-v2/windows-git-clone-validation.json)。该交付修正升级源码候选标签到
`v0.2.0-rc.2`，桌面安装器和Avatar上传包保持不变；RC1及失败证据保留。
