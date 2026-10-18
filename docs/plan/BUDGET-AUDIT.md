# FullFT 同步预算与联合构建审计

更新时间：2026-10-19。实际FullFT SDK联合构建已经通过 **256/256bit**，FT112项161bit、MF3 56bit、定位14bit及8个FT控制全部直通；无需压缩carrier。本文保留审计过程及最终结果，完整证据见 [FullFT验收](../fullft/README.md)。

## 1. 当前事实基线

完整源场景：

```text
D:\Projects\VRChatProjects2\Nemesis\Assets\Main-2.unity
```

其中只有一个 `VRCAvatarDescriptor`，根对象为 `Nemesis_Full_FT`。

完整基础 Prefab：

```text
Assets/Nemesis/Nemesis_Full_FT.prefab
```

该 Prefab 在 Nemesis、Roadster 和 MorphForge 中的 SHA-256 都是：

```text
C4151830701F937773A80CBEEFC1217CA878182728BFFA883D3C1E8F34FD25AB
```

VRCFury 记录来自：

```text
C:\Users\Chiro\AppData\Local\VRCFury\DesktopSyncData
```

| 记录 | 时间 | 同步参数 | 原始 bit | 压缩参数 | carrier | 最终 |
|---|---|---:|---:|---:|---:|---:|
| `avtr_9404e9b5…` | 2026-10-03 01:13 | 163 | 289 | 27 | 1 Int + 4 Bool + 3 Index = 15 | 256 |
| `avtr_cbec5a0e…` | 2026-10-03 14:02 | 172 | 333 | 38 | 3 Int + 11 Bool + 3 Index = 38 | 256 |

`Main-2.unity` 的保存时间、`Packages/com.vrcfury.temp/Builds/Nemesis_Full_FT` 的构建时间和 cbec 记录时间吻合。联合构建以 cbec 为当前基线，9404 只保留作历史对照，不能混算。

## 2. FullFT 保护项

完整面捕固定为：

```text
FT/* 同步参数               112 个
其中 Bool                     105 个
其中 Float                      7 个
合计                          161 bit
```

161 bit 不是只由前缀推导。`FullFTSyncManifest.cs` 固化了 cbec 上传记录中的
112 个最终参数名和类型。与只读记录逐项对照的结果为：

```text
名称集合                   完全一致
类型集合                   完全一致
重复名称                    0
Bool                        105
Float                         7
合计                        161 bit
```

7 个 Float 是：

```text
FT/v2/EyeLeftX
FT/v2/EyeLidLeft
FT/v2/EyeLidRight
FT/v2/EyeRightX
FT/v2/EyeY
FT/v2/JawOpen
FT/v2/MouthClosed
```

`Max-200` 和 `Max-50` 都按这份 manifest 检查。任一预期参数缺失、类型改变、
出现同步的额外 `FT/` 参数，或整张 Expression Parameters 中出现同名重复项，
都会阻止构建。报告会列出 `missing`、`typeMismatches`、`unexpectedSynced` 和
`duplicates`，其中重复项记录名称、数量、类型和默认值。这样即使 Modular Avatar
改名后另一个 FT 参数顶替了数量，也不会仅凭 `112 / 161` 的合计误判通过。

舌头参数全部在这个范围内，包括：

```text
FT/v2/TongueOut
FT/v2/TongueOut1/2/4
FT/v2/TongueX
FT/v2/TongueX1/2/4
FT/v2/TongueXNegative
FT/v2/TongueY
FT/v2/TongueY1/2/4
FT/v2/TongueYNegative
FT/v2/TongueRoll
FT/v2/TongueRoll1/2/4
```

另有 **8 个不带 `FT/` 前缀的 VRCFT 控制 Bool**：

```text
EyeTrackingActive
LipTrackingActive
EyeDilationEnable
VisemesEnable
FacialExpressionsDisabled
FaceTrackingEmulation
FaceTrackingLimits
MixExpressions
```

profile 同时保护：

```text
FT/ 前缀
MF2_ 前缀
MF3_ 前缀
MF_ 前缀
上述 8 个 VRCFT 控制
```

当前 MorphForge 目标几何是 `MF3_` 7 个 Int，共 56 bit；定位是 `MF_` 14 bit。旧 `MF2_` 88 bit 通道保留为兼容保护项，但 FullFT 测试 Avatar 必须使用 Stage4 的 56 bit 几何，不能误装 Stage2 的 88 bit 通道。

## 3. 最小 FT 注入

不复制 `Main-2.unity` 的所有附加系统。FullFT 测试 Avatar 按以下最小集合生成：

