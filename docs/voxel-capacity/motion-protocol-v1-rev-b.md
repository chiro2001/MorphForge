# 运动协议v1修订B：整数传输与浮点工作缓存

日期：2026-10-18。状态：当前推荐设计，尚未修改Unity资产、接收器或OSC发送器，未完成Int传输客户端验证。本修订替代 [修订A](motion-protocol-v1-rev-a.md) 的Float载荷布局。摘要：[JSON](motion-protocol-v1-rev-b.json)。

## 结论

外部OSC和Expression同步入口统一采用Int，Animator内部按需要使用Float。BlendTree需要浮点控制，不要求同步源类型也是Float。首版推荐参数类型保持一致的Int源，提交时通过Parameter Driver Copy一次性转换、锁存到仅Animator声明的Float工作参数。

官方还支持Expression Int到Animator同名Float的自动类型转换。它可作为更直接的桥接方式，但本项目仍需要工作缓存避免下一条输入影响正在进行的擦除/发射；因此显式Copy兼任锁存与转换，便于验证。无需为每个整数值生成256个状态。

## 三层位宽

| 层 | Int | Float |
| --- | --- | --- |
| 标准OSC标量值 | i类型为32bit有符号整数 | f类型为32bit浮点 |
| VRChat同步Expression参数 | 0–255，8bit预算，256状态 | -1到1，8bit预算，255状态 |
| Animator工作参数 | 原生整数 | 原生浮点；仅Animator声明时不受Expression范围夹取 |

后面的249bit统计指Expression同步状态预算，不是UDP包长，也不是每次网络更新固定发送249bit。OSC消息还包含地址、类型标签、对齐、可选bundle及网络头。20个32bit标量的值部分为80字节，实际OSC报文更大。

## 同步参数预算

| 参数 | 数量 | 类型 | 每项同步bit | 合计 |
| --- | ---: | --- | ---: | ---: |
| MF_Ctl | 1 | Int | 8 | 8 |
| MF_Seq | 1 | Int | 8 | 8 |
| MF_Target | 1 | Int | 8 | 8 |
| MF_Epoch | 1 | Int | 8 | 8 |
| MF_D00…MF_D15 | 16 | Int | 8 | 128 |
| 新增 | 20 | Int | — | 160 |

此前原Avatar同步预算89bit，加本接口160bit共249bit，剩7bit。现有MFProbe原型参数是local-only，并未部署本接口。最终合并资产需重新统计，不能把旧报告89bit永远当作不变值。

MF_Ctl沿用版本2bit、操作码4bit、阶段2bit；Seq、Target、Epoch各8bit。它们是32bit协议控制空间，不等于已解决可靠传输。Target可编码256个值，不等于配置256个运动组或粒子系统。

仅Animator内声明MF_Work00…15等Float工作参数，不放入Expression Parameters，额外同步成本0bit。它们仍有运行时内存、动画求值和驱动成本。

## 推荐7字段记录

Int直接承载XYZ各0–255，取消xyzHi字段及粗细位置合成要求。空出的字段用于独立染色ID，首版填0即可。

| 相对载荷索引 | 字段 | 输入范围 | 字段预算bit | 有效逻辑bit |
| ---: | --- | --- | ---: | ---: |
| 0 | gx | 0–255 | 8 | 8 |
| 1 | gy | 0–255 | 8 | 8 |
| 2 | gz | 0–255 | 8 | 8 |
| 3 | styleLo | 0–63 | 8 | 6 |
| 4 | styleHi | 0–63 | 8 | 6 |
| 5 | tintId | 0–63，0无额外染色 | 8 | 6 |
| 6 | motionPhase | 组号3bit+相位3bit | 8 | 6 |
| 合计 | 一块 | — | 56 | 48 |

仍为256³局部网格、默认每轴64m、中心锚点；style=styleLo+64*styleHi为12bit组合样式编码；额外tintId选择64个预上传染色表项，首版仅0有效。运动组和相位保持8组（含静态0）与8档相位。可用样式数量以实际预上传表项为准，不能仅赋新ID就创造新样式。

