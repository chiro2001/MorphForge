# 唯一动画默认发布（Desktop 0.26.1）

项目只有一个已保存动画时，载入、重启或新增该片段后，主界面与动画面板都会自动选中它。无需打开动画窗口，点击“发布当前版本”即可一起发布模型与动画。多个片段保留明确选择，未保存草稿不会被自动覆盖；手动选择“不选择新动画”也会保留。

HTTP普通发布省略`animation`时默认使用唯一片段，从0秒、1×、播放状态发布；显式`animation:null`选择普通模型发布。补播和重建沿用当前动画，不重新选择。

[相关Rust9项、前端62项](automated-tests.json)、[Chromium16项](browser/validation.json)和[Windows原生16项](native/validation.json)通过，涵盖不打开动画窗口直接发布、重启后自动选择、多片段切换、未保存提示、静态选择和旧接收器版本阻止。

主安装已通过正式NSIS升级至 **0.26.1**，[安装14项检查](install-report.json)通过，程序与原数据备份在`E:\tmp\morphforge\main-backup-v0261-default-motion`，安装前后数据逐文件摘要一致。[主程序8项检查](main-smoke.json)确认项目第16版、模型/文字/材质和API连接保留；动画窗口始终关闭，`flow-loop`自动选中，没有手动恢复选择或替用户发布。发送、麦克风保持关闭。

[285个冻结构建输入](source-freeze.json)与[正式载荷哈希](windows-payload.json)对应当前主安装。真实9000目标发布动画要求能力版本9，旧版会明确提示重新加载更新后的头像。对应[头像修正与复验](../motion-cache-20261024/RESULTS.md)。未推送或创建Release。
