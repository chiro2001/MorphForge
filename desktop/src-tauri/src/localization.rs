use std::path::PathBuf;

pub fn language() -> String {
    let root = std::env::var_os("MORPHFORGE_DATA").map(PathBuf::from).or_else(||
        std::env::var_os("LOCALAPPDATA").or_else(|| std::env::var_os("XDG_DATA_HOME")).map(|v| PathBuf::from(v).join("MorphForge")));
    if let Some(root) = root {
        if let Ok(bytes) = std::fs::read(root.join("ui-settings.json")) {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                if let Some(code @ ("zh-CN" | "ja" | "en")) = value["language"].as_str() { return code.to_owned(); }
            }
        }
    }
    #[cfg(windows)]
    { return match unsafe {windows_sys::Win32::Globalization::GetUserDefaultUILanguage()} & 0x3ff { 0x04=>"zh-CN", 0x11=>"ja", _=>"en" }.into(); }
    #[cfg(not(windows))]
    {
        let lang = std::env::var("LC_ALL").or_else(|_|std::env::var("LANG")).unwrap_or_default();
        if lang.starts_with("zh") { "zh-CN" } else if lang.starts_with("ja") { "ja" } else { "en" }.into()
    }
}
pub fn message(source: &str) -> String {
    let lang = language();
    if lang == "zh-CN" { return source.to_owned(); }
    let mut value = source.to_owned();
    for (zh,en,ja) in [
        ("MorphForge 启动失败","MorphForge startup failed","MorphForgeの起動に失敗"),
        ("无法建立 Windows 进程树隔离，为避免留下孤儿截图进程，MorphForge 未启动。","Cannot create Windows process isolation. MorphForge was not started to avoid orphan capture processes.","Windowsのプロセス分離を作成できません。撮影プロセスの残留を防ぐため起動を中止しました。"),
        ("Tauri WebView 初始化失败。","Tauri WebView initialization failed.","Tauri WebViewの初期化に失敗しました。"),
        ("无法定位 MorphForge 核心","Cannot locate MorphForge core","MorphForgeコアが見つかりません"),
        ("解析失败。请确认安装包中同时包含 morphforge.exe。","could not be resolved. Check that the installation includes morphforge.exe.","を解決できません。インストールにmorphforge.exeが含まれるか確認してください。"),
        ("MorphForge 核心启动失败","MorphForge core failed to start","MorphForgeコアの起動に失敗"),
        ("无法启动 sidecar morphforge.exe。","Cannot start sidecar morphforge.exe.","補助プロセスmorphforge.exeを起動できません。"),
        ("内部进程状态锁已损坏，核心进程树将被终止。","Internal process-state lock failed; the core process tree will be terminated.","内部プロセス状態のロックが破損したため、コアプロセス群を終了します。"),
        ("MorphForge 核心启动超时","MorphForge core startup timed out","MorphForgeコアの起動がタイムアウト"),
        ("核心在 ","The core did not report its HTTP port within ","コアは"),
        (" 秒内没有报告 HTTP 监听端口。"," seconds.","秒以内にHTTP待受ポートを報告しませんでした。"),
        ("MorphForge 核心事件流中断","MorphForge core event stream interrupted","MorphForgeコアイベントストリームの中断"),
        ("核心事件通道意外关闭。","The core event channel closed unexpectedly.","コアのイベントチャンネルが予期せず閉じました。"),
        ("核心事件通道在启动完成前关闭。","The core event channel closed before startup completed.","起動完了前にコアのイベントチャンネルが閉じました。"),
        ("MorphForge WebView 导航失败","MorphForge WebView navigation failed","MorphForge WebViewのページ移動に失敗"),
        ("无法打开核心 HTTP 页面。","Cannot open the core HTTP page.","コアのHTTPページを開けません。"),
        ("MorphForge 核心事件错误","MorphForge core event error","MorphForgeコアイベントエラー"),
        ("MorphForge 核心已退出","MorphForge core exited","MorphForgeコアが終了しました"),
        ("过多并行WebView窗口","Too many concurrent WebView windows","同時実行中のWebViewが多すぎます"),
        ("日志：","Log: ","ログ："),
    ] { value = value.replace(zh, if lang == "ja" {ja} else {en}); }
    value
}
