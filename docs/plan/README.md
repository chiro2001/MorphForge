# MorphForge 已确认方案与实施基线

更新：2026-10-19。状态：用户已授权实施并创建长期目标；本文记录已确认需求，不将计划写成完成。原Stage3/MF2、64 Box、Python网页原型作为历史基线保留；Rust/MF3/16k与桌面实施状态以测试证据为准。执行证据在 [STATUS](STATUS.md) 与 [验收矩阵](ACCEPTANCE.md) 持续登记。

## 产品定位

MorphForge 是独立的体素艺术生成器、作品浏览器和编辑器。Rust核心必须独立在Windows运行；人工编辑和AI Harness使用同一前端、领域API、文档和材质定义。无需运行Unity或VRChat即可生成、浏览、编辑、截图、存档。Tauri最后组装，窗口不是权威状态存储。

VRC是可选发布适配器。256³逻辑空间、0.25m格、12bit样式编号、56bit数据通道和Avatar粒子容量属于适配器能力，不写死在通用业务核心。16,384 Box是容量/性能验证目标；约5k Box完整作品是日常基准，不将64继承为产品上限。用户后续要求“先准备4096容量对照模型”属于安全排查的对照变量，没有将产品容量降为4096。

## 已确认决策

1. Rust核心、独立HTTP/CLI服务、共享网页前端先分离开发，最后组装Tauri并交付Windows可运行产物。
2. Harness调用DeepSeek官方API，支持真实工具循环、多视角反馈、连续输入、暂停/停止/恢复、预算和上下文compact。
3. 完整原始运行记录保留；compact只改变活跃模型上下文，不删除日志、图片、recipe、版本或作品。
4. 作品库支持保存、搜索、预览、加载、分支、继续创作，保留源recipe、seed、材质版本和成品快照。
5. 几何主档56bit：6个数据Int（位置3+尺寸3），一个8bit控制头。外观/运动元数据作为共享刷子单独设置。新协议命名MF3，与MF1/MF2显式区分。
6. FT manifest（105 Bool + 7 Float）固定按**161bit**预留，不替换或精简版本；另8个VRCFT控制Bool计入其余25bit。MorphForge几何56+定位14=70，给其它本体状态及压缩承载留**25bit**；联合构建最终必须<=256。不得继续套用“基础本体89bit后剩97bit”的旧结论。
7. 其它本体可将不用功能改为本地，或将低频外观纳入串行；必须审计参数用途/来源/远端影响，不凭名字前缀误关舌头及必要联动。压缩后不是按原始bit等额回收。
8. MA提供MorphForge模块、FX、菜单和原生约束；保留VRCFury面捕与低频压缩。只由一个压缩器管理同一批状态，几何命令不交给会合并中间状态的普通压缩器。
9. 定位球为底部中心，默认世界固定，可拿取、复位、显示和缩放；新增水平开关消除pitch/roll并保留yaw。抓取Int改两Bool后，含水平开关定位预算14bit。
10. 修正编辑器/Unity方向一致性，支持自定义OSC目标端口；用户当前单Avatar场景必须保留。测试接收器应进入Play即可初始化，不依赖固定对象名称和手动run_script。
11. PC/PCVR接受Very Poor。评级线不是粒子容量硬上限；Particle Limiter、客户端设置、实际帧时间与回收正确性分别验收。
12. 材质涵盖基础、自然、建筑木材、金属、自发光、裁切、云、水、玻璃冰、烟火。Minecraft首先支持Java整立方体资源包；新贴图在编辑器即时预览，在Avatar使用须预打包上传。
13. 默认不自动上传Avatar、不启动VRChat；2026-10-19用户已明确授权本轮SDK上传/更新本项目私有诊断Avatar和服务端查询，仍不启动VRChat或发送9000 OSC。真实客户端与多人表现不能由Editor或服务端分析冒充。

## 最终预算落地

完整面捕固定按 161 bit 计算。实际 VRChat SDK preprocess 在 FullFT 构建上得到：

