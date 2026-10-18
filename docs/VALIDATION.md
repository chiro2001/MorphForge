> 新执行目标及未完成项见 [计划验收](plan/ACCEPTANCE.md) 和 [状态](plan/STATUS.md)。历史64 Box与基础Nemesis196bit不能作为16k/Full_FT完成证明。

# MorphForge验证状态与第一阶段清单

历史记录更新：2026-10-18。以下分别记录Stage2与Stage1历史验证。旧版客户端观察与新版Editor证据分开，不互相替代。

> 2026-10-19 的 Stage4 原生材质与容量边界另见 [native-validation](materials/native-validation.md)、[native-validation-status.json](materials/native-validation-status.json) 和 [capacity-validation.json](stage4/capacity-validation.json)。其中 Catalog 视觉、临时 16k 显示性能和 256 条短协议已执行；FullFT联合预算256/256bit与最终166项API烟测现已通过，见 [FullFT验收](fullft/README.md)；真实客户端GPU/多人和首次16k无损保证仍没有通过结论。

**下文历史版本为[Stage2/MF2](stage2/README.md)**：单次网络提交、一个本地待执行槽、盒体UV贴图。Stage1已由用户确认本机/其他玩家可显示，旧协议0.25s双阶段观察正常；以下Stage2结果为Editor验证，新版尚未上传客户端复测。

## Stage2验收

| 检查 | 结果与证据 |
| --- | --- |
| 单次提交字段、32序号、去重、原生绘制/擦除/长方体/容量回收 | [主回归](stage2/validation.json)1404项断言通过；130次发送包含1次重复序号，129次接收与完成 |
| 0.25s和0.2s整条命令的分阶段速率测试 | [扩展回归](stage2/extended-validation.json)186项断言；每档96条命令，每8条后核对完整几何和元数据；观测Game FPS约177.5/147.4 |
| 待执行槽保护工作缓存 | [队列隔离](stage2/queue-validation.json)通过；两次本地突发提交，观察到Busy期间InboxReady，两颗粒子的出生尺寸/元数据正确保留 |
| 无检查停顿的连续循环，0.25s/0.2s整条命令 | [循环回归](stage2/loop-validation.json)通过；每档3轮、74条命令，每一条完成时的原生粒子数量都与预期一致，完整覆盖第5/6级的生成、换色、删除 |
| 平移/偏航/三轴旋转/0.5和1.8倍缩放/组合姿态纹理稳定 | [纹理回归](stage2/texture-validation.json)通过；最大RGB平均误差约0.000051；旧Shader仅平移负对照误差约1.4215 |
| 本地执行耗时 | [主轨迹](stage2/core-trace.json)129个完成样本，接收锁存到完成约0.049–0.079s，中位约0.060s；是Editor观测，不是网络时延或客户端保证 |
| NDMF/MA产物 | [export](stage2/export.json)通过：Missing Script=0；3层；11个同步Int新增88bit，总177bit；原生子发射器/无限寿命/UV与尺寸流保持；Avatar自定义运行时脚本0 |

Stage2资源使用独立目录和MF2命名空间。构建期间保留用户原Stage1未保存场景；只生成/保存新版资源。原Stage1的Unity运行和客户端记录继续作为历史证据。

本轮还修复了生成资源重建后的控制器引用持久化：保留生成资源GUID，最终经Unity Prefab API显式保存Merge Animator引用；重建后模块、Avatar和NDMF产物均检查控制器/参数。姿态图见[新旧纹理对照](stage2/texture-comparison.png)。

主回归经独立Windows Python发送130条命令，1371项断言通过，保留34个粒子快照、13组斜视图及6个方向参考图。自动几何编译输出另有14项集成断言通过。几何编译器5项单测通过，其中500个随机盒体相减样本与独立体素集合核对。

主回归所有图像的平均RGB绝对误差小于0.002（通道范围0–255），六方向图像误差为0；参考图独立构造几何但复用同一Shader。预览见[截图总览](stage1/overview.png)。

## 已有证据

