# FullFT 联合构建、场景与烟测证据

更新时间：2026-10-19。以下结果来自 MorphForge Windows Unity 工程的实际
VRChat SDK preprocess/Play 流程，不包含上传或 VRChat 客户端启动。

## 联合构建

```text
finalBits                       256 / 256
FT manifest                     112 / 112
FT represented                 112 / 112
FT bits                        161 / 161
FT direct                      112 / 112
FT physical-direct gate        pass (161 bits)
VRCFT controls direct            8 / 8
MF3 geometry direct              7 / 7, 56 bit
MF placement direct              7 / 7, 14 bit
VRCFury compressor candidates     0
duplicate names                   0
unexpected FT names               0
missing FT names                  0
FT type mismatches                0
```

初始压缩前为 278 bit。先把 `FT/Debug`（1 bit）本地化到 277 bit，再精确本地化
7 个共存本体项（21 bit）到 256 bit，因此 compressor 没有启动：

```text
FaceLock                   1
Fasial                     8
NadeNadeFace               8
AllowShield                1
ColliderJump               1
NadeNadeHeadToggle         1
TailColliderDisable        1
合计                      21
```

关闭的是 `NadeNadeHeadToggle` 启用开关；`NadeNadeHead` 的结果参数仍保持同步。

远端影响：

- 手动 FaceLock/Fasial 选择与结果不同步。
- 盾开关、跳碰撞开关不同步。
- 摸脸、摸头和尾巴碰撞开关不同步。
- FT/v2 高频面捕、8 个 VRCFT 控制、MF3 几何与定位全部保持 direct。

证据：

```text
joint-build.json
final-budget.json
pre-compression.json
profile.json
avatar-build.json
```

## 场景

```text
Assets/MorphForge/FullFT/MorphForge_FullFT.unity
```

```text
sceneAvatarCount                  1
gestureManagerCount               1
receiverCount                     1
receivePort                       9012
sendPort                          9013
cameraCount                       1
lightCount                        7
GM favourite redirected           true
Receiver avatar redirected        true
source SHA unchanged              true
source scene still open           true
source dirty before/after         true / true
source scene saved or discarded   false
```

证据：

```text
scene-build.json
scene-close.json
```

首次场景创建被过严的 MF3 父级字面路径检查拒绝，保留：

```text
scene-build-error.json
```

## Play 烟测

源 Stage3 场景的三个对象仅在 Play 前临时停用，退出后全部恢复：

```text
MorphForge_Stage3_Avatar
MorphForge Stage3 Gesture Manager
MorphForge Test Receiver
```

真实 UDP9012 协议烟测：

```text
interval                 0.5s
commands                 50
checks                   166 / 166
```

证据：

```text
protocol-smoke.json
play-isolation.json
play-isolation-restore.json
```

远场面貌路径：

```text
IsLocal                  0
EyeTrackingActive        1
LipTrackingActive        1
VisemesEnable            1
FT inputs                EyeLeftX, EyeRightX, JawOpen, MouthClosed
changed outputs          18
```

观察到主体 JawOpen、眼部 BlendShape、双眼骨骼和 VRCFT Debug 输出变化。

证据：

```text
ft-remote-smoke.json
```

## 收尾清理

`FullFTBudgetGate` 已增加独立的 `FT is fully direct (161 physical bits)` 检查，
门禁补强后的联合构建再次通过，结果仍为 256/256 且 compressor 候选为 0。该检查
防止未来恢复其它本体同步时，通过串行压缩 FT 参数来表面满足预算。

这次门禁补强属于纯 Editor 验证变更：Avatar Prefab SHA 仍为
`A0C1C4446E8E0C313E030416B5A501675364505395B8A31B2C49848BBD4C21F6`，
没有改变 Avatar、场景或运行时功能；因 `final-budget.json` 新增检查记录，报告文件
SHA 会与场景创建时不同。

8766 测试服务及精确归属的 capture Edge 子进程树：

```text
root PID             126580
owned processes      9
remaining owned      0
port 8766 listener   none
port 8765 listener   none
Unity PID 46280      alive
stale FullFT tasks   none
```

清理只终止了 `target/debug/morphforge.exe serve --port 8766 --data
C:\Users\Chiro\tmp\morphforge\workbench-validation` 及其子进程。正在运行的
`VRChat.exe` 未被触碰。证据：`workbench-cleanup-after.json`。

## 恢复边界

- 未执行 Avatar 上传。
- 未启动 VRChat。
- 源 Stage3 文件 SHA 保持
  `B09F22C2F830ADF876DB8FE2754C6AD0F81221706B71534BA9FA012F82092FED`。
- 源 Stage3 仍为打开且 dirty，未保存、未关闭、未丢弃。
- 新 FullFT 场景在 Play 后保存并关闭，场景文件保留；最终活动场景恢复为源 Stage3。
- 新场景 Avatar 的 missing script 为 0，Avatar 内 MorphForge 运行脚本为空。
