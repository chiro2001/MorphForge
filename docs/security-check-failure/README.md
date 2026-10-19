# Stage3 人物上传后服务端安全检查失败

状态：最终 FullFT 已更新同一 private 诊断 Avatar，服务端 generic、standard、
security 三项均终态 200。实际上传包为 256 bit，FT/控制/几何/定位逐项匹配；
Store 16384、Pump 1、Grid metadata 256×64 与原生特效碰撞修复均已实测。当前
没有进行中的 operation。真实客户端加载和多人显示仍需用户现场确认。

## 最初失败与静态审计

- 2026-10-19 08:07:25/35/37，客户端在下载前拒绝 Avatar，错误为
  `AssetBundleFailedServerSideChecks`，安全分析 API 返回 403。详细
  [客户端证据](client-evidence.json) 已删去无关用户、房间和聊天信息。
- 原 Avatar `avtr_3ba403cb-569d-4198-8de6-678e59c4440f`；文件
  `file_d68d4ea2-015f-40cd-bd19-444a2e49b24a` 第 5 版。
- 实包 7,519,081 字节，解包 57,330,700 字节，与公开 PC 文件大小硬限相比余量充足。
- 实包包含 8 个 AnimatorController 资产、57 层、437 个状态、960 个转场和
  243 个唯一 AnimationClip；FX 44 层，其中 MF3 本身 3 层。
- 实包有两个粒子系统：Pump 容量 1、寿命 0.5 秒；Store 容量 16,384、
  `Infinity` 寿命、单次 burst 1。
- 同一 Avatar ID 在更早版本曾正常加载，但缺少当前第 5 版的同构可加载证据，
  不能据此证明 16,384 或 Infinity 本身合法。

完整静态审计见 [stage3-audit.json](stage3-audit.json)。公开性能评级不能给出
安全检查的精确规则；Very Poor 也不等于服务端扫描放行。

## 服务端对照结果

所有后续对照使用同一个 private 诊断 Avatar
`avtr_df2ff222-b6ff-47fb-80be-e060868596a2` 和同一个 bundle
`file_0e05d9cd-e04f-4bb2-9267-e01fad2fd7bf`。每组都审计了实际上传的 VRCA，
远端 descriptor 的版本、字节数和 MD5 与本地包一致。

| Avatar/File 版本 | 粒子容量 | 渲染路径 | Catalog 资源 | 服务端结果 |
|---|---:|---|---|---|
| 2 / 1 | 4,096 | 原 Catalog | 原 Shader、Atlas、16,384×1 metadata | generic/standard/security 403 |
| 3 / 2 | 64 | 原 Catalog | 原 Shader、Atlas、16,384×1 metadata | generic/standard/security 403 |
| 4 / 3 | 64 | LocalUV | 无 Catalog 资源 | 200 |
| 5 / 4 | 64 | Catalog Narrow | 原 Shader、Atlas、4,096×1 metadata，1,024 styles | 200 |
| 6 / 5 | 64 | Catalog Grid | 新 Grid Shader、原 Atlas、256×64 metadata，4,096 styles | 200 |
| 7 / 6 | 4,096 | Catalog Grid | 新 Grid Shader、原 Atlas、256×64 metadata | 200 |
| 8 / 7 | 16,384 | Catalog Grid | 新 Grid Shader、原 Atlas、256×64 metadata | 200 |

证据：

- [4,096 + 原 Catalog](upload4096.json)
- [64 + 原 Catalog](upload64-catalog.json)
- [64 + Legacy LocalUV](upload64-legacy.json)
- [64 + Narrow metadata](upload64-metadata-narrow.json)
- [64 + Grid metadata](upload64-metadata-grid.json)
- [4,096 + Grid capacity](upload4096-grid-capacity.json)
- [16,384 + Grid capacity](upload16384-grid-capacity.json)

汇总判断：

1. 粒子容量不是触发项：16,384 和 4,096 失败后，64 仍失败。
2. 原 Catalog Atlas 不是触发项：Narrow 和 Grid 都保留原 Atlas 并通过。
3. 整个 Catalog 方案不是触发项：只改 metadata 布局后，两种独立布局都通过。
4. 当前唯一保留的主要嫌疑是原 `16,384×1` RGBAFloat metadata 资源或其被
   Shader 直接消费的方式。现有证据还不能区分纹理尺寸、行宽、采样方式和该
   metadata 内容子集的作用。
