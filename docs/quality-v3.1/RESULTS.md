# 0.3.1 配置页与上游用量统计

主工作台保留模型摘要、预设、对话和运行操作。顶部「配置」或AI栏「配置」打开独立 `#settings` 页面，包含连接、模型、预算、Goal与自动compact；返回时保留草稿和几何。支持直接打开配置页及浏览器前进/后退。创作设置即时保存；连接设置仍通过保存按钮提交。

非DeepSeek官方接口强制内嵌图片，即使旧配置选择Files也自动转为inline，不调用专用上传通道。Files选项仅对HTTPS官方 `api.deepseek.com` 的Responses接口开放，Chat同样使用inline；前后端均执行该规则。

输入框旁显示当前数据目录的API累计Token、Prefix hit rate和本轮Token。累计包括模型循环及compact，按上游实际usage统计，与预算使用的本地估算分开。没有usage时显示未报告或已知总数的下限；这不是账单金额，取消/网络中断可能存在未报告费用。

缓存命中率按 `Σcached_input_tokens / Σinput_tokens_with_cache_metadata` 加权，不平均每次请求百分比，也不把缓存Token再加到总Token。支持Responses的 `input_tokens_details.cached_tokens`、Chat的 `prompt_tokens_details.cached_tokens` 和DeepSeek的 `prompt_cache_hit_tokens` / `prompt_cache_miss_tokens`；缺失显示未报告，明确返回0才算0%。悬停显示输入/输出、缓存命中分子分母和元数据覆盖请求数。

统计从已有请求事件恢复，内存维护汇总，页面轮询不扫描全日志。相同请求ID的完成/不完整事件去重；截断流已收到的usage也保留，工具执行仍必须满足完整响应条件。重启恢复已有日志保留的统计，不能补出旧版本已丢弃的字段。清空模型上下文不会清空API累计用量。

本轮验证与Windows候选构建结果追加于此；0.3.0历史记录保留在相邻quality-v3目录。
