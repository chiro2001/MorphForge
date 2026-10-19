# MF4 0.8 验证与交付

日期：2026-10-20（Asia/Shanghai）。

## 上传结果

- 更新原私有 Avatar，名称 **MorphForge MF4 16K**。
- Avatar ID：`avtr_df2ff222-b6ff-47fb-80be-e060868596a2`。
- Avatar version **13**，资源 file version **10**，`pendingUpload=false`。
- generic、standard、security 三项服务端分析均终态通过，性能等级 VeryPoor。
- 资源包约 10.59 MB；实际服务端分析见 [analysis.json](analysis.json)，上传记录见 [upload.json](upload.json)。
- 最终预算 **254/256 bit**，其中三路几何 240、定位 14；面捕与本体的额外同步均为 0。37 项全部直接同步，见 [budget.json](budget.json)。
- 三个显示池各 5,462，配置合计 16,386；软件逻辑容量 16,384。此次未重新跑 16,384 条真实 UDP 首建，容量分配边界已做 16,384 Box 自动测试。

## 闪烁结论

历史自定义色板 ID 25/26 与生产目录的动画/六面测试材质碰撞。修复后：自定义材质缺少目标声明即拒绝入队；相同 ID 但 UID、颜色、纹理、动画等定义不同也拒绝。配套目录包含 47 种材质，不再把黄色映射为 0.05 秒换帧的测试材质。

外观改为命令内的完整 24 bit 数据，避免换刷子额外批次。RGB8 数据经 Shader 解码，关闭粒子颜色空间转换，数据不做片元插值。没有把网络声明当作真实远端 ACK。

## 验证

| 范围 | 结果与证据 |
|---|---|
| Linux Rust | 本轮全量 158 项通过；最终受影响专项 10 项通过（包含新增 MF4 回归及原 v0.7 接口回归） |
| Windows Rust | 本轮全量 158 项通过；最终受影响专项 10 项通过；日志在 gamepc `tmp/morphforge/mf4-windows-*.log` |
| 协议专项 | 字段边界、16,384 Box 三池均衡、UDP 60 Box/20 批、独立序号、材质碰撞/无声明拦截、错误协议拦截 |
| Unity 原生 UDP | **7 阶段通过**：48 个混合尺寸 Box、外观覆盖、删半、重画、空闲保持、广播清空；坐标、尺寸、RGB24/Alpha 数据零错误，见 [native-smoke.json](native-smoke.json) |
| Unity GPU | **22 项通过**：MF4 与 MF3 参考逐像素相同、4 个自定义颜色随时间保持静态，见 [visual.json](visual.json) |
| Chromium UI | **8 项通过**：真实按钮选择 MF4、预算显示、错协议/超容量拦截、离线发布、刷新持久化、无 JS/GPU 错误，见 [browser.json](browser.json) |
| 编译/安全 | Unity 无编译错误；SDK 上传成功；服务端三类分析通过 |

没有调用真实付费大模型接口，没有启动 VRChat，也没有向 9000 发测试包。Play 测试通过 9012。旧场景与旧安装未覆盖；测试时临时停用的旧头像/模拟器已恢复，见 [isolation.json](isolation.json)。

## 桌面部署

开发副本 **0.8.0** 已部署并启动：`%LOCALAPPDATA%\MorphForge Preview\versions\0.8.0-af1f9fde86c6`。原验收版二进制 SHA256 保持 `23c86a41…ee74d8a`。当前作品 ID/revision 保留，配套目录匹配、MF4/240bit、0.25 秒、9000 端口，发送关闭。见 [实际运行状态](live-preview.json)、[部署记录](installation.json)、[Windows 包运行检查](windows-runtime.json)。

## 需要用户验收

重新切换到更新后的 **MorphForge MF4 16K**，重开「MorphForge - 开发测试副本 0.8.0」。配套目录应为 MF4、几何 240 bit，建议间隔 0.25 秒。开启发送后检查本人和其他玩家的显示、不同颜色及删改；新加入玩家的完整刷新仍依赖全量重播，没有远端确认。

无面捕版本把非 MF4 的衣装/道具自定义参数改为本地；其他玩家看到的这些本体状态以打包默认值为准。自定义新纹理/新色板仍需更新头像目录，不能单靠 OSC 发送资产。
