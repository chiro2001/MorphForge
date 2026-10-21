# 独立 Avatar 插件与透明度对齐

日期：2026-10-21。插件版本：1.0.0。此轮交付独立 `.unitypackage`、配套公开材质目录和源码修复，不创建新的 GitHub Release。

- [Unity 插件](../../dist/avatar/MorphForgeAvatar-1.0.0.unitypackage)
- [Desktop 配套材质目录](../../dist/avatar/MorphForge-public-catalog.zip)
- [安装、预算、材质变体说明](../../Assets/MorphForgeAvatar/README.md)

## 修复内容

旧编辑器把每个透明表面转换成 sRGB 后直接混合到 Canvas；当前 Unity 工程使用 Linear 色彩空间。相同 opacity 在这两条路径下观感不同。编辑器观察和截图现在都先渲染到线性 HalfFloat/MSAA 目标，再做最终 sRGB 输出。VRC 预览不额外加入艺术模式的 ACES tone mapping，艺术模式在合成之后统一应用。

Unity 透明 pass 原先把 RGB 混合因子同时用于 Alpha。黑/白不透明背景上的 50% 透明材质，输出 Alpha 会变为 0.75。现在使用独立因子：`Blend SrcAlpha OneMinusSrcAlpha, One OneMinusSrcAlpha`，输出 Alpha 保持 1。没有通过修改 opacity 常数掩盖颜色空间差异，也没有更改 MF6 线协议。

目录材质支持精确 opacity，贴图 Alpha 与 opacity 相乘。实时程序材质仍只有 0、1/3、2/3、1 四档；艺术预览与 VRC 有效值的区别依然存在。交叠透明表面和跨粒子池的全局透明排序也仍有局限。本轮不宣称已实现顺序无关透明或所有世界中逐像素相同的照明效果。

## 插件组成

只导出 `Assets/MorphForgeAvatar/` 中的公共基础资源和代码，用户生成的 `Variants/` 不参与公共导出。

- 编辑器安装器：选择人物后一键安装、移除和 Undo，阻止重复模块及保留参数命名冲突。
- 原生定位模块：世界固定、跟随人物、手部抓放、保持水平及缩放。定位球仅本人显示；抓握和作品位置仍同步。
- MF6：30 个直接同步 Int，240 bit；8 个显示池，配置几何容量 16,384。
- 本地能力声明：协议版本、材质指纹和插件标记，不占同步位。
- 材质：544 项公共目录，54 张经生成器验证的原创 PNG，无商业人物、Mojang PNG/JAR、私人 Avatar ID。
- 材质变体：从 Desktop 目录创建独立材质、Decoder 和 Prefab，更新指纹，保留基础资源。
- VRCFury 可选桥：调用公共 FullController API，触发其已有菜单参数压缩；无废弃 Unlimited Parameters 组件，无全局偏好改动。
- 最终构建保护：压缩之后验证全部数据参数仍直接同步并检查 256 bit 总预算。不能保证任意人物都能容纳本插件；不会通过关闭用户参数来制造预算余量。

修复了开发工程旧 MF6 gate 与独立模块的冲突：旧 gate 识别插件标记后跳过，不再用人物专用的同步白名单覆盖新插件的最终参数。旧配套人物的专用流程保持原有行为。

## 验证结果

| 验证 | 结果与证据 |
|---|---|
| Unity GPU 透明度 | opacity 0、0.1、1/3、0.5、2/3、0.9、1；测得混合因子与输入一致，修复后背景 Alpha 恒为 1。[修复前](unity-alpha-before.json)、[修复后](unity-alpha.json) |
| Chromium WebGL | 14 组测试覆盖精确目录 opacity、有/无 50% 纹理 Alpha；最大不透明度误差约 0.00405，输出 Alpha 为 1，截图生成及相机恢复通过。[报告](web-alpha.json) |
| 干净工程导入 | 新工程中通过 `AssetDatabase.ImportPackage` 导入，只有 SDK、MA、NDMF 和 Pipeline，不含旧 MorphForge 脚本与商业人物。合并后 253 bit，全部数据参数直接同步。[报告](acceptance-ma.json) |
| VRCFury 压缩 | 在同一隔离工程加入 VRCFury，合成 Humanoid 上额外放入 8 个服装菜单 Bool，真实合并/压缩后 256 bit；30 个数据 Int 保持直接同步。[报告](acceptance-fury.json) |
| 预算负例 | 264 bit 的不可压缩参数被拒绝且同步标记不被篡改；即使预算未满，数据通道取消直接同步也被拒绝。见两份 acceptance 报告 |
| 材质变体 | 导出 ZIP，再导入修改过 opacity 的目录；新材质 Alpha 为 0.37、指纹改变、基础 Prefab 不变、8 个显示池及脚本引用完整。[报告](catalog-variant.json) |
| 包资源检查 | 依赖闭包中的 Assets 全在插件根内；导出不包含第三方包源码；公共 PNG 的内容 SHA-256 与 artifact 名称一致。见 [package-audit.json](package-audit.json) |

Unity 像素测试使用生产 Shader 和材质、诊断标准 Cube，并非 native 粒子全链路的重新验收。SDK 验证使用程序生成的 Humanoid 测试骨架，依次调用实际 SDK preprocess callbacks，避免 SDK 的测试失败弹窗；包含真实 MA/NDMF 合并与 VRCFury 压缩。压缩测试在测试骨架上显式保留 Write Defaults 策略，没有把该策略写入公共插件或用户全局配置。

本轮没有连接真实 VRChat 发送、没有重新上传私人 Avatar，也没有重新执行完整 16K 长时显示验收。原 Avatar 的 Shader 修复需要重新构建上传后才会进入客户端；编辑器端改动需要安装包含新前端的桌面版本。用户正在运行的主程序、OSC 和数据保持原状。

## 复现

- 包资源与源码一致性：`python3 tools/check_avatar_package.py`。
- WebGL：`python3 tools/check_alpha_parity.py`，需要 Chromium/ChromeDriver。仅服务本地 Web 静态文件，不启动项目或 OSC API。
- 公开素材：`tools/cargo_local.sh run -p morphforge-app --example export_public_catalog -- materials/mf6/catalog.json <新的隔离输出目录>`。生成器拒绝非原创 texture 引用，去掉源 Avatar/项目绑定。
- Unity Shader：通过 Pipeline 运行 `AgentScripts/TransparencyPixelProbe.cs` 的 `TransparencyPixelProbe.Run`。
- 包抽取与导出：在现有开发工程通过 Pipeline 运行 `AvatarPluginBuild.Build`（首次生成）和 `AvatarPluginBuild.Export`。已有生成资产不被静默覆盖。
- 干净工程：通过 Pipeline 运行 `PluginImport.Run`，等待编译，再运行 `PluginAcceptance.Run` 和 `PluginCatalogCheck.Run`。测试脚本放在 `AgentScripts/`，不随插件导出。

Unity 操作遵循仓库的 `UNITY-GUIDE.md`；生成与修改 Prefab、控制器、材质以及导入导出均通过 Unity/Pipeline。
