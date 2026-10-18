# Stage2：单次提交与随盒体固定的纹理

更新：2026-10-18。本目录对应新版`MF2_`协议及独立Stage2资源。验收结果见[主回归](validation.json)、[速率扩展](extended-validation.json)、[无停顿循环](loop-validation.json)、[队列隔离](queue-validation.json)、[纹理对比](texture-validation.json)和[NDMF产物](export.json)；旧版Stage1的客户端观察不能替代新版验收。

已通过：主回归1404项断言、扩展186项断言、队列隔离、无停顿循环及NDMF检查。连续循环每档0.25s/0.2s分别3轮74条命令，每条完成时的原生数量都正确。纹理7姿态对比最大平均RGB误差约0.000051，旧世界坐标Shader平移负对照约1.4215。见[新旧对照图](texture-comparison.png)及[完整验收](../VALIDATION.md#stage2验收)。

## 工程入口

- 场景：`Assets/MorphForge/Scenes/MorphForge_Stage2.unity`
- Avatar：`Assets/MorphForge/Stage2/Avatar.prefab`
- 显示模块：`Assets/MorphForge/Stage2/Display.prefab`
- 构建器：`Assets/MorphForge/Editor/ParticleStage2Build.cs`
- Shader：`Assets/MorphForge/Shaders/ParticleBoxLocalUV.shader`
- 单次发送：`tools/send_stage2.py`
- 连续发送：`tools/loop_stage2.py`

原Stage1的场景、Prefab、Shader和协议保留。构建Stage2时保留其它已打开场景及其未保存修改；只保存新场景。自动测试只在Play Mode内暂时关闭其它场景的根对象，退出后恢复；不保存这些临时状态。构建器会尝试继承已打开Stage1中的上传ID，最终以Avatar Descriptor上的Pipeline Manager为准。

## API与字段

高层几何命令仍为`box`、`remove`、`erase`、`clear`。盒体布尔相减继续复用外部几何编译器，长方体内部挖洞仍需删除旧盒并绘制碎片。

每次提交发送一个OSC bundle，内容为10个载荷Int和一个控制Int：

| 字段 | 定义 |
| --- | --- |
| `/avatar/parameters/MF2_Ctl` | `Ctl = (seq << 3) | opcode`；低3bit为操作，高5bit为序号0–31 |
| `MF2_D00..02` | min XYZ，0–255 |
| `MF2_D03..05` | size XYZ减一，0–255 |
| `MF2_D06..07` | 样式低6位／高6位 |
| `MF2_D08` | 染色码0–63 |
| `MF2_D09` | 运动预留码0–63，只保存 |

opcode：1=SetBox、2=按中心筛选的底层EraseRegion、3=RemoveBox、4=Clear。0、5、6、7保留。总同步成本仍为11个Int、88bit；扩大序号利用的是原来的阶段位，没有减少Int数量。

`MF2_`与`MF1_`协议不兼容，必须配套新版Avatar和发送器。默认单次发送不重发，也不再发送准备／执行两种控制字；`--repeat`可选重复同一提交，靠最近序号去重。

## 接收器

固定3层、40个Clip：数据输出、输入待执行槽、粒子生命周期。源Int先复制进本地`Q00..09`，执行器再将它复制进独立的`W00..09`。工作缓存只在下一条开始执行时改变；忙时可以保留一条后续命令，输入槽满时不会覆盖其中内容。

执行流程为`Latch → Settle → Erase → Release → Fire（仅绘制）→ Done`。Settle保留0.03秒，覆盖测试环境下动画属性与碰撞器更新所需的等待；其余Clip为0.001秒、实际至少一次动画求值。配置时长合计约0.035秒，实际耗时由帧率和更新顺序决定，须看观测轨迹，不能把配置值当实测速率。

本地`RxCount`、`DoneCount`、`InboxReady`用于观测，均不进入Expression Parameters。单槽不能吸收持续过载；槽满期间，未捕获的中间源参数提交仍可能被后续输入替换。没有网络ACK、跨参数校验、历史重放保护或晚加入重建。一个OSC bundle只证明本机发送打包，不能证明远端跨参数永远原子同步。

## 纹理修复

旧Shader用世界坐标与世界法线选择投影面，Avatar移动/旋转时会改变表面上的采样位置。新Cube预先保存每面的UV及面轴标识；原生粒子流提供出生尺寸，Shader按`面UV × 对应边长 / 0.25m`计算重复次数。

面轴、UV和尺寸都随盒体保留，不使用世界位置或世界法线切换投影。示例明暗也按盒体面轴计算，因此整体转动不会改变纹理布局。UV与面轴预烘焙在共享Cube内，不增加OSC字段。

姿态回归通过跟随Avatar的相机比较相同观察角度，覆盖平移、37度偏航、三轴旋转、0.5/1.8倍缩放及组合姿态；旧世界坐标Shader作为负对照，确认测试能检测原先的纹理游动。

## 发送和验证

Windows工程目录执行，端口9012为Editor，9000为用户已启动且已切换新版Avatar的VRChat：

```powershell
python tools/send_stage2.py --port 9000 --seq 0 --interval 0.25 box --min 128 128 128 --size 20 10 1 --style 2
python tools/loop_stage2.py --port 9000 --interval 0.25
```

连续循环为8柱绘制→8柱换色→反向删除；**每0.25秒一条完整命令**。在Windows用户目录`tmp/morphforge/stage2-loop.stop`创建文件即可停止，状态文件为`stage2-loop-status.json`。独立多次调用发送器时使用上次返回的nextSeq。

Linux项目目录重跑：

```bash
python3 -m unittest discover -s tools -p test_stage2_protocol.py -v
python3 tools/check_stage2.py
python3 tools/check_stage2_extended.py
python3 tools/check_stage2_queue.py
python3 tools/check_stage2_loop.py
python3 tools/unity_remote.py --check-result run_script --file AgentScripts/ParticleStage2Entry.cs --entry ParticleStage2Entry.ValidateExport --timeout_ms 60000
```

默认自动测试使用独立Windows Python程序向GM原生OSC发包；Editor观察器不代写Animator输入、不Emit粒子。速率测试核对接收数、完成数与每一阶段的完整粒子状态。新版真实客户端、多人及边运动边编辑的可靠性仍需单列记录。
