> 2026-10-20：用户已改为默认更新主安装。本文保留双副本隔离说明；主安装更新流程见 `tools/install_main.ps1` 和 [0.15交付](../creative-v15/RESULTS.md)。

# gamepc 双副本使用与更新

首次建立时两个副本均为 0.6.1。开发测试副本现已部署 0.7.0；原验收版保持 0.6.1。运行中的旧副本继续使用旧版本，关闭重开后进入新版。

| 项目 | 当前验收版 | 开发测试副本 |
|---|---|---|
| 桌面入口 | 原 MorphForge Desktop | MorphForge - 开发测试副本 |
| 安装根 | `%LOCALAPPDATA%\MorphForge Desktop` | `%LOCALAPPDATA%\MorphForge Preview` |
| 作品与上下文 | `%LOCALAPPDATA%\MorphForge` | `%LOCALAPPDATA%\MorphForge Preview\Data` |
| WebView、用户配置与日志 | 原有路径 | 副本的 `Profile/` |
| 更新约定 | 用户继续验收，保持运行 | 默认向此副本部署新代码 |

副本首次复制当前 791 Box 作品、作品库、快照、Recipe 源码和 API/模型设置。新副本创建空白独立对话，不复制正在执行的任务、排队消息、Token/缓存账本或 OSC 连接状态。副本的 Provider 连接已通过本机 API 独立持久化；凭据留在 gamepc，未写入报告和 Git。后续两边的数据各自保存，不自动双向同步；对照同一个作品可以导出/导入。

开发更新通过 `tools/install_preview.ps1`，从包含两个 EXE 与 `licenses/` 的运行包部署。它不运行 NSIS，不改变原安装的卸载注册信息，不关闭任何窗口。按完整运行包内容哈希创建不可变 `versions/` 目录，然后切换启动指针。已打开的副本继续使用原目录，关闭重开后采用新版本；这样也能保持 Recipe 子进程所需的 `current_exe()` 路径。运行中的版本目录不应删除。

桌面快捷方式启动独立环境，并将标题设为 `MorphForge - 开发测试副本 <版本>`。重复点击会聚焦已有副本，避免启动第二个占用同一数据目录的核心。默认保留截图进程。

验证：[部署、窗口与隔离回执](../quality-v6.1/preview-install.json)。

- 原验收版进程保持运行，副本同时启动，两边使用不同端口。
- WebView2 使用独立 user-data-dir；对话 ID 不同，副本从空白上下文开始。
- 副本修改项目名后，原版项目名不变；初始 791 Box 场景一致。
- 两边均运行时再次部署成功，两个进程都保持原 PID，运行中的核心哈希不变，副本数据未重置。
- 副本实际 Recipe 子进程 dry-run 通过，截图进程实际生成当前 revision 的 640×480 PNG。
- 桌面快捷方式实际启动/聚焦通过，窗口标题及可见窗口截图已核实。
- 未调用付费模型、发送 OSC 或发布 Release。
