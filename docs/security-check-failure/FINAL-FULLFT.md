# 最终 FullFT 同诊断 ID 验证流程

状态：最终 FullFT 已更新同一 private 诊断 Avatar，服务端
generic/standard/security 三项均终态 200。实际上传包逐项验证为 256 bit，
FT 112 项/161 bit、VRCFT 控制 8 bit、MF3 几何 56 bit、定位 14 bit；Store
16384/Infinity、Pump 1、Grid metadata 256×64、Atlas 384×96。真实 VRChat
客户端加载与多人显示仍需用户现场确认。

## 最终可切换对象

```text
Avatar display name  MorphForge FullFT 16K
Avatar ID             avtr_df2ff222-b6ff-47fb-80be-e060868596a2
Avatar version        10
File version          8
Release status        private
Pending upload        false
Bundle SHA-256        9db310813431e0e85744beec22cfbc58494c7e5f70aacc2af1360b1da4c0cbfc
```

最终证据：

- [最终通过摘要](final-fullft-passed.json)
- [上传 operation](final-fullft-upload.json)
- [实际上传 VRCA 逐项审计](final-fullft-vrca.json)
- [三项服务端分析](final-fullft-analysis.json)
- [上传时 final-budget](final-fullft-upload-final-budget.json)
- [Collision force 修复报告](fullft-collision-force-fix.json)

## 固定目标与隔离边界

```text
Diagnostic Avatar ID  avtr_df2ff222-b6ff-47fb-80be-e060868596a2
FullFT source prefab  Assets/MorphForge/FullFT/Avatar.prefab
FullFT source scene   Assets/MorphForge/FullFT/MorphForge_FullFT.unity
Diagnostic scene      Assets/MorphForge/Diagnostics/FinalFullFT/
                      MorphForge_FinalFullFT_Diagnostic.unity
```

准备复制操作只更新现有private诊断Avatar，不修改原Stage3 Avatar ID。新场景从FullFT
场景创建独立副本，并将副本中的`PipelineManager.blueprintId`绑定到诊断ID。
后续按受限迁移修复了两个场景的根名，并在最终FullFT Prefab及生成器中将一项
Pickup粒子碰撞力置0；这些修复有独立前后哈希和报告，Nemesis商业源保持不变。
[场景修复](FULLFT-NAME-FIX.md) · [碰撞力修复](FULLFT-COLLISION-FIX.md)。
准备报告中的“源场景仍打开”只是准备时刻的现场快照，不是上传前置条件。短测可用
单场景模式加载诊断场景并关闭其它场景；上传只校验诊断场景、Prefab 和源场景磁盘
文件 hash 的后续一致性，不要求源 FullFT 场景一直保持 loaded。

Avatar 根对象必须被生产同步 profile 的
`FullFTBudgetUtil.IsFullFTTarget` 接受，即名称以
`MorphForge_FullFT` 开头。最终诊断场景副本在准备时会显式写为
`MorphForge_FullFT_Avatar`。仅有 JointBuild 临时 clone 改名不足以让真实场景的
SDK 构建进入 FullFT 门禁。

根名问题前的准备快照（不能直接用于最终上传）：

```text
diagnostic scene      Assets/MorphForge/Diagnostics/FinalFullFT/
                      MorphForge_FinalFullFT_Diagnostic.unity
target scene SHA-256  c90540a76714190e3f4978b65882c995e459ecb982d8d132f8309119fb3ed7de
target Avatar ID      avtr_df2ff222-b6ff-47fb-80be-e060868596a2
source FullFT SHA     335ccf4f416548571b01abf91c6e818c4d71a2c37b5ddd204db63a1466466926
source Prefab SHA     669dd8fe902c60055dcc616d26edbeada8c98a114b900f399d7d2cbb7329e328
```

准备报告：

