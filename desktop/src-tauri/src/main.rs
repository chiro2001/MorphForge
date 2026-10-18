#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use tauri::webview::{NewWindowResponse, PageLoadEvent};
use tauri::{AppHandle, Manager, RunEvent, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

const SIDECAR_NAME: &str = "morphforge";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(45);
const MAX_LOG_BYTES: usize = 32 * 1024;

#[derive(Clone)]
struct DesktopState {
    child: Arc<Mutex<Option<CommandChild>>>,
    expected_port: Arc<AtomicU16>,
    stopping: Arc<AtomicBool>,
    page_ready: Arc<AtomicBool>,
    _process_tree: Arc<ProcessTree>,
    log_path: Arc<PathBuf>,
}

impl DesktopState {
    fn new(process_tree: ProcessTree, log_path: PathBuf) -> Self {
        Self {
            child: Arc::new(Mutex::new(None)),
            expected_port: Arc::new(AtomicU16::new(0)),
            stopping: Arc::new(AtomicBool::new(false)),
            page_ready: Arc::new(AtomicBool::new(false)),
            _process_tree: Arc::new(process_tree),
            log_path: Arc::new(log_path),
        }
    }

    fn terminate_core_tree(&self) {
        let child = self.child.lock().ok().and_then(|mut slot| slot.take());
        if let Some(child) = child {
            if let Err(error) = child.kill() {
                append_log(&self.log_path, &format!("[desktop] kill failed: {error}"));
            }
        }
    }

    fn append(&self, line: &str) {
        append_log(&self.log_path, line);
    }
}

fn main() {
    let log_path = desktop_log_path();
    let process_tree = match ProcessTree::new() {
        Ok(tree) => tree,
        Err(error) => {
            let message = format!(
                "无法建立 Windows 进程树隔离，为避免留下孤儿截图进程，MorphForge 未启动。\n\n{error}"
            );
            append_log(&log_path, &format!("[desktop] {message}"));
            show_native_error("MorphForge 启动失败", &message);
            return;
        }
    };
    let state = DesktopState::new(process_tree, log_path.clone());
    append_log(&log_path, "[desktop] application start");

    let run_result = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(state.clone())
        .setup(move |app| {
            let window = startup_window(app, state.clone())?;
            launch_core(app.handle().clone(), state.clone(), window);
            Ok(())
        })
        .build(tauri::generate_context!());

    let app = match run_result {
        Ok(app) => app,
        Err(error) => {
            append_log(
                &log_path,
                &format!("[desktop] Tauri initialization failed: {error}"),
            );
            show_native_error(
                "MorphForge 启动失败",
                &format!(
                    "Tauri WebView 初始化失败。\n\n{error}\n\n日志：{}",
                    log_path.display()
                ),
            );
            return;
        }
    };

    app.run(move |app_handle, event| {
        if matches!(event, RunEvent::ExitRequested { .. } | RunEvent::Exit) {
            if let Some(state) = app_handle.try_state::<DesktopState>() {
                if !state.stopping.swap(true, Ordering::SeqCst) {
                    state.append("[desktop] exit requested; terminating core process tree");
                }
                state.terminate_core_tree();
            }
        }
    });
}

fn startup_window<R: tauri::Runtime>(
    app: &mut tauri::App<R>,
    state: DesktopState,
) -> tauri::Result<WebviewWindow<R>> {
    let expected_port = state.expected_port.clone();
    let page_ready = state.page_ready.clone();
    WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("MorphForge")
        .inner_size(1440.0, 920.0)
        .min_inner_size(900.0, 600.0)
        .center()
        .on_navigation(move |url| {
            if url.scheme() == "tauri"
                || (matches!(url.scheme(), "http" | "https")
                    && url.host_str() == Some("tauri.localhost"))
            {
                return true;
            }
            let port = expected_port.load(Ordering::SeqCst);
            url.scheme() == "http"
                && url.host_str() == Some("127.0.0.1")
                && port != 0
                && url.port() == Some(port)
        })
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .on_page_load(move |window, payload| {
            if payload.event() == PageLoadEvent::Finished {
                page_ready.store(true, Ordering::SeqCst);
                let _ = window.eval(
                    "window.__MF_DESKTOP_PAGE_READY__ = true; \
                     if (window.__MF_DESKTOP_RENDER__) window.__MF_DESKTOP_RENDER__();",
                );
            }
        })
        .build()
}

fn launch_core<R: tauri::Runtime>(
    app: AppHandle<R>,
    state: DesktopState,
    window: WebviewWindow<R>,
) {
    let _monitor = tauri::async_runtime::spawn(async move {
        let mut command = match app.shell().sidecar(SIDECAR_NAME) {
            Ok(command) => command
                .args(["serve", "--port", "0"])
                .set_process_group(true),
            Err(error) => {
                startup_failure(
                    &state,
                    &window,
                    "无法定位 MorphForge 核心",
                    &format!(
                        "sidecar `{SIDECAR_NAME}` 解析失败。请确认安装包中同时包含 morphforge.exe。\n\n{error}"
                    ),
                )
                .await;
                return;
            }
        };

        if capture_disabled() {
            command = command.arg("--no-capture");
            state.append("[desktop] core capture disabled by MORPHFORGE_DESKTOP_DISABLE_CAPTURE");
        }

        let (mut events, child) = match command.spawn() {
            Ok(value) => value,
            Err(error) => {
                startup_failure(
                    &state,
                    &window,
                    "MorphForge 核心启动失败",
                    &format!("无法启动 sidecar morphforge.exe。\n\n{error}"),
                )
                .await;
                return;
            }
        };

        let mut pending_child = Some(child);
        let stored = match state.child.lock() {
            Ok(mut slot) => {
                *slot = pending_child.take();
                true
            }
            Err(_) => false,
        };
        if !stored {
            if let Some(child) = pending_child.take() {
                let _ = child.kill();
            }
            startup_failure(
                &state,
                &window,
                "MorphForge 启动失败",
                "内部进程状态锁已损坏，核心进程树将被终止。",
            )
            .await;
            state.terminate_core_tree();
            return;
        }
        state.append("[desktop] core sidecar spawned");

        let mut ready = false;
        let deadline = tokio::time::Instant::now() + STARTUP_TIMEOUT;
        loop {
            let next_event = if ready {
                events.recv().await
            } else {
                match tokio::time::timeout_at(deadline, events.recv()).await {
                    Ok(event) => event,
                    Err(_) => {
                        startup_failure(
                            &state,
                            &window,
                            "MorphForge 核心启动超时",
                            &format!(
                                "核心在 {} 秒内没有报告 HTTP 监听端口。",
                                STARTUP_TIMEOUT.as_secs()
                            ),
                        )
                        .await;
                        state.terminate_core_tree();
                        break;
                    }
                }
            };

            let Some(event) = next_event else {
                if !state.stopping.load(Ordering::SeqCst) {
                    if ready {
                        show_native_error(
                            "MorphForge 核心事件流中断",
                            &format!(
                                "核心事件通道意外关闭。\n\n日志：{}",
                                state.log_path.display()
                            ),
                        );
                    } else {
                        startup_failure(
                            &state,
                            &window,
                            "MorphForge 核心事件流中断",
                            "核心事件通道在启动完成前关闭。",
                        )
                        .await;
                    }
                    state.terminate_core_tree();
                }
                break;
            };

            match event {
                CommandEvent::Stdout(bytes) => {
                    let line = String::from_utf8_lossy(&bytes).trim().to_owned();
                    state.append(&format!("[core stdout] {line}"));
                    if !ready {
                        if let Some(url) = ready_url(&line) {
                            let port = url.port().unwrap_or(0);
                            state.expected_port.store(port, Ordering::SeqCst);
                            match window.navigate(url) {
                                Ok(()) => {
                                    ready = true;
                                    state.append(&format!(
                                        "[desktop] opening core HTTP UI on 127.0.0.1:{port}"
                                    ));
                                }
                                Err(error) => {
                                    startup_failure(
                                        &state,
                                        &window,
                                        "MorphForge WebView 导航失败",
                                        &format!("无法打开核心 HTTP 页面。\n\n{error}"),
                                    )
                                    .await;
                                    state.terminate_core_tree();
                                    break;
                                }
                            }
                        }
                    }
                }
                CommandEvent::Stderr(bytes) => {
                    let line = String::from_utf8_lossy(&bytes).trim().to_owned();
                    if !line.is_empty() {
                        state.append(&format!("[core stderr] {line}"));
                    }
                }
                CommandEvent::Error(error) => {
                    state.append(&format!("[core event error] {error}"));
                    if !ready && !state.stopping.load(Ordering::SeqCst) {
                        startup_failure(&state, &window, "MorphForge 核心事件错误", &error).await;
                    }
                }
                CommandEvent::Terminated(payload) => {
                    state.append(&format!(
                        "[core terminated] code={:?} signal={:?}",
                        payload.code, payload.signal
                    ));
                    state.terminate_core_tree();
                    if !state.stopping.load(Ordering::SeqCst) {
                        let message =
                            format!("MorphForge 核心已退出（code: {:?}）。", payload.code);
                        if ready {
                            show_native_error(
                                "MorphForge 核心已退出",
                                &format!("{message}\n\n日志：{}", state.log_path.display()),
                            );
                        } else {
                            startup_failure(&state, &window, "MorphForge 核心启动失败", &message)
                                .await;
                        }
                    }
                    break;
                }
                _ => {}
            }
        }
    });
}

fn ready_url(line: &str) -> Option<Url> {
    const MARKER: &str = "MorphForge Rust ready: ";
    let start = line.find(MARKER)? + MARKER.len();
    let candidate = line[start..].split_whitespace().next()?;
    let url = Url::parse(candidate).ok()?;
    if url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || url.port().filter(|port| *port != 0).is_none()
    {
        return None;
    }
    Some(url)
}

fn capture_disabled() -> bool {
    matches!(
        std::env::var("MORPHFORGE_DESKTOP_DISABLE_CAPTURE")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes"
    )
}

async fn startup_failure<R: tauri::Runtime>(
    state: &DesktopState,
    window: &WebviewWindow<R>,
    title: &str,
    message: &str,
) {
    state.append(&format!("[desktop] startup failure: {title}: {message}"));
    let payload = json!({
        "phase": "error",
        "title": title,
        "message": message,
        "logPath": state.log_path.display().to_string(),
    });
    let script = format!(
        "window.__MF_DESKTOP_STATE__ = {payload}; \
         if (window.__MF_DESKTOP_RENDER__) window.__MF_DESKTOP_RENDER__();"
    );
    for _ in 0..30 {
        if state.page_ready.load(Ordering::SeqCst) {
            if window.eval(&script).is_ok() {
                return;
            }
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    show_native_error(
        title,
        &format!("{message}\n\n日志：{}", state.log_path.display()),
    );
}

fn desktop_log_path() -> PathBuf {
    let root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_STATE_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .unwrap_or_else(std::env::temp_dir);
    root.join("MorphForge").join("desktop-launch.log")
}

fn append_log(path: &Path, line: &str) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(metadata) = fs::metadata(path) {
        if metadata.len() > MAX_LOG_BYTES as u64 {
            let _ = fs::remove_file(path);
        }
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}

fn show_native_error(title: &str, message: &str) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
        let title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
        let message: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                message.as_ptr(),
                title.as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }
    }
    #[cfg(not(windows))]
    {
        eprintln!("{title}: {message}");
    }
}
struct ProcessTree {
    #[cfg(windows)]
    handle: windows_sys::Win32::Foundation::HANDLE,
}

