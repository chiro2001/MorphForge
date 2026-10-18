# Stage4 原生材质收尾与视觉验证

> 最终补充：此材质阶段结束后，FullFT组合已在相同最终Fire→Done资产上完成50条有效命令、166项完整API回归（0.5秒）并通过。见 [FullFT协议证据](../fullft/protocol-smoke.json)。未重跑整轮16k无损长测，历史失败保留。

状态：**已于 2026-10-19 执行，视觉、性能和短协议验收通过，但保留首遍 16k 与 256 首跑的偶发漏 Birth 限制。** 机器可读汇总见 `native-validation-status.json` 和 `native-strict-validation.json`。

## 所有权与回避范围

本轮新增：

- `Assets/MorphForge/Editor/Stage4FinishBuild.cs`
- `AgentScripts/Stage4FinishEntry.cs`
- `AgentScripts/Stage4VisualStrict.cs`
- `docs/materials/native-validation.md`
- `docs/materials/native-validation-status.json`

不修改：

- `Mf3DisplayBuild.cs`
- `PlacementStage4Build.cs`
- `ParticleBoxCatalog.shader`
- 源码中的构建器和 Runtime 组件

Finish 会通过 Unity API 重建生成的 FX controller、材质、Atlas，并更新 Stage4 生成 Prefab；不会手工改 `.unity`、`.prefab`、`.mat`、`.anim` 或 `.meta`。

## 执行结果

| 项目 | 结果 |
| --- | --- |
| Finish / Inspect | 通过；100 材质，Atlas 384×96，Fire 0.03s，Pump 0.5s |
| Catalog 材质 | `ParticleBoxCatalog` 已绑定 Geometry Display、Placement、Stage4 Avatar |
| 顶点流 | 三者均保留 Normal / UV / UV2 / SizeXYZ |
| Strict 视觉 | 40 项通过 |
| 根刚体纹理 | MAE 0、变化像素 0 |
| Catalog 16k 临时 rig | count 16384、visible；p50 8.92ms、p95 11.80ms |
| 主协议（FireHold 前基线） | 166 项通过 |
| 256×0.25s 第二遍 | 256 native / 256 unique / metadata 0 mismatch |
| 256×0.5s 对照 | 256 native / 256 unique / metadata 0 mismatch |
| NDMF 基本验证 | 通过；0 missing script、0 custom Avatar script、159bit |
| 最终 Unity | Edit Mode、非编译、Fire 0.03→Done；交回 FullFT |

最终源场景为 `Assets/MorphForge/Scenes/MorphForge_Stage3.unity`，在本轮开始时已 dirty；Finish 没有保存或修改该场景。实际交付场景仍是 dirty，等待用户自行保存。

FireHold 曾用于尝试稳定 Birth 时序，但两次定向试验都在 Fire 状态停住，因此已撤销。产品状态和临时 clip 已恢复为 Fire 0.03→Done；失败报告保留在 `protocol-validation-firehold-attempt*.json`。

主协议的 166 项通过记录是在 FireHold 实验前、同一 Catalog 与 Fire 0.03 资产上取得。FireHold 撤销后通过 Mf3DisplayBuild 和 Finish 重建，控制器检查为 Fire→Done、3 层；没有在最后一次 Edit Mode 交付状态上再重跑整批协议，因此报告明确保留这个版本时序边界。

## 输入

Unity 工程中的输入根：

```text
Docs/Materials/
  catalog.json
  artifacts/*.png
```

当前 union 记录为 100 个材质、7 个 artifact：

- builtin 1..24；
- synthetic 25：3 帧，0.05 秒/帧，蓝→红→紫；
- synthetic 26：六面 +X 红、-X 绿、+Y 蓝、-Y 黄、+Z 青、-Z 品红；
- legacy 艺术材质 257..330 等。

`CatalogMaterialBuild` 会校验 styleId 唯一且在 `1..4095`，解析 `/artifacts/<hash>.png`，生成：

```text
Assets/MorphForge/MaterialsCatalog/
  CatalogAtlas.png
  CatalogMetadata.asset
  ParticleBoxCatalog.mat
```

## Edit Mode Finish

长测结束后执行：

```powershell
python tools/unity_remote.py --check-result run_script --file AgentScripts/Stage4FinishEntry.cs --entry Stage4FinishEntry.Finish --timeout_ms 180000
```

顺序固定为：

