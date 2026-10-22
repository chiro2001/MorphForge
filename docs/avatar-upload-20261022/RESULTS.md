# 当前MF6头像构建上传结果

2026-10-22，用户明确要求“构建并上传当前Unity头像”后，已使用MorphForge工程内VRChat SDK 3.10.5公共`BuildAndUpload`接口完成Windows头像更新，并二次强制刷新读取线上记录确认。

| 检查 | 结果 |
|---|---|
| 头像 | MorphForge MF6 16K |
| 线上版本 | **5 → 6** |
| ID、名称、可见性 | 原有ID及名称不变，仍为private |
| 服务端待上传状态 | pendingUpload=false |
| 构建环境 | Unity 2022.3.22f1，StandaloneWindows64，SDK 3.10.5 |
| 几何 | 8个池，物理粒子16,448，逻辑编辑容量16,384 |
| 本轮文字/动画/预览 | 4个文字Renderer、分组动画控制器、512×384 RenderTexture本人预览相机均在源头像中 |
| 本机能力标志 | 127，包含文字和动画能力及私人材质目录标志 |
| 编译内容检查 | 0个缺失脚本，MorphForge相关Shader错误0个 |
| SDK最终同步预算 | 254/256bit；几何数据240bit直接同步，无几何参数压缩 |
| Unity收尾 | MF6源场景Edit Mode、无未保存修改；原Prefab依赖哈希与上传前一致 |

完整结构化证据见`result.json`，包含上传操作ID、前后线上记录、最终预算和场景恢复结果。源码入口为`AgentScripts/AvatarUpload20261022.cs`。

SDK上传前显示版权归属确认；按本次明确上传授权及项目已有Nemesis资源范围，触发SDK当前头像确认窗口的正常按钮回调，随后继续SDK构建流程。未修改SDK、服务端权限或同意缓存。

SDK自动保存了源场景，并在构建后留下NDMF预览/Activator临时脏状态。通过Pipeline先保存独立Windows快照`Assets/MorphForge/Validation/Upload20261022-AfterSdk.unity`，检查不存在额外用户场景、源Prefab依赖哈希不变后，再重新打开已保存MF6源场景。未把临时预览状态写回源场景，快照和用户商业资产留在Windows工程。

本轮未启动真实VRChat客户端、未执行头显或远端玩家验收。SDK上传成功与线上记录回读不代表头边预览舒适度、第一人称/镜子/拍照和远端网络表现已通过实机测试。桌面应用仍为已安装的0.22.0。
