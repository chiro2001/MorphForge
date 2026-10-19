# Windows v2 最终验收

状态：2026-10-19 已执行并通过 Windows 原生、NSIS 载荷和隔离安装生命周期验收。冻结业务源码在构建前后保持一致。

## 冻结输入

Windows 工程：

```text
D:\Projects\VRChatProjects2\MorphForge
```

冻结清单：

```text
docs/quality-v2/source-freeze.json
```

构建前后分别执行：

```powershell
python tools/release_manifest.py check docs/quality-v2/source-freeze.json
```

两次结果一致：

```json
{
  "ok": true,
  "files": 69,
  "sha256": "91a91fd5022a4c57173dc837d350f4fc67be9ef0c8c018ff0c08a4cba8af42b0",
  "mismatches": []
}
```

本轮新增验收脚本和报告不属于 69 个打包输入。没有修改业务源码、Unity `Assets/` 或 dirty Stage3。

## 环境与构建

- Windows `10.0.26200`；
- PowerShell `7.6.7`；
- Python `3.12.10`；
- Cargo / rustc `1.94.0`；
- Tauri CLI `2.10.1`；
- 构建时 `CARGO_BUILD_JOBS=4`。

先执行：

```powershell
.\tools\build_windows.ps1 -CheckOnly
```

结果：core 74 项测试通过，desktop shell 2 项通过。此前按活跃 sender 条件过滤的 transport 周期测试已实际执行并通过。

随后执行默认构建：

```powershell
.\tools\build_windows.ps1
```

未传入 `-ReuseSidecar`。生成 `0.2.0` release sidecar、Tauri 桌面程序和 NSIS 安装器。

## 原生 HTTP 验收

脚本：

```text
tools/check_windows_v2.ps1
```

报告：

```text
docs/quality-v2/windows-v2-native-final.json
```

结果：**59 项检查通过，0 失败**。

测试从全新空目录启动 release core，使用随机 loopback 端口和 `--no-capture`。provider 指向不可用的本机地址，未调用付费模型；OSC 始终关闭。

覆盖：

- 内置 `builtin:noto-sans-sc`、OFL-1.1 与字体哈希；
- 中文纯预览、缺字 typed error、apply、durable receipt 重放和撤销；
- `zh-CN` / `en` tool schema 一致，版本为 `morphforge-tools/2.3` / `morphforge-system/2.3`；
- 12 个工具包含 `finish_artwork`；
- `mf.text` Recipe dry-run；
- 16,384 Box Recipe dry-run，结果精确为 16,384；
- 所有 dry-run 均不改变 revision 或 scene；
- 无 provider 或 Harness 消息事件；
- 脚本自有 core 进程退出。

原生测试还比较了内嵌资源的 HTTP 原始字节、Windows 磁盘源码和冻结 manifest 三方 SHA-256：

| 资源                              | SHA-256                                                            |
| --------------------------------- | ------------------------------------------------------------------ |
| `web/studio.html`                 | `4b5625c4fdea995d77c69a0f3c143abe864c8102f35d147d1094a640b7feb906` |
| `web/studio.js`                   | `dab5f183ff0793d936ad08f26c56746324510491a2933e8f4c4afb1d8bd1c178` |
| `web/group-ui.js`                 | `57ba9e8bfb28c63d543065c08ed01650e75b0c34cde3a1b89c8dfff974a64b18` |
| `web/harness-ui.js`               | `5aa8d4861713777fe02ea0b0fd28fa65265983880dec80a5f71edb767e0e3a7f` |
| `web/text-ui.js`                  | `c89456b75fd4d74c4ca68cb2cfd04e1f09dce089d3cb0ea2475bfc57d60f4e86` |
| `web/workbench.css`               | `339200f5f4b5e57c9f95f68edd8a7869b45761b5e63d08a3231364d562b95c51` |
| `web/licenses/OFL-NotoSansSC.txt` | `6a73f9541c2de74158c0e7cf6b0a58ef774f5a780bf191f2d7ec9cc53efe2bf2` |

这证明实际 sidecar 提供的是冻结的新前端，而不仅是磁盘源码。

