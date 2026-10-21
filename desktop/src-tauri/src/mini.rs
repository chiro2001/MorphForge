//! Native mini windows use a closed set of declarative file preferences, never
//! invoke permissions on the remote workbench origin.
use super::*;
use tauri::WindowEvent;

fn valid_preferences(v: &serde_json::Value) -> bool {
    matches!(v["mode"].as_str(), Some("speech" | "progress" | "custom"))
        && v["miniWidth"]
            .as_u64()
            .is_some_and(|x| (320..=1600).contains(&x))
        && v["miniHeight"]
            .as_u64()
            .is_some_and(|x| (180..=1600).contains(&x))
}
pub fn start<R: tauri::Runtime>(app: AppHandle<R>, state: DesktopState, line: &str) {
    let Some(rest) = line.split(" · data=").nth(1) else {
        return;
    };
    let data = rest.split(" · ").next().unwrap_or("");
    if data.is_empty() {
        return;
    }
    let path = PathBuf::from(data).join("desktop-window.json");
    let _worker = tauri::async_runtime::spawn(async move {
        let mut revision = None;
        while !state.stopping.load(Ordering::SeqCst) {
            if let Ok(bytes) = fs::read(&path) {
                if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    let next = value["revision"].as_u64();
                    if next != revision && valid_preferences(&value) {
                        let result = apply(
                            &app,
                            &state,
                            &value,
                            &path.with_file_name("desktop-mini-bounds.json"),
                        );
                        let error = result.err().map(|e| e.to_string());
                        if let Some(e) = &error {
                            state.append(&format!("[mini] {e}"));
                        }
                        let describe = |name: &str| {
                            app.get_webview_window(name).map(|w|json!({"visible":w.is_visible().ok(),"pinned":w.is_always_on_top().ok(),"size":w.inner_size().ok().map(|s|[s.width,s.height])}))
                        };
                        let ack = json!({"revision":next,"nativeProcessId":std::process::id(),"main":describe("main"),"mini":describe("mini"),"error":error});
                        let destination = path.with_file_name("desktop-window-status.json");
                        let temp = destination.with_extension("tmp");
                        if fs::write(&temp, ack.to_string()).is_ok() {
                            if let Err(e) = fs::rename(&temp, &destination) {
                                state.append(&format!("[mini] status write: {e}"));
                            }
                        }
                        revision = next;
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
    });
}
fn apply<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &DesktopState,
    v: &serde_json::Value,
    bounds: &Path,
) -> tauri::Result<()> {
    let port = state.expected_port.load(Ordering::SeqCst);
    if port == 0 {
        return Ok(());
    }
    if let Some(main) = app.get_webview_window("main") {
        main.set_always_on_top(v["mainPinned"] == true)?;
        if v["mainVisible"] != false {
            main.show()?;
            if v["focus"] == "main" {
                main.set_focus()?;
            }
        }
    }
    let existing = app.get_webview_window("mini");
    if v["miniOpen"] != true {
        if let Some(w) = existing {
            w.close()?;
        }
        return Ok(());
    }
    let mode = v["mode"].as_str().unwrap_or("speech");
    let url = Url::parse(&format!("http://127.0.0.1:{port}/mini.html?mode={mode}"))
        .expect("fixed mini URL");
    let mini = if let Some(w) = existing {
        if v["lastAction"] == "resize" {
            w.set_size(tauri::LogicalSize::new(
                v["miniWidth"].as_u64().unwrap_or(460) as f64,
                v["miniHeight"].as_u64().unwrap_or(320) as f64,
            ))?;
        }
        if w.url()?.as_str() != url.as_str() {
            w.navigate(url)?;
        }
        w
    } else {
        let expected = port;
        let w = WebviewWindowBuilder::new(app, "mini", WebviewUrl::External(url))
            .title("MorphForge Mini")
            .transparent(true)
            .inner_size(
                v["miniWidth"].as_u64().unwrap_or(460) as f64,
                v["miniHeight"].as_u64().unwrap_or(320) as f64,
            )
            .min_inner_size(320., 180.)
            .resizable(true)
            .on_navigation(move |u| {
                u.scheme() == "http"
                    && u.host_str() == Some("127.0.0.1")
                    && u.port() == Some(expected)
            })
            .on_new_window(|_, _| NewWindowResponse::Deny)
            .build()?;
        restore_bounds(&w, bounds);
        let handle = app.clone();
        let bounds = bounds.to_path_buf();
        let last_saved = Arc::new(Mutex::new(std::time::Instant::now()));
        w.on_window_event(move |event| {
            let closing = matches!(event, WindowEvent::CloseRequested { .. });
            if closing || matches!(event, WindowEvent::Moved(_) | WindowEvent::Resized(_)) {
                let mut last = last_saved.lock().unwrap();
                if closing || last.elapsed() > Duration::from_millis(150) {
                    if let Some(mini) = handle.get_webview_window("mini") {
                        save_bounds(&mini, &bounds);
                    }
                    *last = std::time::Instant::now();
                }
            }
            if matches!(event, WindowEvent::CloseRequested { .. }) {
                notify(port, "close");
                if let Some(main) = handle.get_webview_window("main") {
                    if let Err(e) = main.show() {
                        eprintln!("[mini] restore main: {e}");
                    }
                }
            }
        });
        w
    };
    mini.set_always_on_top(v["miniPinned"] == true)?;
    mini.show()?;
    if v["focus"] == "mini" {
        mini.set_focus()?;
    }
    if v["mainVisible"] == false {
        if let Some(main) = app.get_webview_window("main") {
            main.hide()?;
        }
    }
    Ok(())
}

pub fn notify(port: u16, action: &'static str) {
    std::thread::spawn(move || {
        use std::io::Write;
        let body = serde_json::json!({"action":action}).to_string();
        let address = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        if let Ok(mut stream) =
            std::net::TcpStream::connect_timeout(&address, Duration::from_secs(2))
        {
            let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
            if let Err(e)=write!(stream,"POST /api/desktop/window HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body){eprintln!("[mini] close notification: {e}");}
        }
    });
}

fn save_bounds<R: tauri::Runtime>(w: &WebviewWindow<R>, path: &Path) {
    if let (Ok(p), Ok(s), Ok(scale)) = (w.outer_position(), w.inner_size(), w.scale_factor()) {
        let value = json!({"x":p.x,"y":p.y,"width":s.width,"height":s.height,"scale":scale});
        let temp = path.with_extension("tmp");
        if fs::write(&temp, value.to_string()).is_ok() {
            if let Err(e) = fs::rename(&temp, path) {
                eprintln!("[mini] save bounds: {e}");
            }
        }
    }
}
fn restore_bounds<R: tauri::Runtime>(w: &WebviewWindow<R>, path: &Path) {
    let restored = (|| -> Option<()> {
        let value: serde_json::Value = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
        let x = i32::try_from(value["x"].as_i64()?).ok()?;
        let y = i32::try_from(value["y"].as_i64()?).ok()?;
        let width = value["width"].as_u64()?;
        let height = value["height"].as_u64()?;
        if !(320..=6400).contains(&width) || !(180..=6400).contains(&height) {
            return None;
        }
        let monitors = w.available_monitors().ok()?;
        let monitor = monitors.iter().find(|m| {
            let p = m.position();
            let s = m.size();
            x >= p.x
                && y >= p.y
                && (x as i64) < p.x as i64 + s.width as i64
                && (y as i64) < p.y as i64 + s.height as i64
        })?;
        let old_scale = value["scale"]
            .as_f64()
            .filter(|s| s.is_finite() && *s > 0.)
            .unwrap_or(1.);
        let ratio = monitor.scale_factor() / old_scale;
        w.set_size(tauri::PhysicalSize::new(
            (width as f64 * ratio).clamp(320., 6400.) as u32,
            (height as f64 * ratio).clamp(180., 6400.) as u32,
        ))
        .ok()?;
        w.set_position(tauri::PhysicalPosition::new(x, y)).ok()?;
        Some(())
    })();
    if restored.is_none() {
        if let Err(e) = w.center() {
            eprintln!("[mini] center window: {e}");
        }
    }
}
