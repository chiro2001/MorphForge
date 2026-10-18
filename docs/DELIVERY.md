# MorphForge 0.1.0 交付记录

日期：2026-10-19。软件核心、共享工作台、DeepSeek Harness、VRC/MF3 适配器及 Windows Tauri 安装包已完成；后续高级材质和实体运动扩展不计为本版已实现。

## 入口

- [设计与实施方案](plan/README.md)
- [使用指南](USER-GUIDE.md)
- [执行状态与验证索引](plan/STATUS.md)
- [Windows安装与构建验收](desktop/README.md)
- [FullFT预算、场景和功能验证](fullft/README.md)
- [16k容量与真实UDP记录](stage4/README.md)

Windows安装器：

```text
D:\Projects\VRChatProjects2\MorphForge\dist\windows\MorphForge Desktop_0.1.0_x64-setup.exe
```

Linux工作区副本：`dist/windows/`，含安装器、README及SHA256SUMS。安装器SHA-256：

```text
CC535107346818E150E95F71519015B48FEC478BA4DB0B123073D29AFE50F3FF
```

用户数据：`%LOCALAPPDATA%\MorphForge`。已准备DeepSeek配置及三份作品、九张多视图；密钥不在安装器、源码或诊断包中。原始作品库和现有工程未覆盖。

## 软件能力

Rust核心独立提供CLI/HTTP；共享前端支持人工绘制、精确切割、分组变换、撤销重做、版本和作品库。DeepSeek工具循环支持受限Recipe、多视角反馈、连续修改、详细日志与compact。Tauri负责窗口和进程生命周期，创作无需Unity或VRChat。

真实AI验证产生42 Box灯塔作品；compact估算上下文从8937降至1568 Token，续写保留全部旧几何并新增树，达到49 Box。原完整艺术精确转换为70,286个唯一体素、7,459个Box，可从作品库直接加载。

Windows原生核心35项、桌面2项、HTTP10项检查通过，安装、启动、退出和卸载已实测。发布包与44个冻结源文件的哈希一致。Linux核心36项测试通过。

## Avatar与预算

```text
Assets/MorphForge/FullFT/Avatar.prefab
Assets/MorphForge/FullFT/MorphForge_FullFT.unity
```

实际SDK联合构建为 **256 / 256 bit**：

| 项目 | bit | 同步方式 |
|---|---:|---|
| 完整面捕112项（105 Bool、7 Float） | 161 | 直通 |
| MF3几何（6数据Int + Ctl） | 56 | 直通 |
| 定位、缩放、水平与抓放 | 14 | 直通 |
| 8个必要面捕控制Bool | 8 | 直通 |
| 其余本体 | 17 | 直通 |
| 合计 | **256** | 无压缩carrier |

MA注入几何与定位，VRCFury集成完整面捕。通过本体参数本地化腾出预算，保留全部高频FT数据；具体关闭组及远端影响见FullFT文档与profile。其余本体17bit保留衣装7 Bool、摸头状态与AFK完成各1 Bool、VRCEmote Int8。

FullFT新场景真实UDP9012完整API回归通过：50条有效命令、166项检查；包括Box、Unit、Brush、覆盖、重复序号、Remove、区域Erase、Clear和40条连续命令的序号回绕。远端分支`IsLocal=0`的眼口输入改变了18个面部BlendShape/骨骼输出。

用户原Stage3场景文件保持不变，原未保存内容已复制到新场景；源场景仍保留未保存状态。Play期间的对象禁用只用于隔离测试，退出后恢复；新FullFT场景已单独保存。

## 显示容量与材质

真实16,384条绘制完整发送后首遍16,192个，缺192；以0.5秒补发后达到16,384唯一Box，零位置、尺寸和metadata错误。满池删除16、补回16、区域擦除64及清空均通过。

原生100材质目录、六面、三帧动画、云水金属/cutout可见性、0.5缩放及根刚体变换共40项严格像素检查通过。新双Pass材质16k混合显示夹具p50约8.92ms、p95约11.80ms，仅代表当时Editor整体帧时间。

## 保留的限制

- 无远端ACK，已发送不等于所有玩家已显示。0.25秒的256条短回归首次漏1个Birth、第二次完整；0.5秒对照完整。报告保留失败，不承诺任意负载下无损。
- 最终版本未再跑一整轮16k无损长测；容量/补发、材质夹具和最终FullFT功能回归分别给出证据。
- 自动测试仅发9012，未上传Avatar，未启动VRChat。真实多人、VR手势、客户端Particle Limiter及GPU成本仍需客户端观察。
- 当前PC/PCVR接受Very Poor。一个持久粒子池、双Pass不保证严格透明排序；水云是表面近似。
- Minecraft支持常见Java立方体、六面和动画纹理；不支持任意异形模型、PBR扩展、逐面染色遮罩、物理体积云或折射。新纹理用于Avatar时需要预打包并上传。
- `motionPhase`为6bit材质时间相位，实体位置/速度/骨骼运动属于后续扩展。

原始日志、Recipe、图片、不可变版本及失败证据均保留，compact不删除这些资料。
