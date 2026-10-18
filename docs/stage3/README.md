# Stage3：底部中心定位球

> 本页记录历史Stage3/MF2实现。用户同名Stage3场景现已承载Stage4模块；当前MF3与FullFT入口见 [实施状态](../plan/STATUS.md)。旧Prefab与旧通道规格不能视为新版配置。

2026-10-19。在 Stage2/MF2 原生粒子接收器之外增加定位模块，不改变 MF2 的 10 个数据 Int 和单次提交格式。

## 使用入口

Windows 工程 `D:\Projects\VRChatProjects2\MorphForge`：

- 场景：`Assets/MorphForge/Scenes/MorphForge_Stage3.unity`
- 本次完整 Avatar：`Assets/MorphForge/Stage3/Avatar.prefab`
- 可复用 MA 模块：`Assets/MorphForge/Stage3/Placement.prefab`
- 菜单：`MorphForge`，由 MA Menu Installer 注入。

需要上传本次 Stage3 Avatar 才能在 VRChat 使用定位功能。完整 Avatar 保留既有 Blueprint；上传仍由用户操作。模块包含原生粒子显示，不应再与旧 Stage2 Display 重复挂载。

金色半透明发光球直径 9cm，是体素空间的底部中心，不随体素缩放一起缩小。默认停放在世界中，初始位置在 Avatar 面前、约双手高度；菜单可改为跟随 Avatar。本人用手接近金球（检测半径 13cm）并握拳拿起，张开该手释放；握住后不要求持续处于触发器内。左右手菜单跟随供桌面模式使用。“移到面前”解除持握并回到当前 Avatar 前方。

## 参数与预算

| 参数 | 类型／新增同步 bit | 默认值 | 用途 |
| --- | --- | --- | --- |
| `MF_World` | Bool／1 | true | 无持握时世界固定；false 跟随 Avatar，切换时保留当前姿态偏移 |
| `MF_Handle` | Bool／1 | true | 定位球与接近检测；不隐藏体素 |
| `MF_Scale` | Float／8 | 1 | 体素内容统一缩放，当前限制 0.05–1.0 |
| `MF_Reset` | Bool／1 | false | 按钮脉冲，释放并移到面前 |
| `MF_Grab` | Int／8 | 0 | 0 放下、1 左手、2 右手；由本人检测结果同步给观察者 |
| `MF_NearL/R` | 本地 Bool／0 | false | 原生 Contact 检测，仅本人允许自接触 |

定位额外 19 bit；NDMF 合并后整套模型 196/256 bit，余 60。定位控制器固定 4 层，加 MF2 的 3 层共新增 7 层；数量不随盒体数增长。位预算包含原模型功能，换模型需重新验证。

## 坐标与原生实现

完整空间仍是 256³、单位长度 0.25m。线上的 X/Z 128 对应定位球中心，Y=0 对应底面：

```
localCenter = ((minX + width/2 - 128),
               (minY + height/2),
               (minZ + depth/2 - 128)) * 0.25
worldCenter = anchor.TransformPoint(localCenter * MF_Scale)
```

例如 `[128,0,128] size=[1,1,1]` 的中心在金球本地 `[0.125,0.125,0.125]`；球不是单个方块中心。手部旋转会旋转整个体素空间。

MA World Fixed Object / World Scale Object 生成原生约束参考空间。主 `VRCParentConstraint` 在 Avatar Dock、左手、右手三路切换；FreezeToWorld 冻结当前姿态，RebakeOffsetsWhenUnfrozen 在重新跟随时保留偏移。抓取前短暂冻结后切源；放下先冻结再切回 Avatar 源。初始化先完成 Dock 定位再冻结，避免出生时锁到错误地点。

Reset 使用独立的原生 `ResetGuide` 定位约束，并经历重新激活／捕获两步。本轮实测发现：动画直接驱动主约束 offset，会被混合 Write Defaults 持续覆盖，导致抓取跳位；仅禁用再启用冻结约束，也会恢复旧冻结缓存。因此不动画 offset，不以自定义运行脚本计算抓放。

粒子 Store 使用 Hierarchy scaling。1/0.25/0.05 比例的实际 BakeMesh 和随动相机截图验证几何、每面格纹理缩放；Local 模式作为反例会使渲染尺寸不随父级等比例缩小。`ParticleSystemRenderer.bounds` 在当前自定义空间组合中报告的数值与渲染几何不一致，不能当作尺寸真值；极限视锥边缘裁剪与长期性能仍未专项测试。

## 验证范围与待测项

证据：`placement-validation.json`、`grip-validation.json`、`scale-validation.json`、`export.json`；网页联调见 [编辑器](../editor/README.md)。

验证采用 Unity Play Mode + Gesture Manager、原生 Contact Sender 输入、真实 UDP OSC 和原生 SDK 约束；测试的临时 Sender 不进入上传资产。Avatar 上没有 MorphForge 自定义运行脚本。

真实 VR 手柄触发范围、动作舒适度、客户端约束求解、多人显示仍需用户上传 Stage3 后确认。`MF_Grab` 和开关同步不等于发送了世界坐标：各客户端根据各自看到的 Avatar／手部姿态求解，放手时刻与网络插值可能带来偏差。没有精确全局 pose 通道、晚加入世界坐标恢复、切换 Avatar 后的位置持久化。当前菜单只允许本人抓取。

重跑：`python3 tools/check_placement.py`；测试会进入 Stage3 Play Mode 并保留接收器便于继续联调，应在结束时停止 OSC、退出本轮 Play Mode。测试脚本不应接管用户已有 Play 会话。构建与合并检查通过项目 Pipeline wrapper 执行 `PlacementEntry.Build` / `PlacementEntry.ValidateExport`。`RebuildGenerated` / `ReadyForReview` 仅用于清理本次自动生成 Stage3 场景的临时测试状态，不得用于丢弃用户新编辑。

收尾在 Scene 视图中临时隐藏保留的 Stage1/2 根物体，避免观察地面重叠；未更改其 Active 状态或保存其未保存内容。可在 Hierarchy 的眼睛图标恢复可见。Stage3 已选中并完成取景，见 `observation-scene.png`。
