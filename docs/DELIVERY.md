# MorphForge 0.2.0 交付记录

日期：2026-10-19。Rust独立核心、共享体素工作台、DeepSeek Harness、可选MF3发布适配器及Windows Tauri安装器已交付。Git分支为 `feat/harness-interaction-and-voxel-text`，基线tag为 `v0.1.0-baseline`，计划发布tag为 `v0.2.0`，客户端诊断收尾后建立；没有推送远端。

## 安装与使用

Windows x64安装器：

```text
D:\Projects\VRChatProjects2\MorphForge\dist\windows\MorphForge Desktop_0.2.0_x64-setup.exe
```

大小13,613,538字节，SHA-256：

```text
CE30B61609FF93C250840560D14AB4278709E7BBD37EACDA63F736B4FCCBEF03
```

Linux工作区有相同副本 `dist/windows/`，该目录保留安装器校验清单；完整Git历史bundle将在本轮客户端诊断收尾后生成。安装器未代码签名。

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

本轮没有自动上传Avatar、启动VRChat或向9000发包。用户原Stage3场景仍打开且有未保存修改，未保存、关闭或丢弃。

## 新增客户端问题（待定位）

2026-10-19用户手动上传`MorphForge_Stage3_Avatar`后，客户端报告`AssetBundleFailedServerSideChecks`。这是Stage3实际上传包的服务端检查失败，不能由FullFT的Editor/SDK验收排除。详见[诊断证据](security-check-failure/client-evidence.json)；具体命中项尚不明确，完整客户端验收暂未通过。桌面0.2.0已安装供用户手测，保持运行。
