# Avatar 外部建模：减少 Animator CPU 开销的候选路线

日期：2026-10-18。范围为标准 PC/PCVR Avatar、OSC 参数入口、外部逐步建模。本文为官方资料、原作者技术资料与设计分析；没有创建 Unity 资产或完成以下方案的客户端验证。

用户关注的是 KipSeek 逐属性路由产生巨大 Animator 的结构性成本。各方向需要区分存储、数据表达和几何显示：将显示改为 Geometry Shader 或 Raymarching，本身不会获得跨帧存储；将网络字段压缩，也不会自动消除逐槽位的 CPU 动画绑定。

## 方向对照

| 方向 | 原理 | CPU 成本如何变化 | 关键限制 | 当前证据 |
| --- | --- | --- | --- | --- |
| Camera + RenderTexture GPU 内存 | 少量 Animator 输入驱动缓冲 Quad 材质；双相机/双纹理交叉反馈；命中地址覆盖，其他像素复制旧值 | 从逐保存属性的 Animator 路由转为少量入口与固定相机提交；GPU处理纹理 | 相机权限、渲染开销、同 Avatar 实例共享纹理、初始化和恢复 | pema99 原作者资料明确讨论 Avatar camera loop；OSC 建模移植未验证 |
| Custom Render Texture 自更新内存 | CRT Realtime 执行更新 Shader，双缓冲读取上一轮状态 | 可避免依赖显式相机的写入，是否省 CPU 需实测 | 动画化材质实例与 CRT 更新材质可能不同；同资源实例隔离；输入数据桥接 | Unity功能明确；完整 Avatar OSC 接入仍属研究，不能等同相机路线成熟度 |
| Parameter Driver 寄存器 + 连续 Blend Tree 输出 | 页地址触发状态进入；Copy 把共享输入一次性存到仅 Animator 声明的参数；少数持续运行的 Blend Tree 输出属性 | 少量状态机层，写入事件化；参数和持续求值的输出树仍随容量增长 | 不是 GPU 存储；大量参数、曲线与树节点可能仍吃 CPU；接收页需重新进入状态 | Copy 和仅 Animator 参数由官方支持；组合架构未测 |
| Mesh 粒子/环形缓冲记录 | OSC 控制发射位置、初始外观及触发；每次发射一个立方体粒子，粒子内部状态保留内容；Ring Buffer可在寿命结束后暂停保留 | 省去每块独立 Animator 路由；仍有 Shuriken 粒子模拟及绘制成本 | 很适合追加，按 ID 修改/删除困难；达到容量后覆盖旧粒子；远端补发易重复 | 允许组件、Mesh渲染与Ring Buffer有官方基础，OSC精确单次发射未测 |
| 紧凑参数化场景 | 外部把墙、柱阵列、楼梯等编译成盒体/重复规则；Shader 根据少量当前参数生成大量视觉几何 | 减少保存、驱动与传输的记录数 | 少量参数只能覆盖生成器表达的场景族；许多独立任意修改仍需存储 | 设计推导；可与 GPU 内存或小型 CPU 缓存组合 |
| GPU 场景命令解释器 | 在前两种 GPU 内存上解释 SetVoxel/FillBox/EraseBox/SetMotion 等固定指令 | CPU 只更新指令入口；GPU 并行修改数据或执行局部运动 | 依赖 GPU 存储先成立；固定指令集、有限循环/容量，复杂度较高 | Shader 有状态计算已有作者实现；MorphForge命令系统未实现 |

按输入通道分层的 Simple1D/WD Off 材质路由仍可作为对照方案，详见 [文本保持核查](text-storage-review.md)。它减少层数，但保存属性、曲线与状态数量仍随容量增加，不能作为摆脱固定 CPU 数据结构的最终答案。

## Camera GPU 内存的具体实现

```text
OSC 数据/地址/写入控制
    → 少量 Animator 驱动材质输入
    → Camera A：读旧纹理B，计算并写入纹理A
    → Camera B：将A复制回B
    → 几何显示 Shader 从纹理读取记录
```