impl ProcessTree {
    fn new() -> Result<Self, String> {
        #[cfg(windows)]
        {
            use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
            use windows_sys::Win32::System::JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
                SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            };
            use windows_sys::Win32::System::Threading::GetCurrentProcess;

            unsafe {
                let handle: HANDLE = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if handle.is_null() {
                    return Err(format!(
                        "CreateJobObjectW failed: {}",
                        std::io::Error::last_os_error()
                    ));
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                if SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const _,
                    std::mem::size_of_val(&info) as u32,
                ) == 0
                {
                    let error = std::io::Error::last_os_error();
                    CloseHandle(handle);
                    return Err(format!("SetInformationJobObject failed: {error}"));
                }
                if AssignProcessToJobObject(handle, GetCurrentProcess()) == 0 {
                    let error = std::io::Error::last_os_error();
                    CloseHandle(handle);
                    return Err(format!("AssignProcessToJobObject failed: {error}"));
                }
                Ok(Self { handle })
            }
        }
        #[cfg(not(windows))]
        {
            Ok(Self {})
        }
    }
}

#[cfg(windows)]
impl Drop for ProcessTree {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

#[cfg(windows)]
unsafe impl Send for ProcessTree {}
#[cfg(windows)]
unsafe impl Sync for ProcessTree {}

#[cfg(test)]
mod tests {
    use super::ready_url;

    #[test]
    fn parses_local_core_url() {
        let url = ready_url(
            "MorphForge Rust ready: http://127.0.0.1:43621 · data=C:\\data · OSC disabled",
        )
        .expect("valid ready URL");
        assert_eq!(url.host_str(), Some("127.0.0.1"));
        assert_eq!(url.port(), Some(43621));
    }

    #[test]
    fn rejects_non_local_or_unspecified_ports() {
        assert!(ready_url("MorphForge Rust ready: http://localhost:4231").is_none());
        assert!(ready_url("MorphForge Rust ready: http://127.0.0.1:0").is_none());
        assert!(ready_url("unrelated output").is_none());
    }
}

