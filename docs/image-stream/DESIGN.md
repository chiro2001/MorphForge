# 图片工具、连续补播和发送顺序（0.10）

本轮软件保持MF5/254bit，兼容已上传的MF5 v15；没有改变Avatar线协议或创建新头像。碰撞、三角面和更多通道仅完成下面的可行性分析，没有宣称已在客户端实现。

## 碰撞：必须区分目标

| 目标 | 可行方向 | 当前系统的限制 |
|---|---|---|
| 粒子撞世界/玩家后停止、反弹或触发效果 | Shuriken Collision/Trigger、Avatar Raycast及已有物理交互工具 | 这是粒子与已有碰撞体的交互，不会为每个可见体素创建一个实体Collider。物理球近似也不等于Box表面 |
| 手指触碰、按键、抓握反馈 | 预放少量Contacts/PhysBoneCollider/代理碰撞盒，由动画驱动 | 可以作为交互实验；需要预制对象与状态保存，不能直接随16K粒子自动出现16K碰撞盒 |
| 墙挡住玩家、地板承重、可攀爬建筑 | 世界内真实Collider/Udon接收器最直接；Avatar少量代理盒可另做验证 | 白名单允许Collider不代表任何世界里都具有玩家承重行为；碰撞层与世界配置会影响结果。Avatar粒子外观不能被当作可靠地形承诺 |

当前Store保存的是粒子位置/尺寸/颜色，Unity只为整个粒子系统保留组件，而没有给每一粒实例化GameObject。因此加一个Collision开关不能让16K方块成为16K实体碰撞盒。若继续Avatar路线，建议先给少量被选中的交互部件做8～32个代理盒实验，分别检查本机、远端、玩家、世界刚体，而不是把全量碰撞作为已具备能力。

## 三角面与楔形

可行，但不是从OSC上传Unity Mesh资产。建议分两级：

1. 预烘焙三角形、楔形、三角棱柱等模板，传shapeId、位置、尺寸和离散朝向；粒子出生时保留形状编码，由固定网格/Shader选择形状。这适合斜屋顶、斜坡、低多边形轮廓。
2. 任意三顶点三角面。8bit/轴、256³范围需要9字节顶点数据；若再保留24bit表面就是96bit载荷，超过现有单路72bit载荷。需要拆包/共享表面，或者传锚点及压缩相对顶点，再由Shader重建。UV、双面、法线和删除标识也必须设计，不能只增加一个枚举值。

Unity粒子Mesh渲染允许使用预制Mesh，但渲染模板并不自动产生碰撞体。当前Box布尔裁切、合并、容量规划、擦除都是轴对齐Box语义，任意三角面应单独建几何类型，而不是伪装成Box。优先验证模板楔形，再决定是否承担任意三角面的协议、编辑和碰撞成本。

## 更多通道的真实收益

VRChat同步参数总预算256bit，当前几何240+定位14=254。把三路80bit改名成更多路并不会增加带宽。现有每路9字节载荷+1字节Ctl，已经支持同外观两块单位体素合包。

| 方案 | 几何+定位 | 理想同外观单位体素（0.25秒） | 外观经常变化 |
|---|---:|---:|---|
| 当前MF5：3×80，自带外观 | 240+14=254 | 6块/批，24块/秒 | 每路不同外观也可发送；不能配对时12块/秒 |
| 4×80，自带完整外观 | 320+14=334 | 不满足预算 | 不可直接加入 |
| 候选：4×60，共享外观 | 240+14=254 | 8块/批，稳态32块/秒 | 每路换外观需要SetSurface批次，可能慢于MF5 |

4×60候选可分为48bit几何、8bitCtl、4bit外观代次检查；SetSurface沿同一路串行发送，Draw保存出生时的表面。两块位置正好48bit，长方体位置+尺寸也正好48bit。外观缓存不能直接读全局当前值，否则以前画出的块会再次变色。

若每次SetSurface之后连续发K条双体素指令，平均速度上限为 `32K/(K+1)` 块/秒：K=1为16、K=3为24、K=10约29.1。每块颜色都不同且只能单块提交时可降至8块/秒，低于现有MF5的12块/秒。因此下一档只应作为长颜色批次的候选，不能宣称必然提速。

此外要验证外观代次丢包拒绝、代次回绕、新加入者没有外观缓存、每轮重发SetSurface、8个粒子池及Animator额外开销。目前没有实现该候选，也没有更换线上Avatar。进一步提速通常应先看Box合并、共同外观分批、可复用模板与空闲通道利用。

## 新API：image_tools

统一通过现有 `/api/harness/tool` 调用，模型工具说明提供中英文，schema升级为3.8。旧上下文显式升级/新开才使用新工具，不重写历史前缀。不请求上游API。

| action | 参数重点 | 输出/行为 |
|---|---|---|
| list | 无 | 内置和本机参考图ID、尺寸与许可 |
| inspect | imageId，points，grid，crop，width/height | 精确RGBA8采样点；紧凑palette+indices网格。基于图库实际保存的规范化参考图（导入阶段最长边1024），不会修改磁盘上的上传源文件。左上原点，x向右/y向下；采样坐标针对裁剪/缩放后的图 |
| resize | imageId，width/height，crop，filter | 新参考图ID与URL，原图保留；只给一边则等比，最多1024边长；保留来源与许可 |
| voxel_plane | imageId，baseRevision，dryRun，width/height，plane，origin，thickness，alphaThreshold，maxColors，materialIds，canvasMode | 默认预览；显式dryRun=false才写入几何/材质，支持撤销与分组保护 |

