# MorphForge / 造形工坊

独立体素艺术工作台：手工编辑、DeepSeek AI 创作、作品库与多视角预览共用一个前端，Rust 核心可独立运行在 Windows。VRChat Avatar 是可选发布目标，不是创作和保存作品的前置条件。

- [用户指南](docs/USER-GUIDE.md)：安装、配置、编辑、AI、compact、作品库与 OSC 发布。
- [交付记录](docs/DELIVERY.md) · [实施状态及证据](docs/plan/STATUS.md) · [已确认设计](docs/plan/README.md) · [验收标准](docs/plan/ACCEPTANCE.md)。
- Windows **0.2.0** 安装器：`dist/windows/MorphForge Desktop_0.2.0_x64-setup.exe`；同目录保存 SHA-256。[本轮质量与验收记录](docs/quality-v2/RESULTS.md)保留真实模型多轮迭代、Windows 与前端验证证据。

## 当前实现

共享工作台支持稀疏 Box 几何、精确切割、分组变换、撤销重做、版本快照和资源包材质。Harness 提供受限 Recipe、多视角截图、工具调用循环、插话、打断续接、排队、暂停恢复、详细日志和上下文 compact；compact 保留原始日志、图片、Recipe 与作品版本。提示词与工具说明优先中文，并提供英文版本。字体文字栅格化为普通可编辑 Box，支持预览与撤销；保护分组可锁住既有作品。真实 DeepSeek 多轮创作、交互、compact 和收尾入库已验证，失败与接续修正记录一并保留。

独立作品库已有铜灯塔及其续写版本，以及旧完整场景的精确转换：70,286 个实际占用体素无损合并为 7,459 Box。Minecraft Java 资源包支持常见整立方体、六面与动画纹理；当前云、水、金属等使用体素表面材质表现。

VRC 主档为 **MF3：56 bit 几何通道 + 14 bit 定位**。完整面捕固定预留 **161 bit**，其余本体功能和压缩承载预算 **25 bit**。MA 注入几何和定位模块，VRCFury 负责完整面捕及低频参数压缩；实际SDK联合构建已通过256/256bit，FT161与MF70全部直通；通过精简本体同步实现，无压缩carrier。最终 prefab、上传场景及生成入口均已排除睡眠插件依赖。已通过补发到达 16,384 个唯一 Box 及满池回收验证。显示池持久保存每个 Box 的位置、尺寸和外观编码，接收动画器保持固定规模，不为每个 Box 创建一组状态；真实 UDP 首遍缺失、最终材质及 FullFT 联合构建结果见实施状态。

自动测试只向 9012 发送，不向 9000 发送，也不会上传 Avatar 或启动 VRChat。网络发布没有远端 ACK；本机发送进度不能证明其他玩家已经显示。客户端、多人及 VR 手势仍需要用户之后实际确认。

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
