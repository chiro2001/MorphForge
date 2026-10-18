# FullFT 审核场景入口

更新时间：2026-10-19。本文描述已经实际执行的 FullFT 审核场景入口、Play 隔离和
恢复结果。

## 调用顺序

```text
FullFTEntry.BuildTestAvatar
FullFTEntry.JointBuild
FullFTEntry.CreateScene
FullFTPlaySmokeEntry.Prepare
FullFTPlaySmokeEntry.SmokeFacialTracking
FullFTPlaySmokeEntry.Restore
```

`CreateScene` 只接受与最近一次成功 `JointBuild` 对应的当前 `Avatar.prefab`：

```text
JointBuild 报告 success=true
报告中的 avatar = Assets/MorphForge/FullFT/Avatar.prefab
报告中的 prefabSha256 = 当前 Prefab 文件 SHA-256
Docs/FullFT/final-budget.json success=true
final-budget 文件仍与 JointBuild 记录的哈希一致
```

任一条件不满足都会返回 `skipped=true`，且不会创建场景。

## 源场景保护

入口把当前活动场景作为唯一模板，并要求：

```text
Edit Mode、无编译
活动场景已有磁盘路径
活动场景位于项目的 Assets 下且文件存在
恰好 1 个 VRCAvatarDescriptor
恰好 1 个 GestureManager
恰好 1 个 MorphForgePlayReceiver
至少 1 个 Camera 和 1 个 Light
GM 与 Receiver 不在旧 Avatar 层级内部
```

活动源场景允许处于 dirty 状态。入口通过 save-as-copy 捕获其当前内存状态，并在
报告中记录 `capturedUnsavedSource=true`。入口不会调用源场景的保存，也不会关闭、
替换或丢弃用户打开的源场景。若 Unity 的 save-as-copy 改变了源场景 dirty 状态，
入口会使用 `EditorSceneManager.MarkSceneDirty` 恢复原状态。

随后使用：

```csharp
EditorSceneManager.SaveScene(sourceScene, tempScenePath, true)
EditorSceneManager.OpenScene(tempScenePath, OpenSceneMode.Additive)
```

创建独立副本。所有替换和引用重定向只发生在该副本及最终新场景中。完成后还会比较
源场景文件创建前后的 SHA-256，并确认原场景仍打开且保持原有的 dirty 状态。

## 场景替换

在副本中：

1. 记录旧 Avatar 的世界 position、rotation、lossyScale，同时记录父级、本层顺序、
   local pose、tag、layer、active 状态。
2. 删除副本里的旧 Avatar。
3. 通过 `PrefabUtility.InstantiatePrefab` 实例化 `FullFT/Avatar.prefab`。
4. 恢复到原父级和层级顺序，并应用原 local pose。
5. 验证新 Avatar 的世界 pose 与旧 Avatar 一致。
6. 把 `MorphForgePlayReceiver.Avatar` 和 `GestureManager.settings.favourite`
   重定向到新 Avatar。
7. 设置接收端口 9012、发送端口 9013、AutoStart=true。

最终场景必须满足：

```text
sceneAvatarCount = 1
gestureManagerCount = 1
receiverCount = 1
Avatar 是 FullFT/Avatar.prefab 的 Prefab 实例
Avatar 内恰好有一个 MorphForgeGeometry 模块
GM favourite 与 Receiver Avatar 都引用唯一的 FullFT Avatar
receivePort = 9012
```

源场景的相机、灯光、GM settings、Receiver 对象和其它非 Avatar 内容都由副本保留。

## 输出与失败清理

成功目标：

```text
Assets/MorphForge/FullFT/MorphForge_FullFT.unity
Docs/FullFT/scene-build.json
```

目标场景已存在时不会覆盖，返回 `target-scene-exists`。临时场景只在验证通过后通过
`AssetDatabase.MoveAsset` 移动到最终路径。若创建或最终复核失败：

```text
删除临时场景
删除本次刚创建但尚未最终验证的目标场景
重新激活原源场景
写 Docs/FullFT/scene-build-error.json
保留可用 Avatar.prefab
```

`CreateScene` 不执行上传、不启动 VRChat，也不改变 OSC 发送目标。

## 实际场景结果

```text
targetScene                Assets/MorphForge/FullFT/MorphForge_FullFT.unity
sceneAvatarCount           1
gestureManagerCount        1
receiverCount              1
receivePort                9012
sendPort                   9013
cameraCount                1
lightCount                 7
GM favourite redirected    true
Receiver avatar redirected true
source scene SHA unchanged true
source scene still open    true
source scene dirty before  true
source scene dirty after   true
source scene saved/discarded false
```

报告：

```text
docs/fullft/scene-build.json
```

第一次执行在过严的 `ProtocolCenter/MorphForgeGeometry` 字面父级检查处失败，错误报告
保留在 `docs/fullft/scene-build-error.json`。资产和预算均未受该检查影响；改为校验
Avatar 内唯一 `MorphForgeGeometry` 后成功创建场景。

## Play 隔离与实际烟测

`FullFTPlaySmokeEntry.Prepare` 只记录并临时停用其它已打开场景中的 Avatar、GM 和
Receiver。本次实际停用源 Stage3 的三个根对象：

```text
MorphForge_Stage3_Avatar
MorphForge Stage3 Gesture Manager
MorphForge Test Receiver
```

新 FullFT 场景保持唯一 active Avatar/GM/Receiver，Play 中自动接收器：

```text
Running=true
ReceivePort=9012
SendPort=9013
Error=null
```

真实 UDP 协议烟测：

```text
docs/fullft/protocol-smoke.json
interval=0.5s
commands=50
checks=166/166
```

覆盖了 Clear、Brush、Box、Unit Source Size 强制、覆盖去重、重复序号忽略、
Remove、区域 Erase、Clear 保留 Brush 和 40 次序号回绕。

远场面捕路径烟测：

```text
docs/fullft/ft-remote-smoke.json
IsLocal=0
EyeTrackingActive=1
LipTrackingActive=1
VisemesEnable=1
FT/v2/EyeLeftX
FT/v2/EyeRightX
FT/v2/JawOpen
FT/v2/MouthClosed
changed outputs=18
```

实际观察到 `Body:JawOpen`、眼部位置 BlendShape、双眼骨骼旋转和 VRCFT Debug 输出
变化，证明非本地面孔求值不会依赖被压缩的 FT 网络参数。

退出 Play 后：

```text
disabled objects restored      3
source scene SHA unchanged    true
source scene still open       true
source scene dirty preserved  true
target scene avatar count     1
target scene GM count         1
target scene receiver count   1
avatar missing scripts        0
MorphForge runtime scripts    []
```

报告：

```text
docs/fullft/play-isolation.json
docs/fullft/play-isolation-restore.json
```

Play 结束后，新 FullFT 场景已保存并关闭，场景文件继续保留，可直接再次打开、审核或
后续手动上传。最终 Editor 只保留 dirty 源 Stage3 场景为 active；源文件 SHA 仍是
`B09F22C2F830ADF876DB8FE2754C6AD0F81221706B71534BA9FA012F82092FED`。
关闭结果见 `docs/fullft/scene-close.json`。