5. Grid 版本保留全部 4,096 styles；随后同一诊断 Avatar 按顺序通过 4,096 和
   16,384 容量，说明容量本身与 Grid metadata 的组合可通过服务端分析。
6. 4,096 和 16,384 的包内 Store 容量分别为 4,096 和 16,384，寿命均为
   Infinity；16,384 安全分析报告的 `totalMaxParticles` 为 16,385（加 Pump 1），
   `meshParticleMaxPolygons` 为 196,608（16,384 × 12）。

generic/standard 报告中的 `fileSize` 与远端 descriptor 一致；security endpoint
报告的 `fileSize` 略小。Legacy、Narrow 和 Grid 均保留了这一差异，Verdict 仍为
200，当前没有证据把它与安全检查失败关联。

## 实际包审计

Narrow：

```text
VRCA       e4ace73d-65ff-49ac-9a3a-0dd301118e1e.vrca
bytes      7,529,458
SHA-256    287d5d7602178c78a0df036000eb09dec45a9eaf7022bb4153698dc8820f82a2
metadata   MetadataNarrow, 4096x1, RGBAFloat
Atlas      CatalogAtlas, 384x96
```

Grid：

```text
VRCA       7e76efa8-7cce-48b3-8352-0cb81a5642da.vrca
bytes      7,525,015
SHA-256    0c6935f76c71c7c31d17046f8bdfa163cffc9cbf329d0b1bd7fdac06f186bcec
shader     MorphForge/ParticleBoxCatalogMetadataGrid
metadata   MetadataGrid, 256x64, RGBAFloat
Atlas      CatalogAtlas, 384x96
```

两个包中的 Store 都是 `maxParticles=64`、`startLifetime=Infinity`。因此
materials 与 metadata 的差异是当前可审计的有效变量。

容量 Grid：

```text
4,096 VRCA  17c3490f-ddbe-4e1e-8856-4e9264248d03.vrca
bytes       7,545,859
SHA-256     e4595f3cd20f4245042e07f436b28ee7f6de0ee993445bba485669572fa97a72
Store       4,096, Infinity
security    generic/standard/security 200, totalMaxParticles=4,097

16,384 VRCA 75b0977a-2866-4e80-a3c8-9ceb5ddb7081.vrca
bytes       7,536,622
SHA-256     f03f315dd63da1dd37fed6413e8a261180fc2a8ad337fccaa57469bdbdffa588
Store       16,384, Infinity
security    generic/standard/security 200, totalMaxParticles=16,385
```

两个容量包都保留 Grid Shader、`MetadataGrid 256×64 RGBAFloat` 和原
`CatalogAtlas 384×96`；没有原 `16384×1` Catalog metadata。

## 当前现场与下一步

- 最终 FullFT 诊断场景：
  `Assets/MorphForge/Diagnostics/FinalFullFT/MorphForge_FinalFullFT_Diagnostic.unity`
- 场景使用生产材质 `ParticleBoxCatalog.mat`，Store 容量 16,384、寿命 Infinity。
- 场景已保存、无未提交修改，唯一 Avatar 绑定诊断 ID
  `avtr_df2ff222-b6ff-47fb-80be-e060868596a2`。
- 场景 Avatar 根名为 `MorphForge_FullFT_Avatar`，并通过真实 SDK clone 的
  FullFT profile 门禁。
- 原 Stage3、源 FullFT 场景/Prefab、原 Catalog 的 GUID、原 Avatar ID 均未改变。
- 当前没有进行中的上传或 SDK operation，没有启动 VRChat，没有发送 OSC。

最终 FullFT 已上传并通过实际包审计及三项服务端终态检查，证据见
[最终 FullFT 流程](FINAL-FULLFT.md) 和
[最终通过摘要](final-fullft-passed.json)。仍需用户确认真实 VRChat 客户端加载
和多人显示；不能由 Editor、VRCA 或服务端分析替代客户端验收。

[官方参考与阈值的适用范围](sources.md)。
