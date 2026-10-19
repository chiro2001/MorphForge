# Stage3 人物上传后服务端安全检查失败

状态：已确认失败阶段，尚未确定具体命中规则。用户确认目标为 `MorphForge_Stage3_Avatar`，不能拿最终FullFT的Editor/SDK报告替代此次上传包。

## 已确认

- 2026-10-19 08:07:25/35/37，客户端在下载前拒绝Avatar，错误为`AssetBundleFailedServerSideChecks`，安全分析API返回403。详细[客户端证据](client-evidence.json)已删去无关用户、房间和聊天信息。
- Avatar `avtr_3ba403cb-569d-4198-8de6-678e59c4440f`；文件`file_d68d4ea2-015f-40cd-bd19-444a2e49b24a`第5版。
- 最新实包大小7,519,081字节（7.17 MiB），解包57,330,700字节（54.68 MiB）。与公开PC文件大小硬限相比有很大余量，普通文件大小问题不是当前优先嫌疑。
- 实包包含8个AnimatorController资产、合计57层/437状态/960转场，243个唯一AnimationClip。FX44层，其中MF3本身3层。控制器资产数、Animator组件数和控制器层数是不同指标。
- 实包仅2个粒子系统：Pump容量1、寿命0.5秒；Store容量16384、寿命正无穷、单次burst1。Infinity是Unity官方支持的永久粒子表达，不能直接称非法值；VRChat对此资产组合是否接受仍需验证。
- 同一Avatar ID在02:29曾正常加载，但当时文件版本、Store容量与寿命尚无实包证据，不能据此推导当前第5版理应通过。

完整静态审计见 [stage3-audit.json](stage3-audit.json)。当前没有上传、没有修改原人物或粒子资产、没有清缓存，没有改用户未保存场景。桌面手测程序保持运行。

## 当前判断

公开的性能评级表不能给出失败的精确规则：Animators一栏统计组件数，Very Poor也不代表通过服务端扫描。实际FX44层和243个Clip不足以单独证明动画器违规；没有证据支持“层数超过32就无法上传/加载”的结论。

新增前后包差分：最接近的旧本地VRCA中Store容量64，最新16384；寿命同为正无穷。旧FX为42层/366状态/801转场，最新44层/344状态/752转场，控制器整体runtime体积下降。18类MonoScript全部为VRChat SDK，没有MorphForge或GestureManager运行脚本。Locomotion非有限值及Mesh AABB哨兵在新旧包一致，不能当成本次独有缺陷。

目前优先检查容量变化，其次是新Catalog材质/Shader/16384×1 RGBAFloat metadata纹理的兼容性，再考虑服务端误判。此排序来自差分，不能称16384已被确认触犯硬限。旧本地VRCA不等于对02:29云端实际加载字节的签名证明，所以“旧版本可加载”的对应仍有这一限制。

## 建议对照顺序

| 步骤 | 相对原第5版的唯一变化 | 目的 |
|---|---|---|
| A（用户最终选择） | 保持无限寿命、材质和全部控制器，仅将Store容量降为4096 | 检查容量变化 |
| B（仍失败时） | 在原包或A的独立副本中仅替换Catalog渲染资产为旧LocalUV，几何协议保持 | 检查新渲染资源组合；实际可行性需另核对 |
| C（另需时） | 单独生成有限寿命副本，不同时降容量或换Shader | 检查寿命兼容性，优先级因旧包同值而下降 |

这些对照应由Pipeline生成到独立诊断资源，不覆盖原人物。仍需用户手动上传并等待服务器扫描；SDK Build/Test或Editor运行不能替代这一结果。若有限寿命方案通过，再设计寿命到期前的刷新续存，不把3600秒临时值直接宣称为最终永久存储方案。

[官方参考与阈值的适用范围](sources.md)。

## 4096 对照准备

用户最后明确选择4096容量，已撤回有限寿命方案。本次新建独立对照资源，只有Store容量更改；保留+Infinity，不改共享原资产。身份解绑只用于独立测试上传，不能当作运行差异。已生成并验证，见 [probe4096.json](probe4096.json)：1395个粒子/渲染器叶子字段对照，只有Store容量差异；控制器和材质引用不变。

测试场景已打开并激活，选中 `MorphForge_Stage3_Avatar_4096Probe`。路径为 `Assets/MorphForge/Diagnostics/Security4096/MorphForge_Stage3_Avatar_4096Probe.unity`。原Stage3仍loaded+dirty，磁盘和受保护共享资源hash保持不变。副本Blueprint ID为空，请用户手动上传为独立私有Avatar，先检查能否切换。当前未构建或上传，因此服务端是否通过仍未知。
