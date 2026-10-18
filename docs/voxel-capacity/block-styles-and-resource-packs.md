# 粒子方块样式编码与资源包导入

日期：2026-10-18。设计分析，尚未实现粒子编码、资源包导入或运行时切换，没有获取或导入Minecraft原版/第三方贴图。

本文保留最初的外观优先编码候选。后续 [运动协议v1](motion-protocol-v1.md) 为运动组和相位重新分配RGBA容量，默认采用4096个组合样式ID，而非同时独立支持4096种方块与64种染色。两种布局不可混用，均未实现。

## 结论

建议采用方块类型ID、染色ID与状态编码，让粒子保留短记录，统一Shader查表决定外观。可以借鉴Minecraft的方块注册表和资源命名，而不依赖某一版本的内部数字ID。

资源包兼容可实现为Unity编辑器导入器：离线解析、生成纹理数组/图集和方块查找表，随Avatar上传。运行时切换的是预先打包的资源；不假定标准Avatar可以读取任意本地ZIP或下载资源包。

## 粒子属性与Avatar入口的区别

Unity Particle含position、velocity、startSize3D、rotation3D、startColor、startLifetime、remainingLifetime、randomSeed等字段。Renderer还可输出位置、颜色、UV、法线、切线、速度、尺寸等Vertex Streams，及Custom1/Custom2。

字段存在并不表示Avatar能逐粒子任意调用C#写入。首版优先验证发射器位置、出生颜色、出生尺寸/朝向这些原生发射属性与动画绑定；randomSeed、SetMeshIndex、SetCustomParticleData等脚本接口不能当作可用的Avatar接收器。

Custom1/2的配置可由模块曲线生成、也有脚本填充方式。不能只因它们有额外向量就宣称能把共享OSC参数永久锁存在每个新粒子中；需先验证模块求值语义，避免输入一变旧粒子也变。

startColor实际为Color32，RGBA各8bit。Shader可将它解释为数据，而不是直接作为最终表面颜色。禁用会改写颜色的生命周期模块和随机渐变；GPU Instancing、Gamma/Linear及实际发射/网络链路需要逐值检查。

## 两级协议候选

### 最小验证版

`SetVoxel(x,y,z,styleId)`，styleId选择64个预定义外观组合，0为空。每块仍是4个有效数值。外观表条目可以同时定义六面贴图、颜色和材质属性，不必只表示纯颜色。

### 扩展版

建议先按每个通道64个离散状态规划，不直接填满全部字节状态：

| 出生颜色通道 | 逻辑内容 |
| --- | --- |
| R | blockId低6bit |
| G | blockId高6bit |
| B | tintId，0–63 |
| A | state，0–63，用于朝向/变体等 |

每路将0–63映射为0–1，经Color32量化后由Shader取整恢复。`blockId=R+64*G`，共4,096种方块编码，其中0可留空；64种染色、64种状态是独立表项容量，不表示能同时显示同样数量方块。

这是24bit逻辑编码的设计目标，尚未验证。内部RGBA32存储不能直接推出OSC具有32bit无损载荷；VRChat同步Float虽然占8bit预算，只有255个取值，不能直接无损映射任意256状态。每路6bit为首轮保守选择，后续可比较7bit。

使用A作数据时，自定义Shader应将其与视觉透明度分离。透明度由纹理/材质表决定，不走默认的顶点Alpha相乘路径。为保持编码，可先用Opaque或Cutout路径验证。

扩展协议还需要XYZ，共7个有效数值。它比最小版占用更多通道；样式自由度增加并不等于原先20块/秒的理想传输估算仍然成立。

## 外观表

后端保存稳定名称，例如`minecraft:oak_log`加axis状态，再映射成本项目自己的blockId。不能假定不同版本Minecraft的内部整数编号永远一致。

可以生成如下静态表：

```text
blockId → 六面纹理索引、染色掩码、粗糙度、金属度、发光参数、渲染类型
tintId  → 颜色
state   → 约定的朝向/变体
```

Cube载体每面提供面编号/局部UV，Shader依据方块类型、面和状态选贴图。草方块上下侧不同、原木端面与树皮不同均适合这个结构。

推荐少量共享材质、纹理数组或图集；不为每个方块类型创建独立ParticleSystem和Unity材质。真正的Alpha Blend、Opaque与Cutout的渲染状态不能凭一个ID随意逐粒子切换，必要时按渲染类型分成少量系统。

## 资源包导入与开关

```text
选定Java版版本的资源包ZIP/目录
    → 解析纹理、models继承及blockstates的支持子集
    → 生成统一尺寸的Texture2DArray/图集和查找表
    → 打包进Avatar
    → OSC/菜单切换预上传资源包或纯色模式
```

不同包应共享本项目方块ID表。切换资源包只改变采样的纹理/映射，旧粒子的方块ID保持，因此设计上无需重发整个场景。该切换链路尚未实现，Shader/Animator资产需后续通过Unity API建立。

可先支持：

- 完整立方体的六面贴图和不同分辨率像素纹理。
- 染色掩码/颜色表；复杂生物群系规则可先由后端选择tintId。
- 后续支持`.mcmeta`动画，导入为帧序列，由Shader按时间选帧。

不将“导入材质包”等同于运行Minecraft渲染器：台阶、门、栅栏等非立方体需要额外形状支持；连接纹理需要邻接信息/规则；OptiFine扩展、PBR约定与Minecraft Shader Pack要分别适配。资源包覆盖不完整时应明确报告缺失或使用自有占位材质，不默认从原版包补齐并打包。

Unity Texture2DArray要求统一切片尺寸/格式，导入器需统一或按规格分组。图集需处理边缘与mipmap串色。增加纹理种类主要增加纹理存储与采样资源，不会提高粒子块数上限。

## 资源许可

技术上兼容资源包不等于任何资源包都可直接随Avatar发布。Minecraft当前Usage Guidelines将向他人分享也纳入其commercial use定义，不以是否收费为唯一条件；EULA也明确涉及内容复制/分发。因此“不盈利/二创”不能单独证明移植原版贴图到其他游戏并分发获得许可。

原版资源按Mojang/Microsoft条款核对；第三方包需看作者是否允许修改、跨游戏使用和再分发。兼容导入器可先用自制素材或明确允许相应用途的素材验证，无需因此改变方块编码设计。

## 一手依据

- Unity Particle：https://docs.unity3d.com/2022.3/Documentation/ScriptReference/ParticleSystem.Particle.html
- Unity startColor / Color32：https://docs.unity3d.com/2022.3/Documentation/ScriptReference/ParticleSystem.Particle-startColor.html 、https://docs.unity3d.com/2022.3/Documentation/ScriptReference/Color32.html
- Unity Vertex Streams：https://docs.unity3d.com/2022.3/Documentation/Manual/PartSysVertexStreams.html
- Unity Custom Data：https://docs.unity3d.com/2022.3/Documentation/Manual/PartSysCustomDataModule.html
- Unity Texture Arrays：https://docs.unity3d.com/2022.3/Documentation/Manual/class-Texture2DArray.html
- VRChat参数：https://creators.vrchat.com/avatars/animator-parameters/
- VRChat组件：https://creators.vrchat.com/avatars/whitelisted-avatar-components/whitelisted-avatar-components/
- MinecraftForge作者维护的模型系统文档：https://docs.minecraftforge.net/en/1.21.x/resources/client/models/
- Minecraft Usage Guidelines：https://www.minecraft.net/en-us/usage-guidelines
- Minecraft EULA：https://www.minecraft.net/en-us/eula
