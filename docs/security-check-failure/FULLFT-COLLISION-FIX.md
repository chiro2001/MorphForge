# 完整 SDK 校验中的人物特效碰撞力

2026-10-19，最终 FullFT 的正式 SDK `BuildAndUpload` 在本地 Avatar validation
阶段发现一项人物特效不符合 SDK 校验。该失败发生在网络上传前，不能称为新的服务端拒绝。

真实 SDK build clone 名称为 `MorphForge_FullFT_Avatar(Clone)`，FullFT profile 已
正常执行，预算报告为 256/256，FT161/控制8/几何56/定位14全部直通。这次错误与
此前根名前缀问题不同。

被拒绝的是 Nemesis 自带 RailGun 的 `Shot/Coin/Light` 粒子系统：World collision、
`colliderForce=1`，碰撞层包含受保护的 Pickup 层。SDK 3.10.5 的验证源码明确允许两种
修复：关闭相应碰撞层，或将碰撞力设为0。本项目选择后者。

修复只应用于 `Assets/MorphForge/FullFT/Avatar.prefab` 中的最终副本：

- `colliderForce` 从1改为0。
- 碰撞层掩码4188439及 Collision enabled 保持原值。
- 全部粒子系统序列化设置剔除 `colliderForce` 后逐一比较，未发现其他变化。
- 保存后重新读取 Prefab，确认覆盖持久化。
- Nemesis 原始 Prefab 前后哈希一致；未修改商业源资产。
- `FullFTBuild.BuildTestAvatar` 同样执行该修复，使用当前官方 SDK 的 forbidden
  layer mask及World碰撞条件，避免重新生成后复发。

最新最终 Prefab SHA-256：

```text
a822389956a7bdb38b5cff91b97b42e94d3435dce09ee3f627b94b659e09acfb
```

证据：

- [修复及设置对比](fullft-collision-force-fix.json)
- [修复后 JointBuild](fullft-collision-fixed-joint-build.json)
- [修复后完整预算](fullft-collision-fixed-final-budget.json)
- [修复后无睡眠 Prefab 审计](fullft-collision-fixed-nosleep.json)
- [修复后无睡眠场景审计](fullft-collision-fixed-scene-nosleep.json)

最终人物还包含其它原有特效。粒子容量审计分别验证 MorphForge Store=16384、
Pump=1，再单独统计人物特效；不能把 Stage3 仅有两套粒子的总容量16385强套到
完整 FullFT 人物上。实际上传和服务端结果见[最终验证](FINAL-FULLFT.md)。