1. `Mf3DisplayBuild.Build()`；
2. `CatalogMaterialBuild.Build()`；
3. 校验 catalog 中存在 style 25/26；
4. 将 `ParticleBoxCatalog.mat` 绑定到 `Store` 的 `ParticleSystemRenderer`；
5. 将 `Pump` 的 `startLifetime` 固定为 `0.5s`；
6. 验证 FX 的 `Fire` clip 长度为 `0.03s`；
7. 更新 Prefab 资产。

目标 Prefab：

| 路径 | 要求 |
| --- | --- |
| `Assets/MorphForge/Stage4/Geometry/Display.prefab` | 必须 |
| `Assets/MorphForge/Stage4/Placement/Placement.prefab` | 必须 |
| `Assets/MorphForge/Stage4/Avatar.prefab` | 必须 |
| `Assets/MorphForge/FullFT/Avatar.prefab` | 如存在则处理 |

只处理祖先层级中名为 `MorphForgeGeometry` 的模块：

- `Store`：绑定材质，关闭 Apply Active Color Space；
- `Pump`：0.5 秒寿命；
- 保留并补齐 `Position / Normal / Color / UV / UV2 / SizeXYZ` 顶点流；
- 不改 Avatar 其他粒子系统、动画、定位约束、相机或 Transform。

若目标 Prefab 中有多个 `Store` 或 `Pump`，脚本报错并停止，不猜测绑定目标。

## 当前场景策略

`Finish` 会在开始时记录所有已打开场景和 dirty 状态。

- 当前场景在调用前是 **dirty**：不修改、不保存，只在报告中记录跳过原因。
- 当前场景干净、恰好一个 `VRCAvatarDescriptor`、恰好一个 `MorphForgeGeometry`：只绑定该模块并保存该场景。
- 场景不满足上述条件：不修改场景。
- 无论场景是否被更新，都另建独立验证场景：

```text
Assets/MorphForge/Scenes/MorphForge_MaterialsValidation.unity
```

验证场景只包含相机、方向光和验证根，不含产品 Runtime 脚本。若固定路径已有资源，Finish 使用带时间戳的新路径并在 `native-finish.json` 记录实际路径，不覆盖现有验证场景。

不建议在有用户相机、Avatar 或其他未保存编辑的场景中直接运行像素验证。视觉验证使用临时 `ParticleSystem` rig，并在结束后销毁，不保存产品场景。

## 只读检查

在工作台操作前后都可以运行：

```powershell
python tools/unity_remote.py --check-result run_script --file AgentScripts/Stage4FinishEntry.cs --entry Stage4FinishEntry.Inspect --timeout_ms 60000
```

报告：

```text
Docs/Materials/native-inspect.json
```

检查：

- catalog 中的数量与 fixture 25/26；
- 目标 Prefab 是否存在；
- 每个模块的 Store/Pump 数量；
- Shader 名称；
- Pump 寿命；
- Normal/UV/UV2/SizeXYZ 流；
- Fire clip。

## Play Mode 原生视觉 fixture

长测结束后，准备一个独立的干净 Play 会话，再执行：

```powershell
python tools/unity_remote.py editor_play
# 轮询编辑器状态，确认已进入 Play 且未在编译
python tools/unity_remote.py --check-result run_script --file AgentScripts/Stage4FinishEntry.cs --entry Stage4FinishEntry.ValidateVisual --timeout_ms 120000
python tools/unity_remote.py editor_exit_play
```

严格检查入口：

```powershell
python tools/unity_remote.py --check-result run_script --file AgentScripts/Stage4VisualStrict.cs --entry Stage4VisualStrict.Run --timeout_ms 120000
```

`Stage4VisualStrict` 位于 `AgentScripts/`，不会进入 Avatar。它会重新解码已保存的 PNG，并额外断言：

- fixture25 三块 ROI 的前景色像素至少 45%，其中期望蓝色/红色/紫色每块至少覆盖 72%；
- fixture26 六张图的中心 ROI 前景至少 55%，期望颜色至少覆盖 82%；
- scale 1 与 scale 0.5 的中央 217×217 区域逐像素平均绝对差低于 6%，变化像素比例低于 30%；
- 两张 scale 图的中心横向扫描线格线过渡数相差不超过 1；
- 所有 PNG 文件存在、带 PNG 签名且尺寸完全匹配。

任何一项失败都会使 `Stage4VisualStrict.Run` 抛出异常，Pipeline 返回失败；不会只留下一个 `success:false` 报告。

审阅边界：`Stage4FinishBuild.ValidateVisual` 本身有真实像素分类，但六面和三帧最初只取每个 ROI 的平均颜色，scale 也只比较整幅 MAE。`Stage4VisualStrict` 补上了对**保存后 PNG** 的重解码、逐像素颜色覆盖、局部区域逐像素差异和格线过渡计数；它仍只验证临时原生材质 fixture，不会向产品 Avatar 注入测试粒子，也不产生 OSC 证据。