- 平面每边1..256，默认把最长边缩到64。`xy`的u/v为x/y，正面+Z；`xz`为x/z；`yz`为z/y。图像顶行映射较大v，左列映射最小u。默认底部/地面居中，可传origin。
- `maxColors`默认256，范围1..4096；按像素频率做中位切分，避免照片的渐变被“前256个颜色”截断。可指定现有`materialIds`（1..256种），按代表色最近邻匹配；有贴图的材质用首面首帧去透明权重平均色与染色合成代表色。
- 默认`canvasMode=overlay`仅覆盖不透明像素，保留透明孔后原场景；`replace`替换整个矩形区域。Alpha阈值默认16，默认RGB把保留像素转换为不透明有色块，不假装支持原始连续Alpha；指定materialIds时保留该材质自身的透明度和表面参数。
- 同色横向像素合成长方体，再合并相邻行；不会强制每像素占一个Box。预览报告实际Box数、材质数量、边界/容量。原始图片和艺术颜色保留。
- RGB材质可直接发MF5；选择已烘焙材质ID可带其纹理。未知新图片作为Unity纹理仍要打包；本工具把像素变为现有Box，不是实时上传PNG纹理，也不是自动三维重建。
- 缩放图与平面PNG标记为reinspection；可给模型看，但不计作当前3D场景的Goal截图复核，避免模型仅看原图就宣布作品完成。

例：

```json
{"name":"image_tools","arguments":{"action":"inspect","imageId":"local:...","points":[[12,8],[20,8]],"grid":[8,8]}}
{"name":"image_tools","arguments":{"action":"resize","imageId":"local:...","width":64,"filter":"nearest"}}
{"name":"image_tools","arguments":{"action":"voxel_plane","imageId":"local:...","width":64,"plane":"xy","maxColors":128,"canvasMode":"overlay","baseRevision":17,"dryRun":true}}
```

应用时沿用同一参数和baseRevision，显式`dryRun=false`。图像内容或工程变化后重新预览。界面入口在左侧“图片像素 / 转体素平面”。

## 连续循环与更新顺序

新增Connection字段：`loopReplay`（默认false）、`sendOrder`（volume / bottom_up / material，默认volume）。保持旧配置可读。`publish`工具也可设置这些选项，但不自动开启OSC。

- `loopReplay=true`：目标发完后立即进入下一轮保留显示补播，保持同一写者、同一节流、同一六池归属，不全局Clear。空场景不会产生忙循环。关闭循环后当前轮完成即停。
- `refreshSeconds>0`：上一轮完成后等待相应秒数再补播；与连续循环区分，连续模式优先。
- 状态增加`replayCycles`与现有`repairing`，UI显示轮数和发送关闭状态。新编辑取消维护补播、优先发送最新差异，随后再循环新目标。
- volume：每个物理池按体积降序，再Y/X/Z；bottom_up：Y/X/Z，再体积；material：先线外观，再Y/X/Z，并做有界同外观配对。
- 三路并行、主/次池交错，不能保证跨所有路的严格全局左到右顺序。优先使用空闲带宽，避免为严格排序空置通道。
- 改颜色但几何相同：原池原位置重写。几何变化：**全路删除屏障**，先发送所有旧几何删除，再绘制新几何。解决“某路已经画新块，另一路还在删旧块”的问题。它是发送顺序保证，不是远端原子事务；低帧率/丢包仍可能损耗。
- 清空重建仍用于漏删残留。无远端ACK的盲补播能补漏画、帮助后来者收到完整作品，不能确认每位玩家已完整接收。全量替换可能暂时空缺；未来可做按更新区域的依赖分组，不能凭排序实现原子交换或在16K满池时双份保留。

## 后加入者的锚点姿态需要另验

当前14bit定位参数没有直接携带世界XYZ/旋转快照，定位球使用VRCParentConstraint的FreezeToWorld和抓放事件。因此本轮补播明确覆盖体素内容，不把几何重播完成视为后来者的世界原点也已一致。当前现场需要分别验证“方块是否齐全”和“原点是否一致”；重新抓放/重置定位可重新触发当前观看者的定位过程，但不是永久姿态同步方案。

后续应评估世界坐标/旋转快照，优先复用几何通道的低频控制命令，而非在已经254bit的配置里直接再加三到六个同步Float。本机姿态测量、编码精度、范围、完整快照提交和远端约束重建均需验证。已有同步WorldDrop项目可作为实现参考，尚未导入或替换当前系统：<https://github.com/gummidot/VRCWorldDrop>。不能同时无条件承诺更多几何通道和完整世界姿态同步都不付出成本。

## 单Avatar场景

`Mf5SingleAvatarScene.SelectOnly`只在所有已加载场景clean时以Single模式打开MF5场景，不丢弃用户未保存内容、不删除旧资产。实际已核验总加载Avatar数为1；SDK内部NDMF预览场景为0。上传/Play均明确选择此场景，避免多个GestureManager或Avatar互相接收。

## 官方资料

- VRChat Animator Parameters：<https://creators.vrchat.com/avatars/animator-parameters/>
- Avatar组件白名单：<https://creators.vrchat.com/avatars/whitelisted-avatar-components/whitelisted-avatar-components/>
- VRChat碰撞层：<https://creators.vrchat.com/worlds/layers/>
- PhysBones：<https://creators.vrchat.com/common-components/physbones/>
- Avatar Raycast：<https://creators.vrchat.com/avatars/avatar-components/raycast/>
- Unity粒子Mesh模板：<https://docs.unity3d.com/2022.3/Documentation/ScriptReference/ParticleSystemRenderer.SetMeshes.html>
