MorphForge 0.16.1：透明材质对齐与独立 Avatar 插件。

- Windows x64 安装包包含编辑器观察与截图的线性色彩合成修复，改善与 Unity Linear 渲染的透明度一致性。
- 首次随 Release 提供 MorphForgeAvatar 1.0.0 Unity 插件：一键安装、定位与抓握、MF6 显示系统、可选 VRCFury 参数压缩桥与最终预算检查。
- 提供配套公共材质目录 ZIP，含 544 项材质定义、54 张原创源贴图；不包含原版 Minecraft 或商业人物资源。

安装：通过 VCC 安装 VRChat Avatars SDK 与 Modular Avatar，再导入 unitypackage，在 Tools > MorphForge Avatar > 安装与检查 选择人物安装。VRCFury 可选。Desktop 导入配套材质目录并识别当前 Avatar 后发送。基础同步预算 253 bit，额外人物参数需通过预算检查。已有旧 MF6 模块的 Avatar 不应重复安装。

透明修复需要 Avatar 重新构建上传及安装新版 Desktop；本次发布不会自动更新私人 Avatar。交叠透明排序仍有限制。网络发送完成代表本机发送，不保证所有远端玩家完整接收；本版未改动 MF6 网络协议。

验证：公开 Unity 包依赖与源码一致性、干净工程导入、Modular Avatar 合并、VRCFury 压缩与预算负例、Unity 与 Chromium 透明度测试已通过。完整证据见 docs/avatar-plugin/README.md；本次新增构建与烟雾验证见 docs/release-v16.1/RESULTS.md。

附件包含 Windows 安装包、Unity 插件、公共材质目录、构建输入清单及 SHA256SUMS.txt。
