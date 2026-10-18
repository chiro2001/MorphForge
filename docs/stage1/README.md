# 第一阶段：OSC到体素／长方体显示

统一设计见[DESIGN](../DESIGN.md)，验收状态见[VALIDATION](../VALIDATION.md)。本目录的`validation.json`是本轮完整回归结果，`compiled-validation.json`是自动拆分命令的集成结果；其它早期调查文件不能替代它们。

2026-10-18实测通过：主回归1371项断言（130条外部OSC命令）、自动拆分集成14项断言、[几何编译器5项单测](geometry-validation.json)（含500个随机相减样本），以及NDMF/MA产物检查。主回归图像平均RGB绝对误差均小于0.002，六方向参考图误差为0。见[截图总览](overview.png)。

## 工程入口

Windows工程：`D:\Projects\VRChatProjects2\MorphForge`。

- 场景：`Assets/MorphForge/Scenes/MorphForge_Stage1.unity`
- 独立显示模块：`Assets/MorphForge/Stage1/Display.prefab`
- 测试Avatar：`Assets/MorphForge/Stage1/Avatar.prefab`
- 生成器：`Assets/MorphForge/Editor/ParticleStage1Build.cs`
- Shader：`Assets/MorphForge/Shaders/ParticleBox.shader`
- 外部发送：`tools/send_stage1.py`
- 区域删除／覆盖编译：`tools/compile_stage1.py`

所有序列化资源通过Unity API构建，原Nemesis与旧Probe14资源保留。Avatar自身不携带MorphForge自定义运行时脚本；Editor Harness只读观察运行结果、启停GM的原生OSC和渲染参考图。

## 接收与保存

独立Python进程发OSC Int，GM原生接收后进入Avatar实际FX。Parameter Driver将源Int复制至本地Float工作缓存，固定规模Direct BlendTree调整发射位置、出生尺寸、出生RGBA。一个短寿命Pump的Birth子发射器向同级Store出生一颗Mesh粒子。

Store使用普通模式与无限出生寿命。只停用Pump，不Stop/Clear子发射器；新参数只影响后续出生，旧粒子的中心、尺寸和RGBA保留。Mesh使用Local对齐；Unity 2022通过Renderer的SerializedObject关闭`m_ApplyActiveColorSpace`，避免把RGBA数据码当颜色转换。现有FX为2层、39个Clip，没有按显示槽位展开Animator。

一颗粒子表示一个盒体，单位块就是1×1×1盒体。空间为256³逻辑格，格边长0.25m，每轴64m；首测容量64个盒体。逻辑地址范围与粒子容量是两个独立限制。

原生Trigger Kill按中心点删除整颗粒子；不会裁剪盒体内部。高层区域擦除由外部编译器查相交盒体，Remove旧盒，再Set剩余碎片。它也处理新盒覆盖旧内容，保持输出盒体互不重叠；超容量在发出前拒绝。

## 测试接口

