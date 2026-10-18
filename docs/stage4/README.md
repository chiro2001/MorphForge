# Stage4 / MF3 验证记录

日期：2026-10-19。当前协议是 MF3（56 bit），容量为 16,384 Box。旧 Stage1–3 的通道、容量与预算不是本阶段规格。

## 完整真实 UDP 与容量

`check_capacity.py` 在 Windows 本机向 UDP 9012 发送，由单 Avatar Play 场景的接收器输入 Gesture Manager，再由原生 FX、Parameter Driver 和 ParticleSystem 形成持久方块。没有直接向产品显示池写入粒子。

首次完整发送为 16,384 条 UnitVoxel，加一次 Clear 和一次 Brush，标称间隔 0.25 秒，实际用时 4105.53 秒。该轮中途修改了 Pump 寿命，Fire 仍是旧版 1ms，属于混合版本证据：

| 指标 | 结果 |
|---|---:|
| 发送命令数 | 16,386 |
| 接收/完成命令增量 | 16,261 / 16,261 |
| 实际唯一方块 | 16,192 |
| 缺失方块 | 192 |
| 错误位置、尺寸、metadata | 0 |

这不是首遍无损通过。125 条命令没有被动画器捕获；已捕获命令与方块数也有差额。已复现短寿命母粒子在长帧中漏 Birth 的问题，详见 [时序记录](PUMP-TIMING.md)。正式修复后的协议回归单独记在 `../materials/protocol-regression-256.json`，不能用短回归抵消本轮缺失。

`finalize_capacity.py` 保留首遍报告和原生快照后，以 0.5 秒发送 Brush 加 192 条缺失绘制，共 193 条。补发完成后得到 **16,384 个唯一方块，位置、尺寸及 metadata 全部正确**。然后完成：

1. 满池删除 16 个：精确得到 16,368 个，缺口集合一致。
2. 补回这 16 个：恢复 16,384 个，无重复和畸形。
3. 对 10×10×10 区域擦除：精确移除 64 个中心，剩 16,320 个。
4. Clear：原生显示池归零，Busy 与 Inbox 为空。

满容量时采样 432 帧：p50 6.84ms、p95 8.29ms、最大 10.51ms，Editor 总分配约 672.6MB。这是当时 Editor 的整体帧耗时，不是独立粒子成本，也不是 VRChat 客户端 GPU 基准；此时使用的是旧显示材质。

## 证据

- [首遍原始报告](capacity-first-pass.json)
- [完整收尾与回收报告](capacity-validation.json)
- [补发后原生报告](capacity-repair-1.json)
- `capacity-first-pass-particles.bin`、`capacity-repair-1-particles.bin`：原生位置、尺寸、颜色码快照；对应报告中的 SHA-256 已复核。
- [初期主协议回归](protocol-validation-initial.json)：Box、Unit、刷子、覆盖、精确删除、区域擦除、Clear、重复序号与序号回绕。最终时序版本复测另行保留。
- [水平定位测试](../plan/STATUS.md)：包括近竖直 Anchor 与远端抓取 Bool 解码。

## 使用边界

16,384 是实际验证过的原生池容量，不是同步地址空间，也不是任意场景下的帧率承诺。协议仍无远端 ACK，Editor 阻塞、低帧和 VRChat 网络采样均可能丢失中间输入。发送端提供可调间隔、独占通道、持续增量发布及周期全量刷新；“已发送”只能表示本机提交完成。

真实 VRChat、多人显示、VR 手势与客户端 Particle Limiter 仍需用户上传后观察。自动验证没有向 9000 发包，没有上传 Avatar，也没有启动 VRChat。