```text
Assets/Nemesis/Nemesis_Full_FT.prefab
Assets/MoeWhite/VRCFT/Prefab/VRCFT_UE_Nemesis_VRCFur.prefab
Assets/MoeWhite/VRCFT/Prefab/Optional/MixExpressions.prefab
Assets/MorphForge/Stage4/Placement.prefab
```

`VRCFT_UE_Nemesis_VRCFur` 自带：

```text
UE TongueSteps FullController
EyeLinkEar FullController
Face Tracking 菜单
Eye Rotation 子 Prefab
TongueExt 参数子 Prefab
AutoToggle
FacialLayerControl
```

`MixExpressions` 单独提供第 8 个控制参数及对应图层。

不导入：

```text
睡眠系统
Supine 生成控制器
Ruruku 服装
VRCFury 生成的 com.vrcfury.temp/Builds
基础 Nemesis 模型文件
```

这里没有复制服装，也没有把完整商业模型复制到新目录。基础 Avatar 直接使用 MorphForge 已有的 Nemesis 本地资源。

## 4. 本机最小依赖

Canonical 源工程：

```text
D:\Projects\VRChatProjects2\Nemesis
```

Roadster 中没有完整 VRCFT、睡眠或生成控制器，不能作为完整 FT 的 canonical 源。

运行期最小导入目录：

```text
Assets/MoeWhite/VRCFT/adjerry91.vrcft.templates/**
Assets/MoeWhite/VRCFT/Anim/**
Assets/MoeWhite/VRCFT/EyeLinkEar/**
Assets/MoeWhite/VRCFT/FBX/**
Assets/MoeWhite/VRCFT/Menu/**
Assets/MoeWhite/VRCFT/Prefab/**
Assets/MoeWhite/VRCFT/Runtime/**
Assets/MoeWhite/_Empty.anim
```

所有目录、文件和 `.meta` 必须成对复制。

`MoeWhite/VRCFT/FBX/Nemesis_FT.fbx` 被完整 Avatar 的 `SkinnedMeshRenderer.m_Mesh` 直接引用；缺少它时只能恢复基础 Nemesis，不能得到 FT 形态键网格。

不导入：

```text
Assets/MoeWhite/VRCFT/Editor
Assets/MoeWhite/VRCFT/Patcher
```

两者用于编辑或重打补丁，不是当前最小注入的运行依赖。

包依赖：

```text
com.vrcfury.vrcfury              1.1430.0
nadena.dev.modular-avatar        1.18.7
nadena.dev.ndmf                  1.14.8
com.vrchat.avatars               3.10.5
com.vrchat.base                  3.10.5
```

MorphForge 已安装后三项，当前没有 VRCFury。安装 VRCFury 后，最小 FullFT injection 才可运行。

## 5. GUID 冲突策略

只读扫描结果：待导入的 VRCFT、VGC、Supine 和服装候选目录与 MorphForge 当前工程同 GUID 冲突数为 0。

正式导入时仍须对每个文件执行：

```text
source path
source GUID
target path
target GUID owner
source SHA-256
target SHA-256
action = add/reused/conflict
```

规则：

1. 同 GUID、同目标路径、同内容：复用。
2. 同 GUID、不同路径：阻塞。
3. 同路径、不同内容：阻塞。
4. 源目录内部同 GUID 多路径：阻塞。
5. 导入中断后重新执行：只会复用已一致的文件。

不能把 VRCFury 的生成目录作为依赖来源。必须安装源包并从 Avatar 重新构建。

## 6. 独立同步 profile

profile 资产：

```text
Assets/MorphForge/FullFT/FullFT_SyncProfile.asset
```

它只对名称以 `MorphForge_FullFT` 开头的 Build Clone 生效。源场景、源 Prefab、原始 Expression Parameters 不会被修改。

关闭组：

| 组 | 参数 | 原始成本 | 远端影响 |
|---|---|---:|---|
| 脚趾 | 4 个 `Ashiyubi` Float + 4 个 Bool | 36 | 远端不再看到脚趾轴和控制位；脚趾 PhysBone/动画可能回落默认 |
| 魔杖 | 16 个 `Nem/*`，含两个 Int | 30 | 远端不再看到拿取、装填、选择、射击和特效状态 |
| 睡眠 | 7 个睡眠/Supine 参数 | 35 | 远端不再看到睡眠、仰卧、蹲姿和高度调整；最小 Avatar 中该组通常不存在 |
| 旧预设选择 | `GestureNum`、`FasialSetNumGlobal` | 16 | 远端不再收到旧表情预设选择 |
| FullFT 共存精简 | `FaceLock` 1、`Fasial` 8、`NadeNadeFace` 8、`AllowShield`/`ColliderJump`/`NadeNadeHeadToggle`/`TailColliderDisable` 各 1 | 21 | 远端不再看到手动锁定/旧表情结果、盾开关、跳碰撞开关、摸脸/摸头状态和尾巴碰撞开关；高频 FT/v2、8 控制、MF3 和定位保持 direct |

