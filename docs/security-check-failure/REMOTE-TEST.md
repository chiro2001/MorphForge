# 本轮远程上传授权与核验边界

日期：2026-10-19。用户离开电脑后，明确授权本轮代为登录VRChat官方网页、上传/更新MorphForge诊断模型并查看是否被禁用。这覆盖本轮此前默认“不自动上传Avatar”的限制；不是对其他模型或以后任务的通用授权。

当前执行范围：先将独立4096Probe上传为新的私有Avatar，不覆盖原人物ID。Unity原dirty Stage3保留，正常MorphForge桌面程序保留，不启动VRChat客户端，不发送OSC。

用户提供的密码不进入源码、配置、日志、报告或Git；没有在浏览器页面提交密码。官方网页登录遇到Cloudflare人机验证，正常浏览器等待后仍未显示登录框，临时浏览器已关闭且profile清理。

官方SDK仍有已验证的同账号会话，canPublishAvatars=true，可执行SDK公开的正常BuildAndUpload；因此当前无需新的账号密码或2FA。既有登录会话用于当前授权操作，不导出或转移cookie/token。

上传成功不能当作服务器安全检查通过。将使用已知正常自有模型作同接口对照；若状态接口权限不足，则明确记录“无法从该接口判断”，不能把403直接当新模型失败。后续上传和状态结果记录在`upload4096.json`。

## 编辑器恢复授权更新

用户随后明确表示场景没有需要保存的内容，可以不保存，并要求尝试截图和鼠标键盘处理后继续。这覆盖本轮此前“不得关闭dirty场景”的限制。

上传前诊断脚本曾错误地在主线程对包含`Task.Yield`的账号只读任务执行同步`GetAwaiter().GetResult`，导致旧Editor等待自身的continuation。此时尚未建立upload操作或新AvatarRecord。已修正为async/await，并将纯账号快照改为`Task.FromResult`，不再强制Yield。

恢复过程：Session1窗口枚举只发现主窗口；截图未能取得Unity自身画面，不能作为Unity内部UI检查的证据。向该窗口发送WM_CLOSE后原PID46280仍未退出。依用户明确不保存授权，仅结束此已核实项目路径的PID，重新启动同一MorphForge工程（新PID141832）。没有执行线程Interrupt、原生注入或进程内存dump，没有启动VRChat。新Editor通过Pipeline报告ready、compiling=false、playMode=stopped，后续从已保存4096测试场景继续正常SDK流程。