- [最终 FullFT 预检](final-fullft-preflight.json)
- [独立诊断场景准备报告](final-fullft-prepared.json)
- [生产 JointBuild](fullft-production-joint-build.json)
- [生产最终预算](fullft-production-final-budget.json)

## Helper

文件：`AgentScripts/FinalAvatarPublish.cs`

入口：

```text
FinalAvatarPublish.Inspect
FinalAvatarPublish.InspectRemote
FinalAvatarPublish.InspectAnalysis
FinalAvatarPublish.PrepareDiagnosticScene
FinalAvatarPublish.RefreshPreparedEvidenceAfterRootRename
FinalAvatarPublish.RefreshPreparedEvidenceAfterCollisionForceFix
FinalAvatarPublish.StartUpload
FinalAvatarPublish.GetOperation
FinalAvatarPublish.Cancel
```

准备阶段不会调用 SDK build/upload。`StartUpload` 是唯一网络写入口。

`RefreshPreparedEvidenceAfterRootRename` 是根名修复的一次性、只读证据迁移入口：

- 源 FullFT 场景和诊断场景必须都已保存、无未提交修改，并且各自恰好有一个 scene root
  Avatar。
- 两个 Avatar 都必须通过 `FullFTBudgetUtil.IsFullFTTarget`。
- 诊断 Avatar 仍必须绑定固定诊断 ID。
- 除这两个场景外，先前 Prepared 报告中的所有文件 hash，包括 FullFT Prefab、
  Shader、Material、JointBuild、final-budget、生产迁移与视觉报告，必须完全不变。
- 两个场景文件 hash 都必须实际发生变化，源 Prefab 必须仍与此前 JointBuild 证明一致。
- 原始 Prepared 报告归档为
  `Docs/Security/final-fullft-prepared-pre-root-rename.json`，刷新后的报告写入
  `Docs/Security/final-fullft-prepared.json`。
- 不允许第二次执行；入口本身不保存、关闭或重开任何场景。

## 预检证据

`Inspect` 检查文件、Prefab 状态、预算与材质。它会在以下任一条件
不成立时返回 `success=false`：

- `Docs/FullFT/joint-build.json` 针对当前 `Assets/MorphForge/FullFT/Avatar.prefab`，
  prefab hash、final-budget hash 一致。
- `Docs/FullFT/final-budget.json` 为 `256/256`、剩余 0；FT 161 direct、
  VRCFT controls 8 direct、MF3 geometry 56 direct、placement 14 direct。
- `Docs/FullFT/scene-build.json` 对应当前 FullFT 场景和当前 prefab。
- `Docs/Security/catalog-production-migration.json` 证明生产 catalog 已迁移、数据
  保留且 GUID 保留。
- `Docs/Security/catalog-production-visual/report.json` 至少包含 16 项生产视觉检查且
  全部通过。
- Store 恰好一个，容量 16,384，寿命 Infinity，使用生产
  `ParticleBoxCatalog.mat`。
- 生产材质使用 `ParticleBoxCatalog.shader` / `MorphForge/ParticleBoxCatalog`，
  `_CatalogMeta` 为 `256×64 RGBAFloat`，`_MetaWidth=256`、
  `_MetaHeight=64`，Atlas 为 `384×96`。
- Prefab 中 descriptor、placement 和唯一 `MorphForgeGeometry` 存在，missing
  script 为 0。

`InspectRemote` 额外验证 SDK 账号、诊断 Avatar 所有权、private 状态和 pendingUpload。

## 执行顺序

生产步骤完成后：

```text
1. FullFT JointBuild
2. 生产视觉验证
3. FinalAvatarPublish.Inspect
4. FinalAvatarPublish.InspectRemote
5. 打开已保存、无未提交修改的 MorphForge_FullFT.unity
6. FinalAvatarPublish.PrepareDiagnosticScene
7. 对诊断场景执行必要的视觉/Play 审核
8. FinalAvatarPublish.StartUpload
9. FinalAvatarPublish.GetOperation
10. FinalAvatarPublish.InspectAnalysis，等待 generic/standard/security 三项终态 200
```

