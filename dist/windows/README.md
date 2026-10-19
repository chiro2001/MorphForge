# MorphForge Desktop 0.2.0

当前安装器：`MorphForge Desktop_0.2.0_x64-setup.exe`（Windows x64，Tauri 2 + Rust）。

新增可打断/插话 Harness、中文/英文工具、字体体素文字、保护分组和作品入库收尾；内置 Noto Sans SC 及完整 OFL 许可。

SHA-256 见 `SHA256SUMS`。验收和使用入口见 `docs/DELIVERY.md`、`docs/USER-GUIDE.md` 和 `docs/quality-v2/RESULTS.md`。默认用户数据在 `%LOCALAPPDATA%\MorphForge`，安装器和源码都不含 API Key。

0.1.0安装器保留用于历史复现；新安装使用0.2.0。

完整Git历史已附为 `MorphForge-0.2.0-rc.2.bundle` 与对应 `.bundle.sha256`，含最新Avatar修复和服务端通过证据。可在新目录执行 `git clone MorphForge-0.2.0-rc.2.bundle MorphForge-source` 查看源码及阶段提交；Unity工作区本身保持原目录结构。源码候选标签为`v0.2.0-rc.2`，最终标签等待真实客户端确认。

历史审查快照也保留：`MorphForge-review-a9e2c08.bundle`，使用和校验见 [REVIEW-SOURCE.md](REVIEW-SOURCE.md)。