两个 Camera 的执行顺序和纹理读写关系应明确，避免同一次绘制采样正在写入的目标。不要依赖单 RenderTexture 在某些 HDR/格式设置下偶然工作的反馈现象。

稀疏布局可把每个 RGBA8 像素作为一条 XYZ格索引+样式的体素记录；64×64纹理有4,096条记录，原始单缓冲16 KiB、双缓冲32 KiB。这只是存储算术。显示载体、颜色表、相机及其他资源另计，也不提高OSC速度。

稠密布局可由像素位置推导体素XYZ，仅存样式。收到 FillBox 时，每个像素自己判断对应体素是否落在盒内，命中则更新样式，否则保留旧值。这给出一条无需每体素 Animator 路由的区域写入实现思路。

相机开销并不为零。即便只写很小的纹理，也需要测量 CPU 渲染提交；纹理全屏读写及双缓冲也有 GPU 成本。

官方当前规则：远端 Camera 加载时禁用，可用动画启用；好友或手动 Show Avatar 可保留 Camera；不满足上述条件时 Camera 会被移除。pema99 旧文的“仅好友”说法应以当前官方规则修正。

同一 Avatar ID 被多个玩家使用时，共享 RenderTexture 是原作者记录过的问题。不能仅凭 GameObject 已克隆就假定缓存独立。独立上传是已记录的绕行方式之一；不能据此宣称已解决通用多实例隔离。

## CRT 的首要门槛

Unity允许初始化OnLoad、更新Realtime、双缓冲读取旧状态。它可执行状态演化，但OSC仍只进入Avatar参数，不能直接写CRT。

必须先回答动画化Renderer材质如何向CRT的更新材质供数。二者可能指向不同运行时材质实例。可以研究中间小纹理/屏幕数据桥接等方案，但不能把箭头画出来就算已实现；若桥接仍需要相机，最终不再是无相机链路。

此外，pema99 的 Avatar camera loop 文档明确把CRT与适合Avatar的相机反馈区分。CRT功能的世界示例或着色器计算示例，不构成当前 Avatar OSC 接入的证明。

## Parameter Driver 路线的区别

这里不依赖WD Off让材质保留值，而是让仅Animator参数保存明确的寄存器值。例如写页状态中将输入D0复制到P0、D1复制到P1；下一页写P16等。远端也应根据同步的共享输入执行Copy，不能误开Local Only使其他观看者没有缓存。

再由少数持续求值的树把本地缓存参数映射成材质属性。若尝试Direct Blend Tree，应按维护者记录采用合适WD设置并验证输出，不能把KipSeek的WD Off树机械合并进去。减少层数不保证CPU按比例下降，因为所有有效输出树仍可能每帧求值。

状态进入时的Copy是一次性行为。同页连续更新需要提交序号/重入路径，否则只改变载荷不一定再次复制。地址、数据、提交的跨客户端配对仍需独立验证。

## 粒子方案的边界

后续已找到原生Trigger Kill与VRLabs Marker的局部擦除依据，具体设计见 [粒子体素增删](particle-editing.md)。因此值得验证按格坐标擦除、再重建的编辑方式；精确发射与Ring Buffer删除仍需实验。

可预置无速度、无重力、World simulation space的Mesh粒子系统，用立方体Mesh显示。发射器移动到目标位置，设置出生外观后产生一个粒子，从而把发射时内容留在粒子状态中。Unity 2022.3 Main Module的Ring Buffer / Pause Until Replaced可让寿命结束的粒子暂停保留，直到达到Max Particles后被新粒子替换，避免只依赖超长寿命设置。该模块在目标Avatar构建后的行为仍需验证。

精确一次发射、出生数据与触发顺序、不可见时模拟及多人同步都需测试。ParticleSystem允许存在，不代表可以在Avatar调用Emit/SetParticles这类自定义脚本API。按ID覆盖任意旧粒子没有因此获得。较适合追加式建造演示、画笔和临时预览，不宜直接承担长期可编辑场景数据库。