`PrepareDiagnosticScene` 会拒绝覆盖已存在的诊断场景，也拒绝保存或丢弃源场景的
未提交修改。`StartUpload` 重新校验：

- 诊断场景文件 hash 与准备报告一致。
- 源 FullFT 场景 hash 与准备报告一致。
- FullFT Prefab hash 与准备报告一致。
- 场景中的唯一 Avatar 仍绑定诊断 ID。
- 当前账号、Avatar 所有者、private 状态和 SDK 空闲状态。

上传后还必须审计实际 VRCA 的容量、寿命、Shader、metadata、Atlas 和真实客户端可加载
性。Editor 预检、SDK build 通过、服务端 security 200 和真实客户端显示是四个独立
结论，不能互相替代。

## 2026-10-19 修复前 validation 失败记录（历史上传前）

真实 SDK build clone 名为：

```text
MorphForge_FullFT_Avatar(Clone)
```

clone 名称没有被人工改写，profile 门禁通过。SDK 实际生成的 11,067,360-byte VRCA
已经直接解析 `VRCExpressionParameters.parameters`，确认：

```text
total synced bits        256
FT manifest              112 entries / 161 bits
VRCFT controls             8 entries /   8 bits
MF3 geometry               7 entries /  56 bits
placement                  7 entries /  14 bits
```

SDK validation 唯一失败项是一个原生 wand 粒子系统：

```text
Armature/Hips/Spine/Chest/Shoulder_R/UpperArm_R/LowerArm_R/
Hand_R/NemesisWandPrefab/WandMenu/LusterRailGun/
LusterRailGun_Toggle/Shot/Coin/Light
colliderForce      1
collidesWith bits  4188439
layers             13 Pickup, 14 PickupNoEnvironment
```

VRChat 要求移除这些 Pickup/Item layer 或把 collider force 设为零。该对象不属于
MorphForge 粒子 Store；生产源修复不能减少 Store 容量或占用同步 bit。当前失败包
证据：

生产修复采用 SDK 官方允许的 `colliderForce = 0`，保留碰撞层配置，因此
`pickupForceDisabled=true`、`disabledPickupLayers=false`。接下来证据刷新只接受
这一项修复，并按以下 SDK 谓词验证：

```text
collision.type == World
&& colliderForce != 0
&& (collidesWith & LayerHelper.Mask_ParticleCollisionForceForbiddenLayerMask) > 0
```

命中该谓词才算违规。Store 与 Pump 分别验证 16384 与 1；其他人物特效系统单独统计，
不把人物总粒子容量当作 MorphForge Store 容量上限。

- [失败摘要](final-fullft-validation-failed.json)
- [实际 VRCA 审计](final-fullft-validation-failed-vrca.json)

该 11 MB 诊断包只完成 SDK build，未上传，不能称为服务端失败。修复报告确认：
只有目标 `Coin/Light` 的 `colliderForce` 从 1 变为 0，碰撞层 mask 4188439 与
`Collision.enabled=true` 均保留，Nemesis 源 Prefab hash 前后不变。

实际包审计使用 [audit_fullft_vrca.py](../../tools/audit_fullft_vrca.py)。它直接读取
VRCA browser 内容，核对：

```text
Store = 16384, +Infinity, size3D
Pump = 1 (verified separately)
other character effects = reported separately, not counted as Store capacity
production shader and material references
_CatalogMeta = 256x64 RGBAFloat
_CatalogAtlas = 384x96
no 16384x1 metadata texture
no MorphForge runtime MonoScript
MF3, placement and FullFT parameter names serialized
```

命令示例：

```bash
python tools/audit_fullft_vrca.py \
  final-fullft.vrca \
  --json-out docs/security-check-failure/final-fullft-vrca.json
```

脚本需要 UnityPy，但只读本地 bundle；不下载、不启动 VRChat、不发 OSC。
