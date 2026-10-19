# FullFT Avatar 无睡眠插件依赖

状态：**通过**。最终 FullFT prefab 不再依赖 `Packages/minminmean.supine`，
SDK 预处理后的同步预算仍为 256 bit。

完整交叉检查见 [no-sleep-audit.json](no-sleep-audit.json)。修复前的问题证据保留在
[no-sleep-audit-before-fix.json](no-sleep-audit-before-fix.json)。

2026-10-19 后续实际上传校验发现人物 RailGun 粒子的 Pickup 碰撞力，已仅在最终
FullFT 副本中置0，并加入生成器。最新 Prefab SHA-256 为
`a822389956a7bdb38b5cff91b97b42e94d3435dce09ee3f627b94b659e09acfb`；
[最新无睡眠审计](../security-check-failure/fullft-collision-fixed-nosleep.json)和
[最新预算](../security-check-failure/fullft-collision-fixed-final-budget.json)仍通过。
下文保留此前无睡眠修复阶段的独立证据。另见[SDK碰撞力修复](../security-check-failure/FULLFT-COLLISION-FIX.md)。

## 最终结果

- 完成无睡眠修复时的 `Assets/MorphForge/FullFT/Avatar.prefab` SHA-256：
  `669dd8fe902c60055dcc616d26edbeada8c98a114b900f399d7d2cbb7329e328`。
- 已保存的 `Assets/MorphForge/FullFT/MorphForge_FullFT.unity` 同样通过只读依赖
  审计：Base 层继承新 prefab 的 `Nemesis_Locomotion_NoSleep.controller`，场景
  递归依赖没有原 Locomotion、`minminmean.supine` 或 Supine 参数。
- 场景内两个 prefab 实例合计 137 条保存覆盖（其中 Avatar 实例 124 条）；这些
  路径中没有 `baseAnimationLayers`、`specialAnimationLayers` 或
  `animatorController`，即没有把 Base 层覆盖回旧控制器的残留。
- 665 个 GameObject、1183 个组件，无缺失脚本。
- 递归依赖 1023 项，`Packages/minminmean.supine` 匹配为 **0**。
- 无 Supine/minminmean MonoBehaviour、Supine 生成器或相关 GameObject。
- 插件专用参数 `Supine`、`VRCSupine`、`VRCSupinePoseAdjust` 均不存在。
- 最终表达式参数中的 `sleep-supine.present` 与 `stillRepresented` 均为空。
- 最终预算仍为 `256 / 256 bit`、剩余 0 bit：面捕 161、VRCFT 控制 8、
  几何 56、定位 14；所有 SDK/VRCFury 预算检查通过。
- 审计前后 `MorphForge_Stage3.unity` 都保持 loaded + dirty + active；没有保存、
  关闭、替换或清理该场景。

保留的 `VRCLockPose`、`VRCFootAnchor`、`VRCFootAnchorHandSwitchable` 属于仍在使用的
crouch/foot-anchor 行为，不是本次插件专用睡眠参数。已保留 Crouching、AFK、
眼动及其他普通动画。

官方 VRChat Avatar 示例包中仍有一项 `proxy_supine_getup.anim`。它是 VRChat 自带的
Action 示例片段，不属于 `minminmean.supine` 包；审计将其单独列出，不计为插件依赖。
工程中仍可保留 `minminmean.supine` 包供 Stage1–3 历史场景使用，但最终 FullFT
资源图不引用该包，上传构建不再要求安装它。

## 生成入口修复

`Assets/MorphForge/Editor/FullFTBuild.cs` 的 `BuildTestAvatar` 现在会自动执行：

```text
复制 Nemesis_Locomotion.controller
  -> Assets/MorphForge/FullFT/Nemesis_Locomotion_NoSleep.controller
删除 Supine 状态、Prepare Supine、插件动画状态及 56 条相关转场
删除 Supine / VRCSupine 参数（确认已无引用）
验证派生控制器不含 Packages/minminmean.supine 依赖
把 FullFT prefab 的 Base layer 0 切换到派生控制器
```

因此重新运行 `FullFTEntry.BuildTestAvatar()` 也不会把原控制器重新带回来。
`JointBuild` 前置检查还会验证 Base 层使用 NoSleep 控制器且 prefab 递归依赖不含
`minminmean.supine`；若有人跳过 BuildTestAvatar，会直接报错而不是继续生成。
本次已经实际从 Base Avatar 重生成一次并再次通过审计。

Unity 在就地删除 Animator 子对象后，序列化文件里可能保留不再可达的孤立子对象。
它们不出现在 `controller.layers`、`parameters` 或 `AssetDatabase.GetDependencies`
中，因此不进入场景/SDK 的动画依赖图；验收以运行时可达图和 SDK 依赖为准。

## 可重复验证

```powershell
unity command recompile
unity command recompile_status

unity command run_script --file AgentScripts/FullFTEntry.cs `
  --entry FullFTEntry.BuildTestAvatar

unity command run_script --file AgentScripts/NoSleepAuditEntry.cs `
  --entry NoSleepAuditEntry.Run
```

`JointBuild` 可能弹出 VRCFury 的 Write Defaults 选择。对隔离构建 clone 选择 `Skip`
即可，不要选 Auto-Fix，也不要保存或关闭用户的 dirty Stage3。

## 证据

- [avatar-build-post-fix.json](avatar-build-post-fix.json)：重生成结果与清理明细。
- [controller-cleanup-post-fix.json](controller-cleanup-post-fix.json)：生成入口返回的
  Base layer 与控制器依赖检查。
- [joint-build-post-fix.json](joint-build-post-fix.json)：SDK 预处理最终 256 bit。
- [pre-compression-post-fix.json](pre-compression-post-fix.json)：压缩前审计。
- [final-budget-post-fix.json](final-budget-post-fix.json)：最终预算与参数明细。
- [fullft-scene-no-sleep-audit.json](fullft-scene-no-sleep-audit.json)：已保存
  FullFT 场景的依赖和 prefab 实例覆盖审计。

本次没有执行 Play Mode、Avatar 上传、VRChat 启动或 OSC 发送。真实客户端表现仍由
后续用户侧上传/多人烟测确认。
