# FullFT 实际场景未应用同步 Profile

2026-10-19，在最终 FullFT 诊断场景的直接 Play 中发现并修复。该问题与 Catalog
Metadata 的服务端拒绝是两项独立问题；未把失败 Play 当作成功证据。

## 原因及失败证据

`FullFTSyncOverride` 与 `FullFTBudgetGate` 通过 Avatar 根名称的
`MorphForge_FullFT` 前缀显式选择目标，避免修改其它 Avatar。
`FullFTBuild.CreateScene` 从 `Avatar.prefab` 实例化后未设置场景根名称，实际为
`Avatar`，因此两个回调都跳过了目标。直接 Play 出现 VRCFury 参数超限：原始
360 bit、最大压缩后仍为 282/256 bit，随后 SDK preprocess 失败。

[原始失败对话框局部截图](fullft-name-failure-dialog.png)仅包含该模态，没有无关桌面内容。
确认错误后退出 Play；没有上传这个失败版本。

此前 `FullFTEntry.JointBuild` 显式给临时 clone 设置了前缀，能验证参数配置本身，
却没有覆盖真实场景的目标识别。这也是本轮增加实际场景构建验证的原因。

## 修复

- `FullFTBuild.CreateScene` 显式按当前 Profile 设置场景 Avatar 名称，并在场景
  保存前及重新打开后验证其匹配。不会影响其它 Avatar。
- 通过 Pipeline 将现有 FullFT 源场景与独立诊断场景的根名称从 `Avatar` 改为
  `MorphForge_FullFT_Avatar`。保留位置、旋转、缩放、源 Prefab 和原 Stage3。
- 最终上传 helper 同样设置并验证前缀；刷新准备证据时只接受该次受限迁移。
- 这是构建 Profile 的显式约定，场景根名称须保留配置前缀。

## 验证

| 检查 | 结果 | 证据 |
|---|---|---|
| 场景名迁移及定位保留 | 两场景通过，Prefab 未改 | [迁移](fullft-scene-name-migration.json) |
| 实际场景 SDK callback 链 | 保留真实 clone 名称，256 bit；FT161、控制8 | [实际场景构建](fullft-actual-scene-build.json) |
| 直接 Play 远端面捕分支 | `IsLocal=0`，18 项输出变化 | [面捕烟测](fullft-final-remote-smoke.json) |
| 正式 Grid 材质 16k 显示 | 16384 粒子可见，784 帧，p50 7.65ms / p95 9.15ms | [显示夹具](catalog-grid-performance/report.json) |
| 无睡眠依赖 | Prefab 与已保存源场景都通过 | [Prefab](fullft-final-nosleep.json)、[场景](fullft-final-scene-nosleep.json) |
| 最后一次 JointBuild | 256/256，FT、MF、定位、控制全部 direct | [构建](fullft-production-joint-build.json)、[预算](fullft-production-final-budget.json) |

显示性能只针对当前 Editor 中的临时粒子夹具，不是 OSC 可靠率、实际 Avatar Store
或 VRChat 客户端 GPU 性能。面捕烟测也不是实际多人或硬件面捕验收。