## 固化 Fire .03 后的真实 UDP 短回归

只有在 `Stage4FinishBuild.Finish` 和视觉 fixture 完成后，且长测原发送器已停止时，才执行：

```powershell
python tools/check_stage4_finish_protocol.py
```

脚本使用真实 Windows UDP `9012`，复用 Stage4Capacity 格网的前 256 个 Unit 中心。它们按 3 格间距排列，不能合并为少数长方体。发送节奏为每命令 `0.25s`，总共至少覆盖 16 个序号回绕周期。

检查项：

- `received` 和 `completed` 各自增加 256；
- 原生粒子数为 256；
- 256 个中心互不重复；
- 每个 Unit 的 `startSize3D` 为 `0.25m`；
- 每个粒子的 metadata 解码为 `style=1, tint=0, motion=0`；
- 最终 `busy` / `inbox` 已清空。

报告只写：

```text
Docs/Materials/protocol-regression-256.json
```

不会写 `Docs/Stage4/capacity-progress.json`，也不会覆盖原有 16k 长测报告。测试粒子均来自外部 UDP；Unity 脚本只读观察，不通过 `SetParticles` 注入产品 Store。

这个 256 条回归只能证明短时协议、序号和原生 metadata 链路，不能修复或抵消正式混合长测首轮已有的 192 缺口（125 条命令）。首轮和补发/满池回收必须分别报告；在补发完成并重新核验前，不能声称新版 16k 首轮无损。

实际短回归记录：

| 轮次 | rx/done | native | 结论 |
| --- | ---: | ---: | --- |
| 0.25s 首次 | +256/+256 | 255 | 缺索引 28；说明 capture/Done 完整但一颗粒子漏 Birth |
| 0.25s 第二次 | +256/+256 | 256 | 通过 |
| 0.5s 对照 | +256/+256 | 256 | 通过 |

三份报告分别保留为 `protocol-regression-256-attempt1.json`、`protocol-regression-256-attempt2.json` 和 `protocol-regression-256-interval050.json`；不能只看第二次而删掉首次失败证据。

## Catalog 16k 临时显示性能

新增 `AgentScripts/Stage4CatalogPerf.cs`，必须在 Play Mode 中运行：

```powershell
python tools/unity_remote.py --check-result run_script --file AgentScripts/Stage4CatalogPerf.cs --entry Stage4CatalogPerf.Run --timeout_ms 120000
```

它创建临时原生 ParticleSystem，明确标记为非 OSC：

- 16,384 个 Unit，同一 Particle Box 网格；
- 循环混合 metal、water、cloud、cutout 材质；
- 使用已构建的 `ParticleBoxCatalog.mat`；
- 检查粒子 count、Renderer visible 和非黑前景像素；
- 采样 6 秒的帧时 p50/p95/max；
- 输出 `Docs/Materials/native/catalog-16k-performance.png`；
- 输出 `Docs/Materials/catalog-16k-performance.json`。

这个测试不访问 Avatar Store，不发送 OSC，也不代表此前的 16k UDP 长测。之前的满池长测使用旧 Shader，不能代表新 Catalog 材质成本；VRChat 客户端 GPU、逐粒子透明排序和用户过滤仍是待验项。

`ValidateVisual` 不依赖产品 Prefab 的相机或粒子状态。它创建临时场景对象和原生 ParticleSystem：

- 使用真实 `ParticleSystem.SetParticles`；
- 使用真实 `ParticleSystemRenderer` Mesh 模式和 `ParticleBoxCatalog`；
- 使用与 `ParticleStage2Build` 相同的面局部 UV 网格：`uv=tileUV`、`uv2=(axis,0)`；
- 设置真实 `UV / UV2 / SizeXYZ / Normal` 流；
- 用 `Camera.Render()` 和 `Texture2D.ReadPixels()` 生成 PNG；
- 结束后销毁临时 rig；
- 不启用 OSC，不连接 VRChat，不调用产品发送器。

### Fixture 25：动画首帧顺序

目标：

```text
frame 0 = blue
frame 1 = red
frame 2 = purple
```

测试放置三颗同样的动态 cube，在捕获时使用 A（motion）相位把三颗分别对齐 frame 0/1/2。这样不依赖编辑器 `_Time` 从零开始，也不只检查“画面有变化”。

输出：

```text
Docs/Materials/native/fixture25-frames.png
```

中心颜色会被分类为 blue/red/purple；任一顺序错误即为失败。

