# MorphForge desktop shell

This is an independent Tauri v2 workspace. It owns only the native shell and a startup error
page. The application UI and APIs remain the existing HTTP frontend served by the Rust core.

The installer product name is `MorphForge Desktop`, so the default current-user install directory
is `%LOCALAPPDATA%\MorphForge Desktop`. Core project data remains separately under
`%LOCALAPPDATA%\MorphForge`.

## Runtime shape

```text
MorphForge.exe                         Tauri native shell
  └─ morphforge.exe serve --port 0     bundled sidecar
       ├─ HTTP UI + Rust APIs          http://127.0.0.1:<random>
       ├─ recipe-worker                same current_exe
       └─ optional screenshot Edge
```

The shell:

- starts the sidecar on a random loopback port;
- waits for the core's `MorphForge Rust ready:` line;
- opens the returned HTTP URL in the WebView;
- gives the remote HTTP page no Tauri permissions;
- kills the core job tree on normal exit;
- joins the Windows shell process to a `KILL_ON_JOB_CLOSE` Job so a shell crash also cleans
  up core, recipe workers and screenshot Edge children.

Unity is not required. The core is the same standalone `morphforge.exe`.

## Build on Windows

Prerequisites:

- Rust MSVC toolchain, `x86_64-pc-windows-msvc`;
- Visual Studio Build Tools with the Desktop C++ workload;
- Microsoft Edge WebView2 runtime;
- Tauri CLI v2.

Install the CLI once when packaging is desired:

```powershell
cargo install tauri-cli --version "^2" --locked
```

An npm-installed `tauri` CLI is also accepted by the build script.

Then run from the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\build_windows.ps1
```

The script performs these steps without Unity:

1. builds `crates/morphforge-app` as `morphforge.exe`;
2. verifies the `current_exe()`/`recipe-worker` source invariant and binary name;
3. stages `desktop/src-tauri/binaries/morphforge-x86_64-pc-windows-msvc.exe`;
4. generates icons through the official `cargo tauri icon` command;
5. runs `cargo tauri build --target x86_64-pc-windows-msvc --bundles nsis`.

Useful options:

```powershell
.\tools\build_windows.ps1 -CheckOnly
.\tools\build_windows.ps1 -SkipBundle
.\tools\build_windows.ps1 -ReuseSidecar
```

`-CheckOnly` runs Cargo checks and tests without producing an installer. `-SkipBundle` builds
the native executable and staged sidecar but skips the NSIS installer. Set
`MORPHFORGE_DESKTOP_DISABLE_CAPTURE=1` when launching the packaged app to omit screenshot Edge
entirely.

`-ReuseSidecar` is an explicit development-only path for repackaging the last successful core
binary while another part of the core source is changing. The normal no-argument build always
rebuilds `morphforge.exe` before packaging.

The launcher log is:

```text
%LOCALAPPDATA%\MorphForge\desktop-launch.log
```

## Security boundary

`tauri.conf.json` declares an explicit capability for the `main` window with zero permissions,
and the build has no invoke handler or command manifest. The normal WebView loads the core's
external HTTP origin, so the page has no Tauri IPC capability. The shell plugin is used only
from Rust, not from JavaScript.

Navigation is restricted to the local startup page and the exact random loopback port reported
by the core. Popup windows are denied.

Official Tauri v2 references used for this shell:

- <https://v2.tauri.app/develop/sidecar/>
- <https://v2.tauri.app/security/capabilities/>
- <https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindowBuilder.html>
- <https://docs.rs/tauri-plugin-shell/latest/tauri_plugin_shell/process/struct.Command.html>


## gamepc side-by-side preview

Keep the user's current acceptance install running. Deploy future development builds to
`%LOCALAPPDATA%\MorphForge Preview`, using a packaged directory containing
`morphforge-desktop.exe`, `morphforge.exe` and `licenses/`:

```powershell
.\tools\install_preview.ps1 -PackageDir 'C:\path\to\packaged-runtime' -ExpectedVersion '0.6.1'
```

For the first deployment only, optionally provide `-SeedDataFrom` and `-SeedBaseUrl`
(the running acceptance backend's loopback URL). The current project is read through
`/api/export`; assets, artwork library, source files, snapshots and configuration are copied.
Model conversations, queued work, usage/prefix/cache ledgers, connection state and browser
profiles are not cloned. Existing preview data is never reseeded during an update.

The desktop shortcut **MorphForge - 开发测试副本** runs `Launch-Preview.vbs` and
`Launch-Preview.ps1`. The process environment selects a separate `Data/` directory,
launcher log, roaming profile and WebView2 user-data folder under `Profile/`. The native
window title includes “开发测试副本” and its version. Launching again focuses an existing
preview instance. Screenshots remain enabled.

Each payload is placed under `versions/<version>-<payload-hash>/`; `current.json` is
replaced atomically. Updating never stops an acceptance or preview process, runs NSIS,
or changes installer registration. Already-running previews keep their immutable runtime
and switch only after closing/reopening. Retain old version directories while in use.
This also preserves `current_exe()` for recipe workers. Neither channel is automatically
published as a GitHub Release.