UDP输入`9012`，OSC路径`/avatar/parameters/MF1_*`。完整字段表见[设计文档](../DESIGN.md#第一阶段实际接收器)。本协议为第一阶段专用，不是最终可配置共享通道协议。

| 命令 | 作用 |
| --- | --- |
| `box` | SetBox，中心擦除后出生；提供min、size、styleId/tintId/motionPhase |
| `remove` | 提供旧min/size，在旧中心删除整个盒体 |
| `erase`（底层发送器） | 删除中心落在区域内的粒子，适合单位粒子阵列 |
| `erase`（高层编译器） | 对所有相交盒体作体素布尔删除，自动拆分 |
| `clear` | 清空整个可寻址显示范围 |

发送器默认准备0.25s、执行0.6s；它在保持期间重复UDP状态。这个值只用于功能验证。`seq`使用上次输出的`nextSeq`；连续独立运行命令时不要一直使用默认0。当前只有最近序号去重与批次发送锁，没有多人ACK、丢包恢复或持久化单写入调度。

## Scene和Game观察环境

Stage1场景已参考SDK的`Robot Avatar/Avatar Dynamics Robot Avatar PC.unity`配置持久保存的Main Camera与Directional Light，另有20米地面、世界原点及显示原点的1米XYZ彩色轴。环境对象在Avatar之外，不参与Avatar构建。后续Build也会自动配置此环境。

主相机使用60度透视、普通可见图层；现场演示不再将Store临时切到31层并仅由Game相机观察。Scene应打开Gizmos及粒子显示；`ParticleStage1Scene.Reveal`可选中运行中的Store并将视角定位到测试区域，也可以在Hierarchy选中Store按F。

2026-10-18现场检查发现，先前Scene视角中心在约(-12.23,-5.17,-9.57)，Gizmos关闭，而方块在另一侧；已有相机因Gizmos关闭而没有图标。修正后[Scene截图](observation-scene.png)和[Game截图](observation-game.png)均显示同一组8级阶梯，八项[观察环境检查](observation-validation.json)通过。

当前Avatar显示模块世界原点为(1,1,0)，单位格0.25米。阶梯整体包围范围为(1,1,0)到(5,3,0.5)，8个中心从(1.25,1.125,0.25)到(4.75,2,0.25)，均与OSC输入换算一致。因此底部离世界地面1米是模块Y偏移的预期效果。粒子保存在单个Store中，不会在Hierarchy下生成8个立方体GameObject；退出Play Mode后现场粒子消失，下一次需要由OSC重建。

## 手动演示

在Unity打开Stage1场景、进入Play Mode。通过Pipeline运行`ParticleStage1Entry.StartOsc`（或在GM中手动打开相同端口的OSC）后，在**Windows工程目录**执行：

```powershell
python tools/compile_stage1.py --demo --output Docs/Stage1/demo-commands.json --report Docs/Stage1/demo-plan.json
python tools/send_stage1.py --seq 0 script --input Docs/Stage1/demo-commands.json
```

演示从空场景开始：绘制20×10×1墙体，挖去一格，再绘制独立2×3×4盒体。最终墙体4颗粒子覆盖199格，另一盒体1颗粒子覆盖24格。此命令的seq=0适用于刚进入Play Mode的接收器；后续使用返回的nextSeq。

自定义高层编辑序列JSON第一条必须是`{"op":"clear"}`，后续可以连续`box`或`erase`：

```json
[
  {"op":"clear"},
  {"op":"box","min":[128,128,128],"size":[20,10,1],"styleId":2},
  {"op":"erase","min":[138,133,128],"size":[1,1,1]}
]
```

将文件作为`compile_stage1.py --input edits.json --output commands.json`输入，再由发送器发送。原型编译器每次从空场景编译整段序列；跨轮持久化编辑和Draft/Published发布仍属后续阶段。

## 重跑验证

Linux本目录项目根下执行：

```bash
python3 -m unittest discover -s tools -p test_stage1_geometry.py -v
python3 tools/check_stage1.py
python3 tools/check_stage1_plan.py
python3 tools/check_observation.py
python3 tools/unity_remote.py --check-result run_script --file AgentScripts/ParticleStage1Entry.cs --entry ParticleStage1Entry.ValidateExport --timeout_ms 60000
```

集成脚本要求Stage1场景已打开、Edit Mode空闲；拒绝接管已有Play Mode。脚本经独立Windows Python发UDP，观测器不直接写接收参数或Emit；结束后关闭本轮OSC并退出Play。截图对比独立CPU构造的参考几何，双方使用相同Shader，因此验证几何／数据路径，不独立验证Shader的所有语义。

`check_observation.py`用于现场观察：它停止OSC后保留Play Mode和阶梯，便于对照Scene/Game；其余集成脚本自动退出Play。

## 尚未覆盖

真实VRChat本机及其他玩家已由用户确认能看到方块；0.2s时隙循环有用户报告的第5/6级缺失，现改为0.25s时隙继续观察，原因尚未定位。已知问题是Avatar旋转时世界坐标纹理发生相对变化，待修复。网络丢包和乱序、晚加入重建、Avatar缩放/重置、长时间压力与CPU/GPU基准、共享运动、资源包导入、GPU Instancing、原子版本切换、面捕预算下的可配置通道，以及AI后端仍未覆盖。当前只显示视觉几何，没有世界碰撞。

持续OSC测试：Windows工程目录运行`python tools/loop_vrchat.py --port 9000 --slot 0.2`。每个时隙仅发一个UDP bundle，准备与执行各占一个时隙，即完整命令0.4s。循环8柱生成→8柱换色→反向删除，直到在Windows用户目录`tmp/morphforge/stage1-loop.stop`创建停止文件。状态写入同目录`stage1-loop-status.json`，发送器整个运行期间持有共享写入锁。停止后重新启动会清除旧停止文件。
