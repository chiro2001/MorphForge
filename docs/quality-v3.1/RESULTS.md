# 0.3.1 配置页与上游用量统计

主工作台保留模型摘要、预设、对话和运行操作。顶部「配置」或AI栏「配置」打开独立 `#settings` 页面，包含连接、模型、预算、Goal与自动compact；返回时保留草稿和几何。支持直接打开配置页及浏览器前进/后退。创作设置即时保存；连接设置仍通过保存按钮提交。

非DeepSeek官方接口强制内嵌图片，即使旧配置选择Files也自动转为inline，不调用专用上传通道。Files选项仅对HTTPS官方 `api.deepseek.com` 的Responses接口开放，Chat同样使用inline；前后端均执行该规则。

输入框旁显示当前数据目录的API累计Token、Prefix hit rate和本轮Token。累计包括模型循环及compact，按上游实际usage统计，与预算使用的本地估算分开。没有usage时显示未报告或已知总数的下限；这不是账单金额，取消/网络中断可能存在未报告费用。

缓存命中率按 `Σcached_input_tokens / Σinput_tokens_with_cache_metadata` 加权，不平均每次请求百分比，也不把缓存Token再加到总Token。支持Responses的 `input_tokens_details.cached_tokens`、Chat的 `prompt_tokens_details.cached_tokens` 和DeepSeek的 `prompt_cache_hit_tokens` / `prompt_cache_miss_tokens`；缺失显示未报告，明确返回0才算0%。悬停显示输入/输出、缓存命中分子分母和元数据覆盖请求数。

统计从已有请求事件恢复，内存维护汇总，页面轮询不扫描全日志。相同请求ID的完成/不完整事件去重；截断流已收到的usage也保留，工具执行仍必须满足完整响应条件。重启恢复已有日志保留的统计，不能补出旧版本已丢弃的字段。清空模型上下文不会清空API累计用量。

验证结果：

- Linux Rust 91项、Node 27项通过，Clippy `-D warnings`与格式检查通过；[摘要](automated-checks.json)。
- 原交互Chromium回归31项通过；[报告](regression-validation.json)。真实Rust + Chromium + 本机Chat服务16项通过，覆盖配置页切换保留草稿、第三方隐藏Files、真实建模/截图/入库及输入框旁4400 Token/60.0%缓存率；[报告](provider-presets-browser.json)。
- 重放此前21次真实DeepSeek请求，恢复379,453 Token、命中37,504/输入370,337，即约10.13%，与原始记录一致；[报告](real-usage-replay.json)。这不是新的上游调用。
- Windows核心91项、桌面2项通过；原生HTTP61项含新usage-ui.js字节核验通过，隔离Tauri启动/设置/退出13项通过；[原生](windows-native.json)、[桌面](desktop-candidate.json)、[包核对](windows-build.json)。
- 81项构建输入在Windows构建前后校验一致；[冻结清单](source-freeze.json)指向运行时代码提交 `c286f12`。NSIS解包核心与已验证核心相同，OFL许可与来源相同。候选包在Windows项目 `dist/windows/MorphForge Desktop_0.3.1_x64-setup.exe`；测试未运行安装器覆盖用户既有安装。
- README新增主页面和配置页WebP截图，旧验收图保留；0.3.0历史记录保留在相邻quality-v3目录。
