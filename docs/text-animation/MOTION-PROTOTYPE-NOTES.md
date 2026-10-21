# 原生动画原型待验证笔记

2026-10-22：此文件是下一步实验候选，不是已实现能力，也不覆盖ANIMATION-DESIGN中的最终验收要求。

1. 可从4组开始（0静态、1..3动画），避免一上来8/16组。普通Boxes/SharedBoxes的metadata高2bit空闲；Voxels的D28高位、Triangles的D28计数字段高位、Strip的D27也可容纳2bit。Region8已占满，动画组先不用Region8，静态组继续。用`encode_group/decode_group`包装现有Frame，保留旧静态规范与测试；不要给每组额外同步参数。
2. 原生Fields=20中D16当前未使用，可承载group。Box birth lifetime=1,000,000+group*16；三角life=524288+id*8+group（仅0..3，所以原来的round(id)不受影响，须测量InvStartLifetime精度）。普通group0继续走现有显示，避免改变静态效果。
3. Box做任意枢轴TRS还缺持久的原始中心。候选：仅动画Box启用初始Rotation3D编码，令cx2=2*minX+extentX+1（9bit），cy2/cz2同理；rotationZ=(cx2+512*cy2)*2^-20，rotationX=cz2*2^-20，Y=0。新增Rotation3D顶点流，解码原中心；用逆Rx*Rz从MF6Basis剥离编码旋转，恢复作品基底，再应用动画。必须先证实顶点流单位/布局、Unity粒子旋转顺序、缩放和逆矩阵，没有验证前不能影响普通静态粒子。
4. 三角面已在Shader重建原始三个顶点，可先验证group本地TRS，法线同样变换。按原始地址删除/替换，不用变换后的空间地址。动画运动包络要入目标范围检查。
5. 小容量片段候选：3动画组、每组4关键帧，纯色/贴图Box和三角面共用。四元数最短路slerp与编辑器保持一致，缩放首轮可限制为正均匀缩放并明确拒绝超能力片段。元数据/关键帧/控制经format0+D28=254扩展，不逐帧上传几何。关键帧可用一个包传归一化time u16、translation i16x3、quaternion i16x4、scale u16，加组/帧索引与版本。
6. 本地时钟优先每组一个Animator循环相位Clip（0..1），speedParameter=1/duration；暂停speed=0。为了不用复杂的任意Float解码器，可先用明确时长预设(.125,.25,.5,1,2,4,8,16,32,64秒)及速度预设。控制命令携带播放/暂停/停止、8bit相位与4bit控制版本；枚举16版本的入态条件实现重播不重启，晚加入初始化。一次性Clip/seek的cycleOffset实际行为需单独验证，不能以_Time承诺跨端同相。
7. 定义最好双缓冲：未显示的bank接收完整定义/所有关键帧，满足对应版本与key flags后commit切换。重复补播不重置接收标志和时钟。需评估状态数与版本匹配，不用无校验的半份定义参与显示。原生cache可复用文字的本地Float缓存/门控输出经验。
8. 作者侧已有creative Clip/Track/Keyframe及web/motion.js。适配器需在每个打包帧只组合相同motion group，LiveBox/LiveTriangle记录已发送group，修改绑定时重写原始几何；不能只看surface/shape而漏掉group变化。删除帧不需要组。目标能力缺失时明确拒绝动画，静态发布继续可用。
9. 文字分组也可通过样式扩展D13（缓存B299）传group，Quad位置在字号/文本框旋转之后应用同一组TRS。文字滚动/闪烁/逐字和材质发光/UV效果可以随后共享本地相位。AI工具、界面、动画包、补播与暂停/继续都要验证，不仅在编辑器看到动画。

额外优化实验：2026-10-22已在Unity临时数学fixture证实未归一化Direct BlendTree权重0、.5、1、2、65、255均不截断，可用缓存byte作为权重、常量系数Clip作乘法，减少文字输出的1D树/双叶。仍需在实际合并Avatar重复文字及CPU验收后才能替换，不将数学fixture当作成品通过。

## 进一步精简候选（尚未验证）

group标签也可以复用出生颜色Alpha，保持现有lifetime和三角ID完全不变：Box rawAlpha=255-(mode+8*shape+40*group)，4组时范围96..255；三角rawAlpha=baseAlpha+6*group，范围1..24；0仍为不可见bounds。两者不重叠，且40是8的倍数，原Box材质mode仍为code%8，只需shape取(code/8)%5。三角先从alpha解group再恢复1..6基础类型。Native颜色树加相应group系数，注意验证负系数在最终Color属性写入前的行为。此方案可能比借用lifetime更简单；动画Box原始中心仍需Rotation3D编码实验。

文字Raw Direct权重优化的真实接收数据通过，但所有子权重为0时旧Renderer属性可能保留，因此输出根节点需要恒定零值基准Clip覆盖所有字符/配置属性。首次无基准优化在删除/透明度归零测试失败，正在加基准后复测。CPU比较必须使用填满真实CJK字节的512槽，而不是只打开空槽；同时记录0槽和收到的28槽，正反顺序测量，注明Editor整个人物Animator开销。


2026-10-22 MotionCarrierProbe 已实际GPU验证：新增Rotation3D按预期打包到centerLife.w及TEXCOORD3.xy，流中单位为出生局部弧度，不包含系统根旋转；(0.4,0,0.8)校准与极值中心编码两例全部通过。按逆Rx*Rz恢复根基底，与系统根XYZ轴点积均>0.999999；半格中心(1,511,511)和(511,1,1)精确读回。说明中心存储候选可继续实现，不代表完整组动画已经完成。实测在Edit Mode隔离预览场景，无Avatar代码或场景改动。