粒子RGBA仍每路使用64个稳定编码：R=styleLo，G=styleHi，B=tintId，A=motionPhase。Int全8bit用于坐标不要求Color32也立即采用全8bit编码。材质、顶点颜色空间和实例化的实际解码仍需测试。

数据布局：D00…06为记录0，D07…13为记录1，D14为1或2条记录，D15预留0。相同地址不得在同批出现两次。style=0为删除，其余元数据归0。

16个Int载荷的通用能力为128bit，旧Float方案人为限64档时为96bit。推荐记录没有立即填满所有位：两块占14个字段，另有计数和预留；每块7字段虽然占56bit同步字段预算，却只用48bit有效信息。未用位留给后续扩展，不自动提升当前每命令最多2块的数量。

## 接收与BlendTree

```text
OSC /avatar/parameters/MF_Dxx，类型i
  → Expression Int（同步0…255）
  → Animator同名Int输入
  → 提交状态中的Parameter Driver Copy
  → MF_Workxx Float（仅Animator声明，原样0…255）
  → BlendTree / 出生位置和颜色 / 固定运动组缓存
```

例：输入255可原样复制成Float 255.0；BlendTree阈值设0和255做线性位置映射。出生颜色通道则用0…63映射0…1。不能把工作Float误注册进Expression并继续假定255.0不会被范围夹取。

Parameter Driver在进入状态时执行Copy，不会因为停留在同一状态而自动持续复制。每条命令需可靠地触发接收状态进入，先稳定输入再锁存。本地与远端都要执行接收器，不能把该Driver设成只在拥有者运行的Local Only。

动作进行期间只读工作快照，完成后再接下一条。Copy虽可按顺序复制多个字段，仍不能凭此宣称跨网络字段到达是原子的。Seq、Prepare/Execute、重发与全量恢复仍需实测。

另一条官方支持路径为Expression Int、Animator同名Float自动转换。它不需要显式类型转换行为，但不等于已有执行快照；OSC配置类型、GM仿真、MA/NDMF参数类型检查和真实远端应单独验证。

## 压缩与未来空间

暂不把3个元数据字节位级打包成紧凑6字节记录。理论上坐标24bit加元数据24bit可装6字节，但在Avatar侧必须解包、驱动原生粒子出生属性；不能假定有自定义C#位运算，也不希望换来庞大状态表。当前16字节载荷即使改6字节记录仍只能容纳两条完整记录，因此首版优先直观映射。

运动组参数暂保留修订A的16字段语义和64档参数表。整数传输允许后续在不增加同步参数数量的情况下提升某些字段档数，但要同步修改编码、缓存、出生/渲染数据和协议版本，不能只改变发送端。

## 验证清单

1. 扫描0–255所有Int值，确认复制后的本地/远端Float一致，重点包含0、1、63、64、127、128、254、255。
2. 输入255后工作Float保持255.0，确认没有被Expression参数合并规则或构建插件夹取。
3. 0–63元数据经出生Color32恢复正确，旧粒子不受下一次输入影响。
4. 同名自动类型转换与显式Copy两条链路各做小实验，比较GM、构建后和真实客户端；最终选一条作为正式入口。
5. 重复提交、同页连续提交、错位到达和晚加入恢复；不以“Int值准确”推出整条命令可靠。

## 一手来源

- VRChat Animator Parameters（8bit预算、255状态Float、同名参数类型转换）：https://creators.vrchat.com/avatars/animator-parameters/
- VRChat State Behaviors（Copy类型转换、Animator-only参数、Local Only与进入时执行）：https://creators.vrchat.com/avatars/state-behaviors/
- VRChat OSC Avatar Parameters（OSC类型和配置）：https://docs.vrchat.com/docs/osc-avatar-parameters
- OSC 1.0 Specification（int32、float32、消息格式）：https://opensoundcontrol.stanford.edu/spec-1_0.html
- 本项目 [预算报告](../channel-capacity/estimates.json)、[现有原型](../probe14/README.md)。
