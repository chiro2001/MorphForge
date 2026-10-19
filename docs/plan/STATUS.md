# 执行状态

更新时间：2026-10-19。软件质量迭代及Windows自动验收已完成；新增Stage3客户端安全检查失败正在诊断，整体交付尚未收尾。完成项以测试证据为准。完整入口见 [交付记录](../DELIVERY.md)。

## 0.2.0 质量迭代

本节为当前交付入口；后文保留0.1.0的协议、容量与材质证据。

- Git建立基线与阶段提交，开发分支 `feat/harness-interaction-and-voxel-text`；发布版本0.2.0，源码未推送远端。
- Harness安全点插话、立即打断并同run续接、排队、暂停冻结活跃预算、恢复与停止；严格工具配对、部分SSE拒绝、epoch提交守卫及幂等回放。
- 真字体转普通体素Box，内置许可完整的Noto Sans SC，支持只读系统字体、三平面/挤出/排版、预览/应用/撤销、跨重启回执。中英文system/tools版本2.3，中文优先。
- 用户可保护分组，所有几何修改与历史/作品载入均验证；`finish_artwork`提交当前版本、关联真实截图、入库并形成完成摘要。Token预算以Provider实际usage计量。
- Linux核心74项、Node24项、浏览器mock31项；真实文字预览24项、Harness UI17项、保护25项及手工5k/16k回归14项通过。
- 多轮真实DeepSeek评测保留失败与修正：R2c收尾766 Box、R3c正面修字791 Box通过；不能称为复杂艺术一次提示自动完成。两份作品及三视图已加入Windows作品库，原项目未变。
- 最终FullFT prefab/上传场景/生成入口无睡眠插件依赖；预算保持256/256，FT161、几何56、定位14。未上传、未启动VRChat，原dirty场景保留。
- Windows69个冻结输入与NSIS安装器对应；原生74项、桌面2项通过，隔离原生HTTP59项、桌面安装/启动/实际嵌入资源/正常退出/卸载26项均通过，清理结果逐项记录。

[质量结果](../quality-v2/RESULTS.md) · [真实模型迭代](../harness-v2/evals/README.md) · [Windows 0.2验收](../quality-v2/WINDOWS-V2.md) · [无睡眠审计](../avatars/README.md)

## 0.1.0 已实现并验证

- 计划、架构、Harness、材质、56bit协议、验收文档已落地。
- Rust稀疏几何：精确切割、合并、分组变换、事务、旧格式导入。随机密集oracle与16k不可合并Box已验证，Linux与Windows原生通过。
- 共享前端：真实鼠标绘制/擦除/撤销、非对称坐标、5k/16k实例化与截图；Windows后端16k为7次绘制调用，三视图约0.65s。当前还支持GPU批次复用和脏实例更新。
- Harness：真实DeepSeek工具循环完成42 Box灯塔作品；compact约8937→1568估算Token，续写后原42 Box全部保留，并新增树。详见`docs/harness/`。
- 原始日志、不可变scene与材质版本、作品库、诊断ZIP、工具延迟/用量统计；bounded Recipe子进程、Windows Job内存限制与16MiB引擎栈。
- 36项Linux Rust测试（含1000材质分页与严格导入）、5项前端资源辅助测试；独立WebGL像素验证六面方向、显式动画帧序、逐帧时长及插值。
- 旧完整作品精确导入：实际70,286个唯一体素→7,459 Box；三份作品及9张多视图已复制到Windows默认作品库，未覆盖现有工程。
- MF3真实UDP9012→原生动画器→原生粒子回归：Box/UnitVoxel/Brush/Erase/Remove/Clear、覆盖、重复序号忽略、40条连续命令的序号回绕。
- 单Avatar场景Play自动接收；基础Nemesis联合构建为几何56+定位14bit，上传产物无自定义MorphForge运行脚本。**基础合计159bit不是Full_FT预算**。
- 保持水平通过16项Editor约束测试：近竖直Anchor、关闭后恢复完整旋转、固定世界朝向、抓取Bool桥接。
- Tauri最终安装包已从含严格Scene导入保护的冻结源码无`ReuseSidecar`重建并实际安装/启动/退出/卸载；Windows核心35项、桌面2项、随机端口HTTP10项、16384操作recipe和7459 Box WebView三视图证据均通过。sidecar `1951AE5D…2182CC`，installer `CC535107…50F3FF`，Linux发布副本已同步。

## 16k 容量与完整发送已验收

16,384条真实UDP绘制已发送完毕，0.25秒/条，4105.53秒。该混合修复版本首遍16,192个，缺192；接收/完成较目标少125条，位置/尺寸/metadata错误均为0。以0.5秒补发193条（刷子+缺口）后，达到**16,384唯一Box、零畸形/零metadata错误**；满池remove16→refill16→区域erase64→clear全部通过。首遍无损结论为false，容量/补发/回收结论为true。

[详细记录](../stage4/README.md)和[完整报告](../stage4/capacity-validation.json)保留首遍失败及补发证据。两个原生二进制快照的SHA-256已复核。满容量Editor432帧p50 6.84ms/p95 8.29ms，不能冒充客户端GPU性能。

短寿命Pump在长帧漏Birth已独立复现并将寿命改为0.5秒；正式资产另延长Fire到0.03秒。无远端ACK边界保持。预填轮不作为OSC证据；未向9000发包，未上传Avatar或启动VRChat。

## 原生材质与最终协议回归已验收

- Unity构建100材质目录，Atlas384×96，metadata16384×1，Stage4几何/定位/Avatar已绑定Catalog材质；保留必要粒子流。
- 严格40项像素检查通过：六面、三帧顺序、云/水/金属/cutout可见、0.5缩放和根刚体变换。缩放与根变换的局部逐像素MAE均为0。
- 新双Pass材质独立16k混合显示夹具通过：p50 8.92ms/p95 11.80ms（Editor整体帧，非OSC、非客户端GPU基准）。
- 新Fire0.03/Pump0.5原生主协议166检查通过。256条0.25秒首次255粒子（rx/done均256，漏Birth1），第二次256完整；0.5秒对照256完整。失败记录保留，不承诺无ACK绝对无损。FireHold试验未保留，正式资产仍Fire→Done；撤销后的最终Fire→Done已在FullFT组合中重跑50条有效命令/166项协议检查（0.5秒）通过；未再重跑完整16k无损长测。
- 基础NDMF产物无missing script及MorphForge自定义运行脚本，几何56/定位14、容量16384。基础159bit不能冒充FullFT预算。
- [材质验收](../materials/native-validation.md) · [机器可读状态](../materials/native-validation-status.json)。

## FullFT预算、场景与功能烟测已通过

实际SDK联合构建已经通过 **256/256bit**：FT清单112项全部直通161bit，MF3几何56bit与定位14bit全部直通，8个FT控制Bool全部直通。其余本体17bit，合计256；最终产物没有压缩carrier，通过本体同步本地化完成精简，不能称依赖FT串行压缩。最终门禁新增FT allDirect检查并再次通过。

独立 `MorphForge_FullFT.unity` 已生成并验证单Avatar、GM/Receiver引用和9012端口；CreateScene通过save-as-copy捕获dirty源场景，源文件SHA不变、仍打开且dirty。完整UDP协议烟测166/166通过，远场 `IsLocal=0` 面捕输出18项变化。Play后隔离对象全部恢复，新场景保存并关闭，文件保留。

最终清理只终止了归属明确的8766测试服务与capture Edge进程树，Unity PID46280和已运行的VRChat进程未触碰。原Nemesis资产只读；真实客户端/多人上传证据仍需用户后续操作。
