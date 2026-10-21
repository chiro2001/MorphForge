# MorphForge Avatar 插件 1.0.0

将 MorphForge Desktop 的体素和纯色三角面，通过 OSC 显示在 VRChat PC Avatar 上。容量为 16,384 个几何对象；显示使用原生 Animator、粒子和 Shader，上传后没有自定义 C# 运行逻辑。

## 安装

1. 使用 VCC 创建或打开 Avatar 工程，安装 VRChat SDK 和 Modular Avatar。
2. 导入 `MorphForgeAvatar-1.0.0.unitypackage`。
3. 在场景中选择人物，在 `Tools > MorphForge Avatar > 安装与检查` 指定 Avatar，点击“一键安装”。
4. 需要压缩服装等低频菜单参数时，先在 VCC 安装 VRCFury，再勾选参数压缩桥。已经安装的模块可移除后重新安装；移除和安装均支持 Undo。
5. 正常构建、上传 Avatar。在 VRChat 打开 OSC，用 MorphForge Desktop 识别 Avatar 能力并发布作品。

已经存在 MF6 模块时先移除旧模块，避免重复接收命令。安装不会改写人物的原始控制器、菜单或参数资产，Modular Avatar 在构建阶段合并资源。SDK、MA 和可选的 VRCFury 需由 VCC 管理，不包含在本包中。

已验证：Unity 2022.3.22f1、SDK 3.10.5、MA 1.18.7、NDMF 1.14.8、VRCFury 1.1430.0。使用 Windows/PC 目标；未验证 Android/Quest。

## 同步预算

| 项目 | 默认开销 |
|---|---:|
| MF6_D00…28、MF6_Ctl，30 个直接同步 Int | 240 bit |
| 世界固定、保持水平、重置、左右手抓握，5 Bool | 5 bit |
| 缩放，1 Float | 8 bit |
| 光球可见性、材质指纹、协议声明 | 本地，0 bit |
| 合计 | **253 bit** |

光球只向本人显示，抓握与几何位置仍同步给其他玩家。世界固定和缩放使用原有原生约束方案。

VRCFury 使用公共 FullController API 建立空桥接，启用其自动菜单参数压缩，不使用已经废弃的 Unlimited Parameters 组件。只有适合低频更新的菜单参数可以压缩；数据通道不能串行压缩，连续面捕参数也不能靠该方案无损腾空。它可能保留更多并行槽来用满 256 bit，这是正常现象。

几何数据不放进压缩菜单。最终检查在 VRCFury 压缩之后运行，要求全部 30 个数据参数依然是直接同步 Int，并检查总预算 ≤256。超限会阻止构建，插件不会擅自关闭用户其他参数的同步。构建报告：`Logs/MorphForge/avatar-budget.json`。

VRCFury 可能针对人物原有 Write Defaults 配置提示修复；这是其正常工作流。本插件不修改全局 VRCFury 偏好，也不替用户自动改写人物原有策略。原生 Direct Blend Tree 的 Write Defaults On 必须保留。

## 材质目录

内置 544 项公共材质，含“像素工坊”“方块工坊”的原创像素纹理及状态变体，不包含 Mojang 原版贴图或任何人物模型。

Avatar 的静态纹理目录必须与 Desktop 对应；本地参数 `MF_Catalog00…31` 声明 SHA-256 指纹，Desktop 可据此识别。窗口中的“导出内置 Desktop 配套材质目录 ZIP”可以导出匹配目录，在 Desktop 导入该目录作为目标 Avatar 的贴图库。

需要自己的纹理时：在 Desktop 导出 Avatar 材质目录，解压后，在窗口中选择包含 `catalog.json` 和 `artifacts/` 的文件夹。插件会在 `Assets/MorphForgeAvatar/Variants/` 创建独立 Prefab、Decoder 和材质，不覆盖基础包或其他变体。选中生成的 Prefab 后使用“安装所选变体”。更新纹理后需要重新上传 Avatar；普通纯色和实时程序材质不需要重新上传。

自定义资源的来源许可由使用者管理。公共打包脚本排除 `Variants/`，不会把用户导入的原版 Minecraft 资源混入公共包。

## 透明度

Shader 现在为 RGB 与 Alpha 使用独立混合因子；新版 Desktop 的观察和截图先在线性空间混合，再输出 sRGB，与 Linear Unity 工程一致。

目录材质保留精确 opacity，最终透明度为“贴图 Alpha × 材质 opacity”。实时程序材质仍受 MF6 的两位透明度编码约束：0、1/3、2/3、1。请使用 Desktop 的 VRC 预览判断发送后的有效值；艺术预览允许更精确的艺术值。

粒子内已启用距离排序。相交透明表面、跨显示池的透明对象仍不保证完全正确的全局排序；这不是修改 opacity 可以消除的问题。不同 VRChat 世界的灯光、后处理也可能改变最终观感。

## 源码和验证

源码、构建脚本与验证文档位于 MorphForge 仓库；`docs/avatar-plugin/README.md` 记录测试范围。包内资产可在 Unity 正常检查和修改，不依赖原作者的商业 Avatar 工程。