未列入 profile 的参数默认保持原来的 `networkSynced`。因此：

```text
Inner
IsAFKFinished
NadeNadeHead
Shirt
Socks
```

以及其他未知本体参数不会被自动关闭。

profile 参数：

```text
expectedFtSyncedBits = 161
expectedFtSyncedParameters = 112
expectedVrcftControlParameters = 8
expectedGeometryBits = 56
expectedPlacementBits = 14
maxFinalBits = 256
preserveUnknownSync = true
requireExactFtManifest = true
forcePlacementDirect = true
forceGeometryDirect = true
requireTestedVrcFuryVersion = true
testedVrcFuryVersion = 1.1430.0
```

Profile schema 已升到 2。`forcePlacementDirect` 保护
`MF_World/Handle/Scale/Reset/GrabL/GrabR/Level` 这 7 个同步参数，共 14 bit；
`forceGeometryDirect` 保护 `MF3_Ctl` 和 `MF3_D00..05` 这 7 个 Int，共 56 bit。
本地 `MF_Grab` 是不同步的 Int，不占网络预算。

Stage4 当前菜单中的定位参数压缩风险如下：

| 参数 | 类型/bit | 菜单控件 | 默认是否会被 VRCFury 压缩 |
|---|---|---:|---:|
| `MF_World` | Bool / 1 | Toggle | 是 |
| `MF_Handle` | Bool / 1 | Toggle | 是 |
| `MF_Scale` | Float / 8 | Radial | 是 |
| `MF_Level` | Bool / 1 | Toggle | 是 |
| `MF_Reset` | Bool / 1 | Button | 否 |
| `MF_GrabL` | Bool / 1 | 无菜单项 | 否 |
| `MF_GrabR` | Bool / 1 | 无菜单项 | 否 |

因此实际暴露在压缩器候选中的是 11 / 14 bit，其中 `MF_Scale` 一个参数就占
8 bit。强制直连仍覆盖全部 7 个参数，因为后续菜单或控制器改动不应悄悄改变这个
预算结论。

对 cbec 上传记录的本地只读对照已经确认：

```text
FT/* 同步项                112
FT 成本                    161 bit
8 个 VRCFT 控制           全部存在，合计 8 bit
脚趾组                     8 项 / 36 bit
魔杖组                    16 项 / 30 bit
睡眠组                     7 项 / 35 bit
旧预设选择组               2 项 / 16 bit
```

四组全部存在时为 117 bit；最小 FullFT Avatar 不导入睡眠，因此通常实际关闭 82 bit。

## 7. 覆盖时机

源码确认的实际 SDK callback 顺序：

```text
NDMF Transforming                         -11000
VRCFury VrcPreuploadHook                  -10000
NDMF Optimizing                            -1025
VRCFury editor-only removal                -1024
FullFTSyncOverride                 int.MaxValue - 200
VRCFury Parameter Compressor       int.MaxValue - 100
FullFTBudgetGate                    int.MaxValue - 50
VRCFury FailureCheckEnd                int.MaxValue
```

`ParamsService` 确实缓存 `_params`，但 VRCFury 压缩前执行：

```csharp
injector.GetService<ParamsService>().ClearCache();
injector.GetService<ParameterCompressorService>().Apply();
```

压缩器随后重新读取 `VRCAvatarDescriptor.expressionParameters`。因此 `Max-200` 修改 Build Clone 的参数资产是可靠的；不通过反射修改 `ParamManager`。

覆盖使用：

```text
SerializedObject(VRCExpressionParameters)
parameters.Array.data[i].networkSynced
ApplyModifiedPropertiesWithoutUndo()
EditorUtility.SetDirty(parameters)
```

不调用 `AssetDatabase.SaveAssets()`。修改留给 VRCFury/NDMF 在临时构建资产中序列化，避免把临时策略写回任何源资产。

`Max-200` 还会拒绝以下情况：