## 构建产物与许可

| 文件                                     |       大小 | SHA-256                                                            |
| ---------------------------------------- | ---------: | ------------------------------------------------------------------ |
| `MorphForge Desktop_0.2.0_x64-setup.exe` | 13,613,538 | `ce30b61609ff93c250840560d14ab4278709e7bbd37eacda63f736b4fccbef03` |
| release `morphforge.exe`                 | 31,462,400 | `cc34fd94473dc25961db3bc6e8fa91e01a32704a03f78929362ad9032996f9f7` |
| release `morphforge-desktop.exe`         | 11,077,120 | `e56caec8e044b84939446c98e64986cf740e86b2d4b4316dd9fa00fca5539ea3` |

NSIS 载荷用 7-Zip 独立解包并核对：

- 内含 `morphforge.exe`，哈希与 release core 完全一致；
- 内含 `morphforge-desktop.exe`；
- 内含 `licenses/OFL-NotoSansSC.txt`，4,301 bytes；
- 安装后许可证哈希与冻结字体许可证一致；
- 安装器未签名。

## 隔离安装生命周期

脚本：

```text
tools/check_desktop_v2.ps1
```

报告：

```text
docs/quality-v2/desktop-v2-validation.json
```

结果：**26 项检查通过，0 失败**。

NSIS 静默安装到用户临时目录中的新 GUID 路径，退出码为 `0`。应用的 `MORPHFORGE_DATA` 和 `LOCALAPPDATA` 都重定向到隔离目录；设置 `MORPHFORGE_DESKTOP_DISABLE_CAPTURE=1`，不启动截图浏览器。

在真实 Session 1 中启动 Tauri。窗口截图采用 Per-Monitor V2 DPI 感知和 DWM 扩展边框，物理尺寸为 **1802×1189**、DPI `120`。标题栏、右侧 AI 栏和底部状态栏完整可见，不再使用 125% DPI 下被裁切的 `1454×958` 图。

截图：

```text
docs/quality-v2/desktop-v2-window.png
```

截图旁车：

- `docs/quality-v2/desktop-v2-window.json`；
- `docs/quality-v2/desktop-v2-close.json`。

关闭辅助只发送一次 `WM_CLOSE`，正常流程中 `PostMessage=true`，不再追加第二次 `CloseMainWindow()`，避免 Tauri 退出重入。

生命周期覆盖：

- 安装后 sidecar 与 OFL 许可证哈希；
- 随机本机端口、OSC 关闭；
- 安装后的 HTTP 资源与磁盘源码、69-file freeze 中对应项一致；
- 字体文字、分组保护、完成总结 UI 标记；
- `morphforge-tools/2.3`、`morphforge-system/2.3` 和 `finish_artwork`；
- 单次 `WM_CLOSE` 后桌面壳与 sidecar 均退出；
- 静默卸载退出 `0`；
- 安装的可执行文件与对应 HKCU 卸载项删除。

本轮没有在内嵌 WebView 中自动点击字体/保护控件。WebView2 CDP 探针未能建立调试端口，因此没有把 HTML 标记检查冒充成按钮级 GUI 自动化。真实 UI 行为和分组保护 API 已由 Linux Chromium + 真实 R2b 后端专项报告覆盖；Windows 侧确认了相同冻结资源确实被安装版服务并显示。

## 清理

最终状态：

```text
验证计划任务：0
MorphForge 桌面进程：0
MorphForge core 进程：0
验证安装目录：0
验证临时数据目录：0
默认 %LOCALAPPDATA%\MorphForge Desktop：不存在
构建后 manifest：69/69，哈希一致
```

未触碰默认 userdata、生产安装、Unity Assets、dirty Stage3 或用户 VRChat 进程；未开启 OSC/connect/publish，也未发送真实模型请求。

## 结论

Windows 冻结源码构建、原生 HTTP 验收、NSIS 载荷、字体许可证、隔离安装、Session 1 启动、完整窗口截图、正常退出与卸载均已通过。验收创建的进程、计划和临时目录已清理。