| 能力 | 证据/边界 |
| --- | --- |
| Unity/Nemesis/依赖 | [setup](setup/README.md)，Editor检查通过 |
| 外部UDP OSC→GM→实际FX→材质→Shader顶点 | [probe14](probe14/README.md)，39项Editor检查通过 |
| 8三角形载体外部造型 | GPU/独立CPU参考图一致；CPU载体未变 |
| 参数预算 | 当时MA/NDMF合并为89bit原有同步；探针75参数均local-only |
| 盒体拆分、位宽、时间公式 | 文档中的数学检查，不能代表Avatar运行 |

## 本轮开始前未实测的部分

粒子持久存储、不清空旧粒子的单次发射、整数载荷与工作缓存、长方体出生尺寸、精准删除/回收、材质按格平铺、运行中覆盖/清空，以及外部盒体拆分。本轮已完成下表范围；此前讨论不作为运行证据。

## 第一阶段验收

| ID | 检查 | 实测结果／证据 |
| --- | --- | --- |
| P1-01 | 独立场景/模块，经Unity API生成，不修改原Nemesis或Probe14 | 通过；[build](stage1/build.json) |
| P1-02 | 无Avatar运行时自定义脚本接收；实际走原生OSC和FX | 通过；[export](stage1/export.json)，发送报告在主回归中 |
| P1-03 | Int0/63/64/127/128/254/255复制到工作Float无夹取 | 通过；`integer-*`，沿X逐值测试并核对粒子中心 |
| P1-04 | 持续保存系统中重复单次发射，旧粒子保持且数量可控 | 通过；`cuboids`、`retained-without-osc`、`capacity64` |
| P1-05 | 单位体素位置/出生元数据保持，改变输入不改旧粒子 | 通过；整数边界序列、`cuboids`、`recolor` |
| P1-06 | 20×10×1和三维非均匀盒体的中心、尺寸、保留正确 | 通过；[cuboids](stage1/cuboids.json)，另测256³最大尺寸 |
| P1-07 | 六面可见且格纹理重复，不随尺寸拉伸成单图 | 通过；[六面报告](stage1/cuboid-faces.json)与斜视图；静止锚点范围 |
| P1-08 | 删除3³中间单位体素，26邻居保持 | 通过；[26邻居](stage1/center-erased-neighbors26.json)，区域擦除后17块 |
| P1-09 | 删除大小相邻盒体，无误删，删后不复活 | 通过；`small-erased-large-kept`、`wall-not-revived`；当前不暂停系统 |
| P1-10 | 同位置擦除后重建/改尺寸、重复提交不累积副本 | 通过；`overwrite`、`duplicate-sequence`、`recolor`、`resize-via-remove-set`；改尺寸显式删旧盒 |
| P1-11 | 原生清空与重新绘制 | 通过；多个clear快照及`final-clear`=0 |
| P1-12 | 64容量、超容量、删除释放与补回、无限出生寿命 | 本轮通过；64→63→64，第65条保持64；未做长时间压力或Avatar停用/恢复 |
| P1-13 | MA/NDMF产物保留接收器与参数，Missing Script=0 | 通过；11个同步Int新增88bit，总177bit；工作参数不入Expression；非SDK上传验证 |
| P1-14 | 图像、粒子快照、外部发送输入与结构报告落盘，可重跑 | 通过；[重跑入口](stage1/README.md#重跑验证) |
| P1-15 | 外部自动将区域挖洞编译为删旧盒／绘制碎片并通过OSC显示 | 通过；[自动拆分集成](stage1/compiled-validation.json)，最终5盒覆盖223格 |
| P1-16 | 参考SDK示例的持久观察环境，Scene/Game同时显示，世界坐标正确 | 通过；[观察检查](stage1/observation-validation.json)8项，8级阶梯范围(1,1,0)到(5,3,0.5)，[Scene](stage1/observation-scene.png)/[Game](stage1/observation-game.png)截图 |

本轮不通过Editor结果推断真实客户端或多人正确性。若原生发射/删除方案失败，应记录失败证据、调整原生机制再测，不用Avatar上的MonoBehaviour替代并报告成功。

## 仍未在Unity或客户端实测实现

| 方向 | 当前状态／下一步 |
| --- | --- |
| 可配置载荷通道、面捕预算适配、分片 | 只有设计；本轮固定11个同步Int，不支持动态缩减 |
| 控制面单阶段优化、吞吐和乱序恢复 | 只有保守双阶段功能测试，未证明0.2s间隔远端可靠 |
| 共享运动组、速度/轨迹/时间同步 | 只保存预留元数据，没有动画输出 |
| 外观资源包、GPU Instancing | 只有示例Shader，尚未实现与测试 |
| AI仿真、截图迭代、Draft/Published、版本队列 | 设计阶段；最小几何编译器不是完整后端 |
| 稀疏体素自动合并、跨轮持久化台账 | 当前只做盒体覆盖/相减；编译输入必须从clear开始 |
| staged一次性显示、双版本切换 | 尚未实现；删旧再补片目前是渐进可见 |
| 大容量/CPU与GPU性能、极端中心间距 | 首测64盒；当前2层并不等于CPU成本已实测合格 |
| Avatar移动、缩放、停用、重置与世界锚定 | 旧版客户端有旋转纹理游动；Stage2已在Editor修复并通过姿态对比。停用、重置、世界锚定和持续运动期间编辑仍待测 |
| 真实VRChat、多人、晚加入持续刷新 | 旧版两端均显示；0.2s双阶段有用户报告缺失，0.25s双阶段用户确认正常。MF2单次提交和新Shader已通过Editor，新版客户端待更新后确认；晚加入刷新未实现 |

## 操作约束

- 专门MorphForge工程，显式project path；不操作Roadster场景。
- 开始时Editor stopped、无未保存用户修改；后续测试只操作本轮生成的Stage1场景。
- 场景、Prefab、动画、材质、Mesh/Texture等序列化资源通过Pipeline/Unity API创建。
- 测试观察程序只读运行参数/粒子并渲染参考，不能代替实际接收器写参数或Emit。
- 测试自行进入Play Mode时负责退出；不接管已有Play Mode。
- 本轮不上传Avatar、不启动VRChat，后续真实客户端仍单列。

## 当前执行日志

- 2026-10-18：读项目/技能规范，确认目标Unity2022.3.22f1、MorphForge、Play Mode stopped、场景干净；准备原生粒子机制调查与第一阶段构建。
- 2026-10-18：原生子发射器机制调查成立，独立触发器停用不清空保存系统。实际UDP Int→GM→FX→出生参数生成单位块和5×2.5×0.25m长方体，第二次输入后第一块保持。尚待完整回归。
- 2026-10-18：发现Transform分轴层覆盖其他轴，改为WD On单状态Direct BlendTree完整基向量输出，控制层WD Off，总2层。
- 2026-10-18：Ring Buffer擦除后保留零寿命记录；替换为普通模式+原生PositiveInfinity寿命，最终64→63→64回收实测通过。早期`removed-wall`与`investigation`文件为调查记录，不是最终通过证据。
- 2026-10-18：图像对比发现默认View对齐与线性色彩转换破坏结果；Local对齐并关闭Renderer颜色空间转换后，几何和元数据图像回归通过。初期Pipeline eval发生5秒主线程超时，测试改为版本化run_script入口并显式设置60秒上限。
- 2026-10-18：完整Editor回归1371项断言、自动拆分集成14项断言、几何编译5项单测及NDMF导出检查通过；未启动VRChat、未上传Avatar。
- 2026-10-18：用户现场确认Game演示正常。随后修复Scene视角偏离、Gizmos关闭带来的观察问题；参考SDK Robot Avatar PC示例保存主相机、方向光，并补地面/坐标轴；恢复普通图层观察，Scene/Game截图与世界坐标检查通过。保留Play Mode供用户检查。
- 2026-10-18：用户自行上传并切换Avatar，外部向VRChat本机9000端口发送20条演示命令。用户确认本机与其他玩家均显示方块；报告Avatar转动时纹理变化，记录为待修复问题。按用户要求启动持续循环：准备0.2s、执行0.2s，一条完整命令0.4s，每轮8柱绘制、换色、逐个删除；不将发送端正常运行视为远端可靠性通过。
- 2026-10-18：0.2s时隙循环约12轮后，用户报告第5/6级可能在快速删除再绘制时缺失。停止旧发送器后，以相同图案重新启动0.25s准备+0.25s执行（完整命令0.5s），等待现场反馈；尚未区分网络同步丢失与接收器Busy/执行耗时。

## 2026-10-19 Stage3 与网页编辑器

最新范围见 [Stage3](stage3/README.md) 与 [网页编辑器](editor/README.md)。以上早期“未实现”表格按阶段保留；本节覆盖定位、手动草稿和发送台账的新进度。

| 项目 | 证据与边界 |
| --- | --- |
| 底部中心、世界固定／跟随、Avatar平移旋转、手部位移、抓放保姿态、持握中Reset、隐藏／缩放 | [原生约束测试](stage3/placement-validation.json)，18项检查 |
| 左右手原生Contact+握拳、离开范围保持、张手释放、远端输入状态 | [抓放测试](stage3/grip-validation.json)，临时原生Sender和手势刺激，非真实手柄／远端客户端 |
| 1／0.25／0.05缩放几何与格纹理 | [缩放截图测试](stage3/scale-validation.json)，随动相机图像与BakeMesh；Local模式反例 |
| MA菜单、参数、约束、上传产物无自定义运行脚本 | [NDMF处理](stage3/export.json)，196/256bit；非SDK上传成功证明 |
| 浏览器放置、Shift擦除、撤销重做、自动合并与示例 | [真实Chromium操作](editor/browser-validation.json) |
| 网页→Windows服务→真实UDP9012→MF2→原生粒子 | [联调](editor/unity-integration.json)，含长方体、底面、缩放、开关、区域挖洞和阶梯 |
| 稀疏盒体模型、节拍发送、持久草稿 | [8项测试](editor/model-tests.txt)，含250次随机操作的独立稠密格比对、容量拒绝和收敛 |

未上传Stage3、未启动真实VRChat。世界位置仍由每端本地约束求解，不包含精确全局pose同步或晚加入位置恢复。主约束offset动画会破坏rebake的问题已改为独立ResetGuide；原生Renderer.bounds与实际渲染尺寸不一致，尺寸验证改用BakeMesh和图像，极限裁剪边缘仍未专项测试。

清理仅重载本次生成Stage3场景；用户原有Stage1未保存修改必须保留。网页收尾停发、选择9000，服务保持可用。

## 2026-10-19 Stage4 原生材质收尾

| 检查 | 结果与证据 |
| --- | --- |
| Catalog构建与Prefab接入 | [Finish报告](materials/native-finish.json)通过；100材质、Atlas 384×96、Fire 0.03s、Pump 0.5s，Geometry/Placement/Stage4 Avatar绑定Catalog材质 |
| 严格像素回归 | [Strict报告](materials/native-strict-validation.json)40项通过；显式动画蓝→红→紫、六面颜色、金属/水/云/cutout可见、缩放和根刚体变换后局部UV稳定 |
| 16k Catalog性能fixture | 16,384粒子可见，657样本p50 8.92ms/p95 11.80ms；[报告](materials/catalog-16k-performance.json)明确为Editor临时rig，不是UDP、产品Store或客户端基准 |
| 真实UDP短回归 | FireHold实验前[主协议](materials/protocol-validation-prefirehold.json)166项通过；[第一次256×0.25s](materials/protocol-regression-256-attempt1.json)少1颗粒子，[第二次](materials/protocol-regression-256-attempt2.json)和[0.5s对照](materials/protocol-regression-256-interval050.json)达到256/256 |
| FullFT边界 | 基础Nemesis+MF3 NDMF为159bit，0 Missing Script、0自定义Avatar运行脚本；FullFT必须另做161bit面捕保护下的联合preprocess，不能引用该159bit结果 |

FireHold曾用于规避偶发漏Birth，但两次定向试验都停在Fire/Busy，已从产品撤销，失败报告保留。最终产品恢复`Fire 0.03s → Done`；最后一次撤销后的Edit Mode检查确认3层与状态路径，但没有重跑整批主协议，因此不能把恢复后的资产写成全量重测完成。原用户场景在开始时已dirty，Finish没有保存或修改该场景。
