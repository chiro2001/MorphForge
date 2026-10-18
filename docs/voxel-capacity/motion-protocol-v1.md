# MorphForge 粒子体素与运动协议草案 v1

日期：2026-10-18。状态：可评审设计，尚未实现接收器、粒子出生编码或运动Shader，没有完成多人验证。机器可读摘要见 [motion-protocol-v1.json](motion-protocol-v1.json)。

后续 [修订A](motion-protocol-v1-rev-a.md) 已建议取消必传姿态，将对应载荷用于XYZ高位，扩大为256³、64m范围。本文的64³和姿态字段保留作旧方案对照，不应与修订A混用。

## 设计选择

把传输入口、每体素持久记录、共享运动组分开。OSC发送编辑意图与运动参数；原生粒子保持逻辑位置与短编码；Shader按共享时钟计算可见运动。保持粒子原生速度为0，避免逻辑格地址漂移，继续支持按格坐标Trigger擦除。

运动优先由少量共享组描述。每块只存组号和相位，不为每块保存一套振幅、频率、支点，也不为每块生成动画层。首轮实现4个组槽（0静态、1–3运动），编码预留8组（0静态、1–7运动）。7组是同时存在的运动配置数量，不是可运动体素数。

每个运动组16个配置数值，首轮3个可写组需48个持久数值，扩展7组需112个。可研究复用小规模参数缓存/材质路由；它仍有固定CPU开销，不能只因组数少就假定性能已成立。缓存规模随组数而非体素数增长。

## 1. OSC传输入口：20个同步参数

沿此前统计，原Avatar已有89bit同步参数。VRChat Int/Float均占8bit、总预算256bit。新增以下20个参数占160bit，合计249bit，剩7bit。预算基于此前报告，落地前需重新核对最终合并资产。

| 参数 | 类型 | 用途 |
| --- | --- | --- |
| MF_Ctl | Int | 版本2bit、操作码4bit、阶段2bit |
| MF_Seq | Int | 命令序号，0–255，非对象ID |
| MF_Target | Int | 目标组/分区，含义由操作码定义 |
| MF_Epoch | Int | 场景代数，重置与快照边界 |
| MF_D00…MF_D15 | Float×16 | 共享载荷，不为各方块新增同步参数 |

`MF_Ctl=(version<<6)|(opcode<<2)|stage`。v1版本号为1；stage=0准备、1执行、2取消、3预留。阶段是设计语义，不是原子提交保证。

载荷首轮每路只用64个逻辑状态：`f=-1+2*q/63`，q=0…63，接收后取整恢复。同步仍按8bit收费，但有效载荷为每路6bit，16路共96bit/批，不能继续将其称为16个任意字节。Color32通道保存时映射到q/63，传输归一化和出生颜色归一化不是同一个公式。

Float有255个远端取值，且可能出现过渡值，所以仅理论取整不构成远端正确性证明。命令数据应先稳定、再提交，接收端锁存后执行；prepare/commit可能跨多次网络采样。序号、代数、OSC bundle均不单独保证可靠、有序或原子传输，也不代表所有观看者已确认。

## 2. 每个体素：7个6bit字段

| 字段 | 逻辑位数 | 范围/用途 |
| --- | ---: | --- |
| gx、gy、gz | 各6 | 0–63格坐标 |
| styleLo、styleHi | 各6 | 合成12bit样式ID；0表示空 |
| poseFlags | 6 | 低5bit为姿态码；高1bit预留为0 |
| motionPhase | 6 | 低3bit运动组；高3bit为相位偏移 |

姿态码0–23对应立方体24种正交朝向，24–31预留。相位偏移为0–7，对应一个周期的0/8…7/8。静态组0忽略运动相位。

粒子原生位置保存格中心，RGBA四路分别保存styleLo、styleHi、poseFlags、motionPhase。RGBA每路暂用6bit，共24bit持久元数据。Shader把A作为数据读取，视觉透明度从样式/纹理取，不与A直接相乘。

首版原生粒子尺寸统一、旋转为0；姿态和组运动由Shader计算，避免原生变换与编码姿态重复作用。Renderer需向Shader提供可用的粒子中心、颜色和Mesh局部几何信息，验证普通绘制与GPU Instancing两种通路。保持刚体结构时，同组成员使用相同相位；不同逐粒子相位用于波浪/错峰等效果。