```text
profile 缺失或 schema 不匹配
Avatar 没有 Expression Parameters
已存在 Parameter Compressor 层
保护参数被列入关闭组
8 控制参数缺失
参数重名
FT 名称或类型与 112 项 manifest 不一致
FT 不是 161 bit / 112 个表示项
MF3_ 不是预期的 7 个 Int / 56 bit
MF_ 同步项不是预期的 7 项 / 14 bit
VRCFury 版本不是测试过的 1.1430.0
```

### 7.1 VRCFury 压缩器的规避路径

只读检查 VRCFury 1.1430.0 源码确认，压缩器没有按参数名前缀、Prefab、
Modular Avatar 组件或自定义 tag 排除参数的机制。`MenuService.GetReadOnlyMenu()`
每次直接返回 Avatar 当前的菜单，没有缓存副本：

```csharp
public VRCExpressionsMenu GetReadOnlyMenu() {
    return VRCAvatarUtils.GetAvatarMenu(avatar);
}
```

压缩候选必须同时满足“同步”“出现在最终菜单”且菜单类型属于
`RadialPuppet`、`Toggle`、`TwoAxisPuppet` 或 `FourAxisPuppet`。`Button` 和
`SubMenu` 不在候选类型中。因此 `Max-200` 在完成所有校验后执行：

```text
遍历最终菜单所有嵌套菜单
找出引用待强制直连参数的全部控件
记录原始 ControlType
临时把控件类型改为 Button
保留原参数列表、标签和菜单引用
让 VRCFury 1.1430.0 的 Parameter Compressor 运行
```

`Max-50` 从按 Build Clone Instance ID 保存的内存计划中取回这些控件，将原始
`ControlType` 全部恢复，再检查每个定位和几何参数是否仍为直接同步。恢复失败、
计划缺失或任一参数被压缩都会使构建失败。该机制只改内存中的 Build Clone，
不改源 Prefab、菜单资产或 profile，也不调用 `AssetDatabase.SaveAssets()`。
如果最终的 `VRCExpressionsMenu` 仍是磁盘上的持久资产，`Max-200` 会直接拒绝
修改，而不是冒风险写回源菜单；联合构建必须由 VRCFury/Modular Avatar 提供内存中的
最终菜单。

这是依赖 VRCFury 当前候选规则的规避路径，不是公开的 no-compress API。版本门禁
因此固定为 `1.1430.0`；升级 VRCFury 后必须重新审查
`ParameterCompressorSolverService`，再调整 `testedVrcFuryVersion`。

## 8. 最终预算入口

`Max-50` 在 VRCFury 压缩之后运行。它使用：

```csharp
parameters.CalcTotalCost()
```

以及最终 Action Controller 中名为 `Parameter Compressor` 的层，识别：

```text
SyncDataNum*
SyncDataBool*
SyncIndex*
state title 中列出的原始参数名
```

一个受保护参数满足以下任一条件才通过：

```text
最终 networkSynced == true
或者
出现在压缩批次映射中
```

这避免把“被压缩后原参数 `networkSynced=false`”误判为丢失。

最终门禁同时检查：

```text
总 bit <= 256（检测到 DexProtect 时为 237）
Expression Parameters 没有重名
FT 的 112 个名称和类型与 manifest 完全一致
FT 表示项 = 112，成本 = 161 bit
FT 的 112 项必须全部 direct（161 bit 物理直通，不允许被 compressor 串行化）
8 控制存在、类型为 Bool 且全部被直接同步或压缩
MF3_ 的 7 个 Int 表示成本 = 56 bit，强制直连时必须全部 direct
MF_ 的 7 个同步项表示成本 = 14 bit，强制直连时必须全部 direct
临时改成的 Button 菜单控件已经恢复原始类型
关闭组没有任何参数继续被直接同步或压缩
未知参数没有被主动修改
```

`pre-compression.json` 和 `final-budget.json` 会为每个 FT、定位和 geometry
参数记录 `direct`、`compressed` 或 `missing`，并分别列出直接同步和压缩表示的
数量与 bit。最终预算不再是“所有 `FT/` 前缀加起来恰好等于 161”这一种弱校验。

报告：

```text
Docs/FullFT/pre-compression.json
Docs/FullFT/final-budget.json
Docs/FullFT/pre-compression-error.json
Docs/FullFT/final-budget-error.json
```

## 9. 最终联合构建结果

2026-10-19 在 MorphForge 的实际 VRChat SDK preprocess 链得到：

