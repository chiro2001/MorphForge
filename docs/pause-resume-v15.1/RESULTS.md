# 暂停与继续修复：0.15.1

2026-10-21。仅部署gamepc“MorphForge - 开发测试副本”，不创建GitHub Release，不替换或关闭正在使用的主安装。额外的ACK与区域编码研究见相邻`../region-ack-research/RESULTS.md`，未进入本版协议。

## 根因与修正

实际日志中，暂停发生于流式响应期间，恢复请求报告`appendOnly=true`、`staticPrefixUnchanged=true`、`expectedReset=false`。阻挡来自独立的费用保护：取消导致最后的usage帧未到达，旧代码把估计输入量181,324和未知缓存信息计为一次大输入异常，继而阻止恢复。

本地旧代码负对照复现了相同告警。修正仅作用于`provider.cancelled`且没有缓存计数的情况：保留独立的未知结算记录，不向缓存样本加入估计值，不虚构零命中或成功命中，不清除此前的真实告警。中断前已收到的有效缓存计数继续接受通常检查；usage=null不会遮住更早帧里的有效报告。界面同时显示中断未回报次数与“费用未知”。

新增监测字段`unreportedInterruptions`和`lastUnreportedInterruption`随已有监测文件持久化；旧文件缺少字段时采用默认值。已有旧版误报告警不会静默清除，用户可检查后在设置中确认告警；该确认不授权历史改写。

真正的前缀改写仍在发送前拦截，确认费用告警也不能绕过。完整响应缺失缓存信息、明确零命中、已有命中后丢失缓存，以及异常截断/传输失败的保护继续生效。请求和会话静态提示词/schema没有改动。

## 验证

| 验证 | 结果 |
|---|---|
| Linux Rust工作区 | 229通过，0失败；`local-tests.json` |
| Node | 56通过，0失败 |
| Windows费用保护单元测试 | 9通过；`windows-guard.txt` |
| Windows大上下文暂停回归 | 1通过，内部同时覆盖Chat与Responses、每种接口两次暂停恢复；`windows-regression.txt` |
| Windows原生WebView2鼠标验收 | 两种接口各15项通过；`native-chat.json`、`native-responses.json` |
| 前缀与费用 | 真实HTTP消息前缀逐项相同，只追加；恢复不授权reset。中断不进入命中率分母，完整夹具usage才进入 |
| 格式与静态检查 | Rust fmt、JS语法、Clippy通过；Clippy仍有既有测试初始化等警告 |

原生按钮测试先构造隔离的大历史，通过实际鼠标点击开始、暂停、继续，再暂停、再继续；两次中断均保留同一run，最终完成。测试只调用本机合成API，缓存数字是夹具值，不证明真实上游缓存命中，也不产生真实模型费用。OSC始终关闭。

Windows完整provider_harness尝试返回成功，但日志缺少完整测试结尾，因此不将其计为44项通过；上表只计算有明确测试结果的选择性回归及原生验收。

## 部署

`preview-deployment.json`记录0.15.1不可变运行目录、程序哈希和桌面入口。原Preview数据保留，未复制主安装的活动任务。临时验收使用独立Data/Profile，已经清理临时窗口与计划任务。

主程序核心与外壳哈希仍分别为`d15a43299e438413259a8f837741ccca90356433b8bcdd849d6562c88fe270fb`、`cb3a793d983da00fae46a7b329ec138f8dabdc0ed29d3763dca32664b3023094`；进程201512、210184持续运行，未被关闭。主数据由用户继续使用，未对活动数据做恢复或覆盖。

头像、Unity资源与9000发送通道没有变更。