### Fixture 26：六面方向

六面顺序：

| 相机观察面 | 期望 |
| --- | --- |
| +X | red |
| -X | green |
| +Y | blue |
| -Y | yellow |
| +Z | cyan |
| -Z | magenta |

Unity 的 `+Z` 在 Shader 面槽应为 **4**；Three.js 的正面投影顺序相反，WebGL 侧 5/4 的转换不能直接复制到 Unity。此项专门防止把 Unity +Z 错映射为 -Z。

输出：

```text
Docs/Materials/native/fixture26-px.png
Docs/Materials/native/fixture26-nx.png
Docs/Materials/native/fixture26-py.png
Docs/Materials/native/fixture26-ny.png
Docs/Materials/native/fixture26-pz.png
Docs/Materials/native/fixture26-nz.png
```

每个视角的中心像素按 RGB 邻近期望颜色分类。截图和分类值都写入 JSON。

### 材质类别可见性

同一视图放置：

- style 10：金属；
- style 16：水；
- style 19：云；
- style 21：无纹理 cutout。

输出：

```text
Docs/Materials/native/catalog-styles.png
```

每颗 cube 的中心区域前景像素比例必须大于阈值。这项检查 Opaque/Cutout pass 与 Transparent pass 的分类是否把材质放到了正确路径；它不证明透明排序或物理水体。

### Scale 0.5 的 UV 稳定性

使用 style 1 的程序化格纹理：

1. 内容缩放为 1，相机取景适配；
2. 内容缩放为 0.5，相机距离和正交尺寸同步减半；
3. 比较两张归一化截图。

输出：

```text
Docs/Materials/native/scale-1.png
Docs/Materials/native/scale-half.png
```

期望局部格纹理比例和重复数不变；报告记录 mean absolute error 与阈值。若 0.5 时格数变化，说明 SizeXYZ、`_CellSize` 或父级缩放路径不一致。

像素检查结果：

```text
Docs/Materials/native-visual-validation.json
Docs/Materials/native-strict-validation.json
```

## 透明和体积效果限制

这是单一 ParticleSystemRenderer 的材质近似：

- 双 Pass 只负责筛选 opaque/cutout 与 transparent，不提供逐粒子正确排序；
- 一个材质只有一个整体 Queue，不透明粒子共享透明队列的绘制时机；
- 水中只有颜色、alpha 和 UV 波动，没有折射、吸收、岸边泡沫或物理水面；
- 云只有局部噪声，不是体积云、体积散射或全局光照；
- 金属为简化表面 PBR，不是完整 IBL；
- 透明粒子交叉、重叠和深度关系必须按实际截图评估，不能因“有像素”就判定正确。

## 验收判定

`native-finish.json` 只能证明 Edit Mode 构建与序列化结果。

`native-visual-validation.json` 的 `success` 只有同时满足以下条件才可为 true；`native-strict-validation.json` 还必须通过重新解码 PNG 后的覆盖率和逐像素差异断言：

- fixture 25 三帧顺序为蓝/红/紫；
- fixture 26 六个视角与六面颜色表一致；
- 金属、水、云、cutout 均有可见前景；
- scale 0.5 的归一化截图 MAE 低于阈值；
- 所有截图文件实际写入；
- 运行过程中没有临时 rig 残留在产品场景。

即使上述通过，也只代表 Editor Play Mode 原生粒子渲染。仍需：

1. NDMF 导出检查；
2. 实际 VRChat 客户端截图；
3. 16k 透明与 Opaque 混合性能；
4. 玩家移动、Avatar 旋转和世界定位下的纹理稳定性；
5. 多人客户端与晚加入显示验证。

## 长测后执行检查表

- [ ] 确认 16k 长测已结束，Unity 退出 Play Mode。
- [ ] 确认 `Docs/Materials/catalog.json` 与 `artifacts/` 是 union 输入。
- [ ] 运行 `Stage4FinishEntry.Finish`。
- [ ] 检查 `Docs/Materials/native-finish.json` 的 Prefab 列表、Pump 寿命和 Fire clip。
- [ ] 运行 `Stage4FinishEntry.Inspect`，确认所有必需 Prefab 的 Normal/UV/UV2/SizeXYZ。
- [ ] 单独准备干净 Play 会话，运行 `Stage4FinishEntry.ValidateVisual`。
- [ ] 查看生成的 PNG 与 `native-visual-validation.json`。
- [ ] 退出 Play，确认没有临时 fixture 对象或运行时发送器残留。
- [ ] 执行 NDMF 导出和真实客户端验证。
