//! HTTP contract tests never acquire a microphone, contact Tencent or call an LLM.
use axum::http::StatusCode;
use morphforge_app::server::App;
use reqwest::Client;
use serde_json::{json, Value};
struct Server {
    app: App,
    base: String,
    task: tokio::task::JoinHandle<()>,
}
impl Server {
    async fn new(root: &std::path::Path) -> Self {
        let app = App::open(root.into(), None).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let router = app.router();
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self { app, base, task }
    }
    async fn post(&self, path: &str, v: Value) -> (StatusCode, Value) {
        let r = Client::new()
            .post(format!("{}{}", self.base, path))
            .json(&v)
            .send()
            .await
            .unwrap();
        (r.status(), r.json().await.unwrap())
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.app.transport.inner.lock().unwrap().stop = true;
        self.task.abort();
    }
}
#[tokio::test]
async fn speech_settings_hide_credentials_and_reject_remote_provider() {
    let dir = tempfile::tempdir().unwrap();
    let s = Server::new(dir.path()).await;
    assert_eq!(
        s.post("/api/speech/config", json!({"provider":"remote"}))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let(status,v)=s.post("/api/speech/config",json!({"credentials":{"appId":"123456","secretId":"fixture-id","secretKey":"private-fixture-value"}})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["credentialsConfigured"], true);
    assert!(!v.to_string().contains("private-fixture-value"));
    assert!(!v.to_string().contains("fixture-id"));
    assert!(dir.path().join("speech-credentials.json").is_file());
    assert_eq!(
        s.post(
            "/api/speech/models/download",
            json!({"kind":"arbitrary-url"})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}
#[tokio::test]
async fn explicit_speech_text_applies_only_to_bound_project_and_does_not_start_ai() {
    let dir = tempfile::tempdir().unwrap();
    let s = Server::new(dir.path()).await;
    let (_, created) = s
        .post(
            "/api/text-panels",
            json!({"action":"create","id":"subtitle","baseRevision":0,"panel":{"text":"old"}}),
        )
        .await;
    let before = created["revision"].as_u64().unwrap();
    let(_,applied)=s.post("/api/speech/apply",json!({"text":"你好 ABC","target":{"kind":"text_panel","panelId":"subtitle"},"requestId":"voice-caption-fixture"})).await;
    assert_eq!(applied["ok"], true);
    {
        let w = s.app.workspace.lock().unwrap();
        assert_eq!(w.project.scene.text_panels[0].text, "你好 ABC");
        assert!(w.project.scene.revision > before);
    }
    assert_eq!(s.post("/api/speech/apply",json!({"text":"wrong","target":{"kind":"text_panel","panelId":"subtitle","projectId":"another-project"}})).await.0,StatusCode::CONFLICT);
    let (_, v) = s
        .post(
            "/api/speech/apply",
            json!({"text":"建一栋房子","target":{"kind":"modeling"}}),
        )
        .await;
    assert_eq!(v["ok"], true);
    assert!(s.app.harness.state()["run"].is_null());
    assert_eq!(s.app.speech.status()["state"]["draft"], "建一栋房子");
    assert!(!s.app.transport.config().enabled);
}
#[tokio::test]
async fn mini_preferences_are_restricted_and_durable() {
    let dir = tempfile::tempdir().unwrap();
    let s = Server::new(dir.path()).await;
    assert_eq!(
        s.post(
            "/api/desktop/window",
            json!({"action":"open","mode":"https://attacker"})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (_, v) = s
        .post(
            "/api/desktop/window",
            json!({"action":"open","mode":"progress"}),
        )
        .await;
    assert_eq!(v["preferences"]["mode"], "progress");
    assert_eq!(
        s.post("/api/desktop/window", json!({"action":"hide_main"}))
            .await
            .0,
        StatusCode::OK
    );
    let (_, v) = s
        .post("/api/desktop/window", json!({"action":"close"}))
        .await;
    assert_eq!(v["preferences"]["mainVisible"], true);
    assert!(dir.path().join("desktop-window.json").is_file());
}

#[tokio::test]
async fn builtin_vad_keeps_silence_off_the_cloud_and_cancel_releases_session() {
    let dir = tempfile::tempdir().unwrap();
    let s = Server::new(dir.path()).await;
    let (status,_)=s.post("/api/speech/config",json!({"provider":"tencent","credentials":{"appId":"123456","secretId":"unused-fixture","secretKey":"unused-fixture-key"}})).await;
    assert_eq!(status, StatusCode::OK);
    let (_, v) = s
        .post(
            "/api/speech/start",
            json!({"source":"pcm","target":{"kind":"modeling"}}),
        )
        .await;
    let id = v["state"]["sessionId"].as_str().unwrap().to_string();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let state = s.app.speech.status();
            if state["state"]["phase"] == "recording" {
                break;
            }
            assert!(state["state"]["error"].is_null(), "{state}");
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    use base64::Engine;
    let silence = base64::engine::general_purpose::STANDARD.encode(vec![0u8; 1024]);
    for _ in 0..94 {
        assert_eq!(
            s.post("/api/speech/audio", json!({"sessionId":id,"pcm":silence}))
                .await
                .0,
            StatusCode::OK
        );
    }
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while s.app.speech.status()["state"]["metrics"]["audioSamples"]
            .as_u64()
            .unwrap_or(0)
            < 94 * 512
        {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        s.app.speech.status()["state"]["metrics"]["cloudSessions"],
        0
    );
    assert_eq!(
        s.post("/api/speech/control", json!({"action":"cancel"}))
            .await
            .0,
        StatusCode::OK
    );
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while s.app.speech.status()["state"]["active"] == true {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(s.app.speech.status()["state"]["phase"], "cancelled");
    assert_eq!(
        s.app.speech.status()["state"]["metrics"]["cloudSessions"],
        0
    );
}

#[tokio::test]
async fn background_voice_submission_enters_the_existing_model_runtime() {
    use axum::{
        response::IntoResponse,
        routing::{get, post},
        Router,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let count = Arc::new(AtomicUsize::new(0));
    let request_count = count.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let router=Router::new().route("/models",get(||async{axum::Json(json!({"data":[]}))})).route("/responses",post(move||{let counter=request_count.clone();async move{counter.fetch_add(1,Ordering::SeqCst);([("content-type","text/event-stream")],"data: {\"type\":\"response.completed\",\"response\":{\"id\":\"voice-fixture\",\"status\":\"completed\",\"output\":[],\"usage\":{\"input_tokens\":20,\"output_tokens\":0,\"total_tokens\":20}}}\n\n").into_response()}}));
    let fixture = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let dir = tempfile::tempdir().unwrap();
    let s = Server::new(dir.path()).await;
    s.app
        .harness
        .configure_provider(&json!({"baseUrl":base,"apiType":"responses","apiKey":"local-fixture"}))
        .unwrap();
    s.app
        .harness
        .configure(&json!({"cacheGuard":"observe"}))
        .unwrap();
    let speech = s.app.speech.clone();
    let context = morphforge_app::speech::SpeechContext {
        workspace: s.app.workspace.clone(),
        harness: s.app.harness.clone(),
        transport: s.app.transport.clone(),
    };
    std::thread::spawn(move||speech.apply(&context,&json!({"text":"语音最终结果提交","submit":true,"target":{"kind":"modeling"},"requestId":"voice-from-native-thread"}))).join().expect("voice callback must not panic").unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while count.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 1);
    s.app.harness.control("stop").unwrap();
    fixture.abort();
}