样式ID不是“4096种材质×64色”。它表示4096个材质/贴图/染色组合（0为空）；外部编辑模型仍可保留独立material、tint、shape等信息，由编译器分配组合ID并检查查找表是否支持。新组合若不在已上传表中，不能仅分配ID就自动生效。

上轮12bit方块+6bit染色+6bit状态已占满24bit，本草案明确调整为组合样式+姿态+运动组+相位，给运动留空间。另一种可选codec是10bit方块、4bit染色、5bit姿态、3bit组、2bit相位，合计24bit，获得1024方块类型、16色、24朝向、8组和4相位。该codec仅预留，不与默认编码混用。

初始格长h=0.25m、原点为网格角点。格中心=`anchor+(g+0.5)*h`，64³网格覆盖16m×16m×16m。可寻址262,144格；同时显示容量另由粒子上限决定。改变格长、原点或codec视为场景重建，不偷偷改变既有粒子的地址含义。

运动改变可见位置，逻辑格坐标保持。点击可见运动方块时应由后端/交互层反求逻辑地址，不能默认把擦除器放到屏幕上看到的位置。若Shader运动超出原生粒子包围盒，需验证Renderer bounds、阴影、深度与镜面等通路。

## 3. SetVoxels载荷布局

```text
D00…D06：体素0（gx,gy,gz,styleLo,styleHi,poseFlags,motionPhase）
D07…D13：体素1（同结构）
D14：记录数，1或2
D15：预留，必须为0
```

同一包中不允许两个记录指向相同逻辑地址，避免并行擦除/发射顺序歧义。删除使用style=0，其余元数据填0。一次覆盖完整记录，不做隐式字段补丁。

一条命令最多2块。按0.2s一条完整已接收命令，理想10块/s；若每条命令准备/提交各占0.2s则理想5块/s。实际还需扣除重复、粒子操作排队与快照，16路数据并不代表本地有两个并行发射器。

## 4. 共享运动组：一次16字段完整覆盖

MF_Target为组ID，0不可改写，首轮1–3有效，后续可扩到1–7。

| 载荷 | 字段 | 解释 |
| --- | --- | --- |
| D00 | mode | 静止、正弦平移、匀速旋转、摆动、绕行、缩放脉冲、往返平移 |
| D01 | axis | +X,+Y,+Z,-X,-Y,-Z；其他值预留 |
| D02 | distance | 平移振幅/行程，q×h/4；h=0.25时0–3.9375m |
| D03 | periodCode | 64档周期，约0.273–65.536s；见时钟算法 |
| D04 | phaseBase | 组相位偏移，q/64周期 |
| D05,D06 | pivotXLo/Hi | 12bit支点X |
| D07,D08 | pivotYLo/Hi | 12bit支点Y |
| D09,D10 | pivotZLo/Hi | 12bit支点Z |
| D11 | playback | 0停止回初始、1运行、2暂停在指定相位 |
| D12 | magnitude | 摆动角度q×180/63度，或缩放振幅q/63；由mode解释 |
| D13 | clock | 0网络循环；1粒子年龄、2共享时间线、3编辑器时钟预留 |
| D14 | frozenPhase | 暂停时显示的相位q/64，不依赖捕获Shader内部瞬间状态 |
| D15 | flags | bit0反向，bit1逐块自身支点；其余预留0 |

支点各轴解码为`(lo+64*hi-2048)*h/16`，相对anchor。h=0.25时约-32到31.984375m，分辨率0.015625m。支点可处在格边/格内，不只限于体素中心。

平移采用组公共位移；整组旋转和摆动应同时变换体素中心与局部顶点方向，形成刚性结构；绕行只改变中心、不随轨道转动朝向；逐块自身支点标志可用于各块原地旋转。模式未使用的字段填0，不让旧配置残留决定行为。

共享组ID需要后端引用计数。覆盖某组会影响全部引用粒子；只改一块的独立运动时，应分配空闲组并重建该块的组引用。仍被使用的组不能静默挪作新用途。不同块的任意独立频率、振幅和轨迹不属于此24bit编码已解决的能力。

整组配置需要锁存后统一应用，不能让绘制阶段长期读取半更新参数。若验证表明需要组参数双缓冲，首轮48个配置数值会变为96个，扩展112会变为224个；同步入口预算不因此翻倍，CPU缓存与输出结构则需重新测量。

## 5. 时钟与循环运动

