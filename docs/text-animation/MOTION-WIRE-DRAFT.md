# 4组本地动画接收器实施草案

字段设计已在2026-10-22实现并完成原生接收/播放/GPU验收，最终打包进展见MOTION-IMPLEMENTATION.md。以下保留设计依据。基础包括：mf6::encode_group/decode_group/packet_group，unified按组打包及记录sent group，出生Alpha与Rotation3D原中心的GPU验证。Windows0.20候选是E1文字版本，不含这些新动画开发。

## 几何组

0静态，1..3动画。普通Boxes/SharedBoxes用D28高2bit，Voxels用D28位3..4，Triangles用D28位2..3（低2bit为数量），Strip用D27低2bit。Region8只用于静态。Frame::decode仍严格拒绝原保留位，decode_group先移除已声明组位再校验原布局。删除不需要组。未声明能力不发组位。

Native normalized record D16保存组。Box出生Alpha=255-(mode+8*shape+40*g)，三角Alpha=base1..6+6*g。Bounds仍0，lifetime与三角ID不变。动画Box RotationZ=(cx2+512*cy2)/2^20，RotationX=cz2/2^20，Y0，cx2=2*minX+extentX+1；Shader剥离Rx*Rz编码恢复根基底与原中心，再应用组TRS。相关源码已改但尚未重建8池并验收。

## 扩展载荷（D28=254，Ctl普通4bit序号/format0）

| 消息 | 字段 |
|---|---|
| op16 定义header | D1组1..3，D2 bank0/1，D3代次0..3，D4时长索引，D5 loop0/1，D6 keyCount1..4，D7插值0step/1linear，D8..13 pivot XYZ各U16(bias32768、1/64格)，D14效果保留，D15速度索引，其余0 |
| op17 关键帧 | D1组，D2 bank，D3 keyIndex0..3，D4..5归一化时间U16/65535，D6..11 translation XYZ U16偏置/64，D12..19 quaternion XYZW U16偏置32768、/32767，D20..21正均匀scale U16/4096（首轮限制1/16..8），D24代次，其余0 |
| op18 控制/提交 | D1组，D2 bank，D3代次，D4控制ticket0..15，D5状态0stop/1play/2pause，D6归一化phase U8/255，其余0 |

时长预设候选[.125,.25,.5,1,2,4,8,16,32,64]秒；速度[.25,.5,1,2]。首轮超能力Clip明确拒绝，编辑器64轨道/任意时间和GLB导出继续保留。以实际测试最终冻结。

## 有限接收状态机，避免版本笛卡尔积爆炸

- 全局rate解码层40状态，在Stage1 header时按(durationIdx,speedIdx)设置一个本地float decodedRate。
- 每组/每bank header接收按4代次枚举。代次变更时写元数据、保存bank rate/keyCount/loop，清4个keyReady与bankReady；重复同代次header不重置。
- key接收按bank×key×代次枚举，必须匹配bank当前代次，写18个字节缓存并置keyReady。
- 每bank Ready层按keyCount1..4检查所需keyReady。通过后才允许控制切换到该bank。旧bank继续显示，未完成定义不参与渲染。
- 控制Validate层按bank×4代次检查bankReady与版本，Stage1先清Valid，Stage2匹配后仅写SelectedRate/SelectedLoop和Valid，不改当前时钟。
- Apply层按16 ticket×3状态，要求Valid且AppliedTicket!=当前常量ticket。设置ActiveBank、Enabled、phaseBase、Rate（play复制SelectedRate，pause/stop0）、loop；更新AppliedTicket和PendingTicket。重复ticket连phaseBase也不修改。
- Clock层按16 ticket×loop/once两种Clip；PendingTicket变化才重入并更新ClockTicket。Clip输出elapsed0..1、speedParameter=Rate。不能用cycleOffset定位once：GPU前数学fixture已证实once忽略它。Shader统一计算loop frac(phaseBase+elapsed)，once saturate(phaseBase+elapsed)。这样可定位、暂停、继续，重复补播不重启。Late join收到当时phaseBase后启动，不承诺跨端严格同相。
- 目标约450个有限状态、每组输出/缓存/时钟少量层，不为每Box或每字形创建层。

## 安装与Shader

Motion MergeAnimator可装在ProtocolCenter上（当前无MergeAnimator），Relative路径直接用Pool0/Store…Pool7/Store与TextDisplay/Panel0…3。本地float字节缓存/RawDirect系数输出复用文字经验，必须保留固定零基准，明确区分Renderer类型。Clock elapsed由独立Clock层输出，不能被缓存零基准覆盖。

定义双bank，各4关键帧；Shader选ActiveBank，查4帧，最短路径四元数slerp、step/linear、正均匀scale。Box按原中心和基底变换，三角面直接变换原顶点及法线。文本框在局部字号/朝向布局后也应用组TRS；样式D13/cache B299可承载文字group。0或未启用组保持基础姿态。运动包络必须满足目标256范围；原始地址用于删除，shader运动不改变擦除地址。

## 应用侧后续

Transport增加motion session（定义/控制重播），不另开UDP。worker用frame.packet_group(seq, unified.frame_group(plan))，组定义和控制先于对应几何，公平调度实时文字。发布动画适配现有creative Clip，最多3轨道/4键/预设时长速度/均匀scale，复杂Clip明确报不支持。同一对话AI可保存、发布、暂停、继续、停止/定位；界面提供对应按钮并保留Editor与Avatar能力差异。切换绑定需要重写受影响的已发送geometry，unified已记录group_id。