```text
最终总量                    256 / 256 bit
FT manifest                 112 / 112
FT represented             112 / 112
FT bits                    161 / 161
FT direct                  112 / 112
FT physical-direct gate    pass（独立检查 allDirect）
VRCFT controls direct        8 / 8
MF3 geometry direct          7 / 7, 56 bit
MF placement direct          7 / 7, 14 bit
VRCFury compressor batch      0（没有候选进入压缩）
duplicate names               0
unexpected FT names           0
missing FT names              0
FT type mismatches            0
```

最终门禁补强后已再次执行联合构建，仍为 256/256、compressor candidate 0，报告包含：

```text
FT is fully direct (161 physical bits) / pass
```

该门禁保证以后即使恢复其它本体同步，也不能靠压缩 FT 面捕输出“通过”预算。

初始压缩前总成本为 **278 bit**。先把 `FT/Debug` 改成本地，得到 **277 bit**；
再把 7 个共存本体控制精确本地化 21 bit：

```text
FaceLock                 1 bit
Fasial                   8 bit
NadeNadeFace             8 bit
AllowShield              1 bit
ColliderJump             1 bit
NadeNadeHeadToggle       1 bit
TailColliderDisable      1 bit
合计                    21 bit
```

`278 - 1（FT/Debug） - 21（共存本体） = 256`，因此 VRCFury 不再创建
`SyncData*` / `SyncIndex*` carrier，
也不会把任何 FT 或 MF 参数串行化。以上是实际报告，不是理论估算。

明确保留：

```text
所有 112 个 FT/v2 与 FT 管理面捕输出，161 bit direct
8 个 VRCFT 控制，8 bit direct
MF3 几何，56 bit direct
MF 定位，14 bit direct
```

明确本地化的远端影响：

```text
手动 FaceLock/Fasial 选择与结果不跨客户端同步
盾与跳碰撞开关不跨客户端同步
摸脸、摸头开关与尾巴碰撞开关不跨客户端同步
```

这里关闭的是 `NadeNadeHeadToggle` 启用开关；`NadeNadeHead` 的结果参数仍保持同步。

这几项不参与 VRCFT 的 112 项面捕输出，也不承担 MorphForge MF3 通信。

## 10. 执行记录

已编写：

```text
Assets/MorphForge/Editor/FullFTSyncProfile.cs
Assets/MorphForge/Editor/FullFTBudgetUtil.cs
Assets/MorphForge/Editor/FullFTSyncOverride.cs
Assets/MorphForge/Editor/FullFTBudgetGate.cs
Assets/MorphForge/Editor/FullFTDependencies.cs
Assets/MorphForge/Editor/FullFTBuild.cs
AgentScripts/FullFTEntry.cs
```

入口：

```text
FullFTEntry.Inspect
FullFTEntry.DependencyPlan
FullFTEntry.InstallVrcFury
FullFTEntry.ImportDependencies
FullFTEntry.EnsureProfile
FullFTEntry.ResetProfile
FullFTEntry.BuildTestAvatar
FullFTEntry.JointBuild
FullFTEntry.CreateScene
```

实际执行顺序：

```text
DependencyPlan
Install VRCFury 1.1430.0
ImportDependencies
EnsureProfile
BuildTestAvatar
JointBuild
CreateScene
```

`JointBuild` 在临时 Clone 上调用：

```csharp
VRCBuildPipelineCallbacks.OnPreprocessAvatar(clone)
```

它实际执行 MA/NDMF、VRCFury、本 profile、VRCFury 压缩和最终门禁，不执行上传。`final-budget.json` 的 `success=true` 才算联合预算通过。

已完成以下验证：

```text
cbec 记录 112 项 manifest 名称/类型逐项一致
FullFT/Entry 源文件通过项目真实编译参考集语义检查，错误数 0
VRCFury 1.1430.0 压缩器候选类型和 MenuService 读取路径已只读核对
实际 SDK preprocess final-budget.json success=true
实际最终 256 bit 且 FT/MF/controls 全部 direct
```

Button 临时覆盖、菜单恢复、压缩候选排除和最终结果均由实际构建报告验证。
测试期间 CreateScene 首次因过严的 MF3 父级字面路径检查失败；修正为“Avatar 内
恰好一个 MorphForgeGeometry”后成功。失败报告保留在
`Docs/FullFT/scene-build-error.json`，不覆盖成功证据。

## 11. 验收边界

联合构建通过只能证明 Editor 中的 SDK preprocess 产物满足预算和参数保护。完整 FT 视觉、VRCFT OSC 驱动、远端同步、舌头材质和真实客户端上传仍需后续验证。

如果 `Max-200` 或 `Max-50` 任一报告失败，禁止上传。不得用公式结果替代最终报告，也不得为了凑 256 bit 关闭未知参数。