官方VRChat Shader Globals提供同步的`_VRChatTimeNetworkMs`，对应Udon的GetServerTimeInMilliseconds。现有SDK镜像也有`Packages/com.vrchat.base/ShaderLibrary/VRCTime.cginc`与`VRC_GetNetworkTimeInMilliseconds()`。因此首版默认网络时钟，不把普通Unity _Time当作跨玩家同步时钟。

为避免把巨大时间戳直接转float以及处理uint回绕，可采用以下64档周期设计（本项目推导，尚未Shader验证）：

```text
e = 12 + (periodCode >> 3)      // 12…19
m = 15 - (periodCode & 7)       // 15…8
windowMs = 1 << e
T = windowMs / (1000*m) 秒
u = (networkMs & (windowMs-1))*m / windowMs
phase = frac(u + phaseBase/64 + particlePhase/8)
```

u只使用低位时间，分子最大7,864,305，小于2²⁴。window是2的幂、m为整数，窗口和32bit时间回绕均减少整数个周期，循环相位在数学上连续。周期最短4096/15000≈0.273067s，最长524288/8000=65.536s。周期不保证是用户熟悉的整数秒，UI应显示实际值；精确指定任意秒数可留给扩展时间线。

同步时钟只解决时间基准，不保证参数同时到达，也不证明动作误差为0。改变周期/组相位可能跳变；暂停使用显式frozenPhase，平滑暂停/恢复和参数过渡需另外设计。普通循环可以无状态计算，任意时间触发的一次性整组动作仍需要明确共享起点。

## 6. 操作码与扩展边界

| opcode | 含义 | v1范围 |
| ---: | --- | --- |
| 0 | Nop | 有效 |
| 1 | SetVoxels | 两条以内完整覆盖记录 |
| 2 | EraseBox | 预留区域删除布局，沿用逻辑格坐标 |
| 3 | SetMotionGroup | 上述16字段 |
| 4 | SetScene | 场景codec、格长/原点配置，重建边界 |
| 5/6 | BeginSnapshot/EndSnapshot | 快照与恢复边界，非网络确认 |
| 7 | ClearScene | 待验证原生清空机制 |
| 8 | SetTimeline | 预留：时间源、起点、duration、easing |
| 9 | SetPalette | 预留：动态颜色缓存，不假定已能运行时改纹理LUT |
| 10/11 | SetSurface/SetMesh | 预留其他显示后端，尚无粒子持久存储方案 |
| 12–14 | Reserved | 拒绝执行 |
| 15 | Extension | D00选择扩展子操作，未实现 |

未来时间线可用6个6bit字段承载32bit参考时间，另含持续时间、缓动与版本。得到可用的共享起点、客户端向外部提供时间信息及精度方案尚未完成；保留字段不等于已获得该能力。

新增独立逐块运动自由度时，优先研究原生Size/Rotation等额外出生流能否安全存数，或切换GPU缓存。Custom1/2、randomSeed等脚本可写字段不作为v1成立前提；不能只增OSC字段而不给旧粒子安排持久存储。

## 7. 首轮实现/验证范围

1. 64块粒子容量；4096样式编码先只填少量样式；四通道逐值编码/取样验证。
2. 固定逻辑位置，验证静态体素的生成、覆盖、删除与Ring Buffer回归。
3. 实现3个非静态组：正弦平移、整组旋转、摆动；24朝向及8相位可分步开通。
4. 同组修改影响全部成员；换组需替换粒子；运动中按原逻辑格擦除。
5. 双客户端验证网络时钟、参数到达错位、晚加入全量快照、周期边界和相机通路。
6. 测量粒子模拟、固定组缓存与Shader的独立CPU/GPU成本，再扩到256/1024块。

## 来源

- VRChat Animator Parameters：https://creators.vrchat.com/avatars/animator-parameters/
- VRChat Shader Globals：https://creators.vrchat.com/worlds/udon/vrc-graphics/vrchat-shader-globals/
- 本地SDK：`tesla-roadster-avatar/Packages/com.vrchat.base/ShaderLibrary/VRCTime.cginc`，只读核查，无修改。
- Unity Particle Vertex Streams：https://docs.unity3d.com/2022.3/Documentation/Manual/PartSysVertexStreams.html
- Unity Particle startColor：https://docs.unity3d.com/2022.3/Documentation/ScriptReference/ParticleSystem.Particle-startColor.html
- 本项目 [通道预算](../channel-capacity/estimates.json)、[粒子增删](particle-editing.md)、[样式编码候选](block-styles-and-resource-packs.md)。