| 项目 | bit |
|---|---:|
| 面捕 112 项（105 Bool、7 Float） | 161 |
| VRCFT 控制 Bool | 8 |
| MF3 几何（6 数据 Int + Ctl） | 56 |
| 定位、缩放、水平与抓放 | 14 |
| 其余本体 | 17 |
| **合计** | **256 / 256** |

112 项面捕全部 direct，MF3 与定位全部 direct，VRCFury compressor candidate 为 0。
“装得下”已经由实际联合构建报告确认，而不是从旧的基础本体 89 bit 估算：

- [最终预算明细](BUDGET-AUDIT.md)
- [最终预算机器报告](../fullft/final-budget.json)
- [FullFT 上下文、场景与烟测](../fullft/README.md)

这里的 17 bit 是当前 FullFT 组合保留给其余本体的实际额度。未来恢复已本地化的
本体功能时，必须重新审计用途、远端影响和联合构建，不从压缩前参数大小直接计算
可回收额度，也不能占用面捕 161 bit。

## 当前执行边界

Rust核心、共享前端、Harness、MF3发布适配器、材质目录与Windows桌面安装流程
已完成当前0.2.0范围内的自动验收。2026-10-19额外完成两项Avatar修复：

- 原16384×1 Catalog metadata触发服务端拒绝；完整256×64布局在同一个私有
  诊断Avatar的64、4096和16384容量对照中均得到三项200。
- 生产生成器已改为256×64，保留4096样式、全部数据和资源GUID。正式材质16项
  像素检查及16k显示夹具通过。
- FullFT场景根名前缀已修正，实际场景clone构建256bit；直接Play远端面捕分支
  再次产生18项变化，无睡眠依赖。此前仅给JointBuild临时对象改名的覆盖缺口已补齐。
- 最终FullFT生产包已更新为私有`MorphForge FullFT 16K`，Avatar v10/file v8，
  实包参数逐项解析与服务端三项200均通过。原人物RailGun粒子的Pickup碰撞力也已在
  最终副本及生成器修正，Nemesis源保持不变。真实客户端、多人和硬件抓取仍待现场确认。

详见[安全排查记录](../security-check-failure/README.md)、[场景修复](../security-check-failure/FULLFT-NAME-FIX.md)
和[执行状态](STATUS.md)。

## 文档索引

- [2026-10-22 下一轮文字、语音与交互需求（待实现）](../text-animation/REQUIREMENTS.md)

- [架构与前端](ARCHITECTURE.md)
- [Harness、日志、作品库、compact](HARNESS.md)
- [材质与Minecraft](MATERIALS.md)
- [VRC协议、预算和集成](VRC-PROTOCOL.md)
- [实施/验收](ACCEPTANCE.md)
- [执行状态](STATUS.md)

## 调研来源

- 用户提供 `E:\tmp\morphforge-harness\AI-VOXEL-HARNESS.md`（2026-10-19）；吸收recipe、事务、多视角、实例化渲染、简短工具结果，修正“HTTP bulk直接送Avatar”的边界。
- 用户提供 `DesktopSyncData\avtr_9404e9b5….json` 上传统计（2026-10-03）：原始289bit、现有压缩后256bit。此上传档案与基础Nemesis89bit不是同一预算。
- 旧作品 `E:\tmp\voxel_art` 有源码、独立HTML及多视角图；导入时分别统计原始体素和合并Box，不混用。
- DeepSeek官方：<https://api-docs.deepseek.com/guides/tool_calls/>、<https://api-docs.deepseek.com/guides/files_api/>、<https://api-docs.deepseek.com/guides/vision/>、<https://api-docs.deepseek.com/guides/responses_api/>。
- VRChat：<https://creators.vrchat.com/avatars/avatar-performance-ranking-system/>、<https://docs.vrchat.com/docs/avatar-particle-system-limits>、<https://creators.vrchat.com/avatars/animator-parameters/>。

凭证来源记录为Linux `~/litellm/.env`，具体变量由后端解析；密钥不得写入仓库、运行日志、前端或文档。Windows使用其本机环境变量/配置路径，不假定能访问Linuxhome。
