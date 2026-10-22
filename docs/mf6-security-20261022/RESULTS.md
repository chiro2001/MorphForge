# MF6 服务端安全检查修复

2026-10-22：**MorphForge MF6 16K 已更新到 Avatar 版本 10 / file 版本 8，generic、standard、security 三项均 HTTP 200 且 `success=true`**。仍使用原私有 Avatar ID，名称、可见性及几何容量不变。

```text
Avatar ID       avtr_9ebb3bdb-6991-42cd-8c3e-b9a35f176e90
File ID         file_29e323cf-176b-4353-bf4a-aedb8149e0a2
Avatar version  10
File version    8
Pending upload  false
VRCA bytes      12,621,120
Unpacked bytes  113,620,168（服务端报告）
VRCA SHA-256    6b348137e58ef8b6b9c6a035d6cdda54e6510fbeb797333ef46684400334f0fa
```

实际包 MD5 与服务器 file descriptor 一致。服务端 security 报告的 fileSize 为重封装后的 12,427,759；generic/standard 与原始上传包均为 12,621,120，不将这项历史上也存在的差异解释成失败。

## 原因与对照

客户端在 22:43 报 `AssetBundleFailedServerSideChecks`，安全接口拒绝下载。SDK 上传完成及 `pendingUpload=false` 只能证明上传完成，不能代替服务端检查。

| Avatar / file | 对照内容 | FX 参数 | FX 层 | 包内片段 | 安全检查 |
|---|---|---:|---:|---:|---|
| 5 / 3 | 历史成功版本 | — | — | — | 通过 |
| 6 / 4 | 新增文字、动画、预览的原始完整包 | 2794 | 136 | 3083 | 403 |
| 7 / 5 | 仅移除 Motion.controller 合并组件 | 2120 | 116 | — | 403 |
| 8 / 6 | 仅移除 Text.controller 合并组件 | 1541 | 128 | 1902 | 通过 |
| 9 / 7 | 文字层/片段移除，保留全部文字参数 | 2794 | 128 | 1902 | 403 |
| **10 / 8** | **完整功能，压缩内部缓存** | **1926** | **136** | **4071** | **通过** |

参数对照保留了字体、文字网格、动画、相机等资源；实际包审计确认零层文字控制器的参数也确实合并到最终 FX。由此确认内部参数规模是本次失败的充分触发因素。没有推导服务端的精确阈值，也不把性能评级 VeryPoor 当作安全检查结果。

## 最终实现

- Unicode、位置/尺寸、动画时间/四元数等 16 位数据改为一个本地 float 缓存，不再每个字节各存一个参数；删除未使用的缓存字段。
- 接收状态使用归一化混合树直接写缓存；每个字节的归一化值与补值一并原子锁存，避免放大其他显示属性。所有字值输出显式包含零值。
- 渲染输出同时持续保持这些缓存，避免退出接收状态或隐藏文本框时丢失已收页面。控制组启用状态、字符数及透明度负责隐藏显示。
- 不从动画曲线参数执行 ParameterDriver Copy；未改变 SDK、检查服务或网络协议。
- 私人及公共 Decoder 都更新了快照输入。公共 Avatar 包同步升级到 **1.2.1**，Desktop 保持 **0.22.0**。

内部 Animator 参数由 **2794 → 1926**。这与 **254/256 bit** 的网络同步预算是两个不同指标；几何仍为 **240 bit、30 个直接同步 Int**。八个池共 16,448 物理粒子、16,384 编辑容量，四个 128 槽文字框、三组动画和本人预览均保留。

用更多动画片段换取较少的参数，不以此宣称 CPU/FPS 提升。本轮没有重测 VRChat 客户端性能；实验用性能采样辅助脚本也已适配曲线缓存，避免直接改写缓存或关闭保持层造成数据丢失。

## 验证与证据

- [线上最终三项结果](analysis-file-8.json)：全部 HTTP 200、success=true。
- [实际上传包审计](full-fixed-v10-inventory.json)与[远端文件记录](file-record-final.json)：大小、MD5、参数数及同步预算对应同一个包，无自定义 MorphForge 运行脚本。
- [原生文字接收](text-receiver-play.json)：15 项通过，包括多页中日英、保留、字幕、代次与零值覆盖。
- [动画四阶段](native-motion-suite.json)：接收、时钟/暂停/继续/原子换库、GPU、清空恢复全部通过。
- [三角面与文字 GPU](native-motion-surfaces.json)：三角形宽 160→113、位移 64 像素；文字旋转到边视后消失。
- [单次动画](native-motion-once.json)：从半相位起播并停在末帧，GPU 宽 64 像素。辅助脚本使用新代次及票据，避免与前一组固定测试值冲突。
- [重建记录](cache-rebuild.json)：同步 254 bit，人物原有 13 个 Prefab 依赖哈希保持一致。
- [公共包审计](package-audit.json)：159 项均与仓库资源匹配，544 项公共材质、54 张原创 PNG，无私有 Avatar 绑定或用户材质变体。
- [收尾状态](final-editor-state.json)：源场景 Edit Mode、无未保存内容，SDK Build/Upload 均 Idle。SDK 临时状态已另存 Windows 验证快照。

对照只更新同一个已授权私有 Avatar，没有创建新 ID、启动 VRChat 或向 9000 发包。诊断与回归使用隔离的 Unity 9012。商业人物及私有 VRCA 留在工作站/私有临时目录；报告不包含登录信息或资产包解密字段。

实机下一步：先切换到其他头像，再切回 **MorphForge MF6 16K**，使客户端重新请求版本 10；随后按原验收步骤检查实际显示、头显及远端玩家。服务端通过不等于这些实机项目已经验收。