## 数据表达与渲染可以独立选择

少量参数的程序化生成可以省数据，但一个seed只描述约定生成器的输出，并不能压缩任意AI场景。重复墙体可编译为一个盒体；多个独立物件仍需多个记录或更复杂指令。

有了GPU记录后，可选择：

1. 固定Mesh载体的顶点Shader按记录摆放方块/面片，资源容量预上传。
2. Geometry Shader根据输入记录生成可见面；需要测GPU输出量和平台兼容性，不能把理论生成数量当成免费容量。
3. 稠密体素使用射线网格遍历，CSG/SDF使用Raymarching；减少载体几何，但GPU成本与屏幕覆盖、步数和VR分辨率有关。它们本身不提供历史存储。

Shader字节码解释器可让运行时上传的指令控制形状和运动，但不需要模仿通用CPU。先实现幂等的绝对覆盖指令；动画优先存振幅、频率、轴和相位等参数并本地计算，不通过OSC逐帧发送所有顶点。运行时指令变化不等于可下载新的HLSL或任意Unity组件。

## 不能作为现成替代的能力

- 标准Avatar没有自定义C#/Udon执行通道，不能直接照搬new Mesh、HTTP加载器、Graphics.Blit或ComputeShader.Dispatch。
- 仅声明RWTexture/RWStructuredBuffer，不会自动建立随机写资源与更新调度。
- GrabPass可复制当前帧缓冲，不能独立当作可靠的跨帧随机访问数据库；若用于桥接，仍需完整的存储及隔离设计。
- 外部进程能发OSC，不等于能把任意本机纹理直接注入标准Avatar材质。
- 所有方向仍有预上传资源和能力边界。目标是固定少量接口、容量与指令集，使场景内容可变，而非为每条数据生成动画层。

## 建议优先级

优先验证Camera双缓冲的随机写入、停发保持、局部覆盖和双实例干扰，得到能与原Animator基线比较的CPU/GPU数据。若目标是一般观看者无额外Show Avatar操作，则相机条件是产品约束，CRT桥接值得另行先研究。

缓存成立后叠加盒体/区域命令与Shader本地运动。Parameter Driver方案作为不依赖相机的中小容量对照；粒子仅在可接受追加式操作时考虑。暂不开发完整GPU虚拟机。

## 一手依据

- VRChat Allowed Avatar Components：https://creators.vrchat.com/avatars/whitelisted-avatar-components/whitelisted-avatar-components/
- VRChat State Behaviors / Parameter Driver：https://creators.vrchat.com/avatars/state-behaviors/
- Unity 2022.3 CRT：https://docs.unity3d.com/2022.3/Documentation/Manual/class-CustomRenderTexture.html
- Unity 2022.3 Particle Main Module：https://docs.unity3d.com/2022.3/Documentation/Manual/PartSysMainModule.html
- Unity 2022.3 Particle Renderer：https://docs.unity3d.com/2022.3/Documentation/Manual/PartSysRendererModule.html
- Unity 2022.3 Direct Blending：https://docs.unity3d.com/2022.3/Documentation/Manual/BlendTree-DirectBlending.html
- Unity 2022.3 GrabPass：https://docs.unity3d.com/2022.3/Documentation/Manual/SL-GrabPass.html
- pema99 原作者 Camera loops：https://github.com/pema99/shader-knowledge/blob/main/camera-loops.md
- pema99 原作者 Geometry Shaders：https://github.com/pema99/shader-knowledge/blob/main/geometry-shaders.md
- pema99 原作者 Raymarching：https://github.com/pema99/shader-knowledge/blob/main/raymarching.md
- PiMaker 原作者有状态Shader计算：https://blog.pimaker.at/texts/rvc1/
- VRCFury维护者WD问题记录：https://vrcfury.com/technical/wd/
