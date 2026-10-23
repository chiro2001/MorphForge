//! Local fake LLMs exercise real HTTP/provider/queue/persistence; no paid requests or microphone.
use axum::{http::StatusCode, routing::post, Json, Router};
use morphforge_app::server::App;
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
struct Fixture {
    _dir: tempfile::TempDir,
    app: App,
    base: String,
    requests: Arc<Mutex<Vec<Value>>>,
    active: Arc<AtomicUsize>,
    maximum: Arc<AtomicUsize>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}
impl Fixture {
    async fn new(api: &str, delay: Duration, fail: bool) -> Self {
        let requests = Arc::new(Mutex::new(vec![]));
        let active = Arc::new(AtomicUsize::new(0));
        let maximum = Arc::new(AtomicUsize::new(0));
        let (rows, inflight, max) = (requests.clone(), active.clone(), maximum.clone());
        let chat = api == "chat";
        let mock=Router::new().route(if chat{"/chat/completions"}else{"/responses"},post(move|Json(v):Json<Value>|{let(rows,inflight,max)=(rows.clone(),inflight.clone(),max.clone());async move{
   rows.lock().unwrap().push(v.clone());let n=inflight.fetch_add(1,Ordering::SeqCst)+1;max.fetch_max(n,Ordering::SeqCst);tokio::time::sleep(delay).await;inflight.fetch_sub(1,Ordering::SeqCst);
   if fail{return(StatusCode::TOO_MANY_REQUESTS,Json(json!({"error":{"message":"fixture-secret-value"}})));}
   let source=if chat{v["messages"][1]["content"][0]["text"].as_str().unwrap()}else{v["input"][0]["content"][0]["text"].as_str().unwrap()};
   let translated=format!("translated: {source}");
   (StatusCode::OK,Json(if chat{json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":translated}}],"usage":{"prompt_tokens":10,"completion_tokens":4}})}else{json!({"status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":translated}]}],"usage":{"input_tokens":10,"output_tokens":4}})}))
  }}));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let provider = format!("http://{}", listener.local_addr().unwrap());
        let first = tokio::spawn(async move { axum::serve(listener, mock).await.unwrap() });
        let dir = tempfile::tempdir().unwrap();
        let app = App::open(dir.path().into(), None).unwrap();
        app.harness
            .configure_provider(
                &json!({"apiType":api,"baseUrl":provider,"apiKey":"fixture-secret-value"}),
            )
            .unwrap();
        app.harness.tool("text_panels",&json!({"action":"create","id":"caption","baseRevision":0,"preset":"caption","panel":{"text":"initial"}}),None).await.unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let router = app.router();
        let second = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        Self {
            _dir: dir,
            app,
            base,
            requests,
            active,
            maximum,
            tasks: vec![first, second],
        }
    }
    async fn post(&self, path: &str, value: Value) -> Value {
        let r = reqwest::Client::new()
            .post(format!("{}/api/{path}", self.base))
            .json(&value)
            .send()
            .await
            .unwrap();
        let status = r.status();
        let v: Value = r.json().await.unwrap();
        assert!(status.is_success(), "{v}");
        v
    }
    async fn enable(&self, language: &str) {
        self.post("speech/config",json!({"translationEnabled":true,"translationLanguage":language,"translationModel":"translation-fixture"})).await;
    }
    async fn apply(&self, text: &str, key: &str) {
        self.post(
            "speech/apply",
            json!({"text":text,"requestId":key,"target":{"kind":"text_panel","panelId":"caption"}}),
        )
        .await;
    }
    async fn ready(&self, phase: &str) {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if self.app.speech.status()["state"]["translation"]["phase"] == phase {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }
    async fn requested(&self, n: usize) {
        tokio::time::timeout(Duration::from_secs(5), async {
            while self.requests.lock().unwrap().len() < n {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }
    fn text(&self) -> String {
        self.app.workspace.lock().unwrap().project.scene.text_panels[0]
            .text
            .clone()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.app
            .speech
            .configure(&json!({"translationEnabled":false}))
            .unwrap();
        self.app.transport.inner.lock().unwrap().stop = true;
        for t in &self.tasks {
            t.abort();
        }
    }
}
#[tokio::test]
async fn responses_and_chat_translate_three_languages_without_touching_modeling_history() {
    for api in ["responses", "chat"] {
        let f = Fixture::new(api, Duration::ZERO, false).await;
        let guard_before = f.app.harness.provider.usage(None);
        let before =
            serde_json::to_value(&f.app.harness.inner.lock().unwrap().conversation).unwrap();
        for (i, lang) in ["zh", "ja", "en"].iter().enumerate() {
            f.enable(lang).await;
            f.apply(&format!("字幕{i}"), &format!("apply{i}")).await;
            f.ready("ready").await;
            assert_eq!(f.text(), format!("字幕{i}\ntranslated: 字幕{i}"));
            let state = f.app.speech.status();
            assert_eq!(state["state"]["draft"], format!("字幕{i}"));
            assert_eq!(state["state"]["translation"]["language"], *lang);
        }
        let requests = f.requests.lock().unwrap();
        assert_eq!(requests.len(), 3);
        for (i, req) in requests.iter().enumerate() {
            let system = if api == "chat" {
                req["messages"][0]["content"].as_str().unwrap()
            } else {
                req["instructions"].as_str().unwrap()
            };
            assert!(system.contains(["Simplified Chinese", "Japanese", "English"][i]));
            assert_eq!(req["model"], "translation-fixture");
            assert_eq!(req["stream"], false);
            assert!(req["tools"].is_null());
        }
        assert_eq!(
            serde_json::to_value(&f.app.harness.inner.lock().unwrap().conversation).unwrap(),
            before
        );
        assert!(f.app.harness.state()["run"].is_null());
        assert_eq!(f.app.harness.provider.usage(None), guard_before);
        assert!(!f.app.transport.config().enabled);
        assert!(!f
            .app
            .speech
            .status()
            .to_string()
            .contains("fixture-secret-value"));
    }
}
#[tokio::test]
async fn superseded_results_never_publish_and_requests_have_one_worker() {
    let f = Fixture::new("responses", Duration::from_millis(350), false).await;
    f.enable("en").await;
    f.apply("old", "old").await;
    f.requested(1).await;
    f.apply("middle", "middle").await;
    f.apply("latest", "latest").await;
    tokio::time::sleep(Duration::from_millis(450)).await;
    assert_eq!(f.text(), "initial");
    f.ready("ready").await;
    assert_eq!(f.text(), "latest\ntranslated: latest");
    assert_eq!(f.requests.lock().unwrap().len(), 2);
    assert_eq!(f.maximum.load(Ordering::SeqCst), 1);
    assert_eq!(f.active.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn partial_burst_is_coalesced_and_final_reuses_translation() {
    let f = Fixture::new("responses", Duration::ZERO, false).await;
    f.enable("ja").await;
    let project = f.app.workspace.lock().unwrap().project.id.clone();
    let revision = f.app.workspace.lock().unwrap().project.scene.revision;
    for i in 0..12 {
        f.post(
            "text-panels/latest",
            json!({"id":"caption","projectId":project,"text":format!("partial {i}")}),
        )
        .await;
        tokio::time::sleep(Duration::from_millis(15)).await;
    }
    f.ready("ready").await;
    assert_eq!(f.requests.lock().unwrap().len(), 1);
    assert_eq!(
        f.app.workspace.lock().unwrap().project.scene.revision,
        revision
    );
    f.apply("partial 11", "final").await;
    f.ready("ready").await;
    assert_eq!(f.text(), "partial 11\ntranslated: partial 11");
    assert_eq!(f.requests.lock().unwrap().len(), 1);
    assert_eq!(
        f.app.speech.status()["state"]["translation"]["cacheHit"],
        true
    );
    f.apply("", "clear").await;
    assert_eq!(f.text(), "");
    assert_eq!(f.requests.lock().unwrap().len(), 1);
}
#[tokio::test]
async fn changing_language_or_disabling_cancels_old_translation() {
    let f = Fixture::new("responses", Duration::from_millis(400), false).await;
    f.enable("en").await;
    f.apply("old", "old").await;
    f.requested(1).await;
    f.post("speech/config", json!({"translationEnabled":false}))
        .await;
    f.apply("keep original", "off").await;
    tokio::time::sleep(Duration::from_millis(700)).await;
    assert_eq!(f.text(), "keep original");
    f.enable("zh").await;
    f.apply("Chinese caption", "new").await;
    f.ready("ready").await;
    assert_eq!(f.text(), "Chinese caption\ntranslated: Chinese caption");
    assert_eq!(
        f.app.speech.status()["state"]["translation"]["language"],
        "zh"
    );
}
#[tokio::test]
async fn failure_preserves_original_and_redacts_provider_errors() {
    let f = Fixture::new("responses", Duration::ZERO, true).await;
    f.enable("en").await;
    f.apply("原文仍可读", "failure").await;
    f.ready("error").await;
    assert_eq!(f.text(), "原文仍可读");
    let state = f.app.speech.status();
    assert!(state["state"]["translation"]["error"]
        .as_str()
        .unwrap()
        .contains("429"));
    assert!(!state.to_string().contains("fixture-secret-value"));
    assert_eq!(f.requests.lock().unwrap().len(), 1);
}
#[tokio::test]
async fn project_switch_and_clear_prevent_late_writes() {
    let f = Fixture::new("responses", Duration::from_millis(300), false).await;
    f.enable("en").await;
    f.apply("old", "old").await;
    f.requested(1).await;
    f.post("speech/control", json!({"action":"clear"})).await;
    tokio::time::sleep(Duration::from_millis(450)).await;
    assert_eq!(f.text(), "initial");
    f.apply("another", "new").await;
    f.requested(2).await;
    f.app.workspace.lock().unwrap().project.id = "different-project".into();
    f.ready("error").await;
    assert_eq!(f.text(), "initial");
}

#[tokio::test]
async fn manual_caption_edits_are_not_overwritten_by_late_translation() {
    let f = Fixture::new("responses", Duration::from_millis(350), false).await;
    f.enable("en").await;
    f.apply("old input", "old").await;
    f.requested(1).await;
    let revision = f.app.workspace.lock().unwrap().project.scene.revision;
    f.post("text-panels",json!({"action":"update","id":"caption","baseRevision":revision,"panel":{"text":"manual correction"}})).await;
    f.ready("error").await;
    assert_eq!(f.text(), "manual correction");
    assert_eq!(f.app.speech.status()["state"]["draft"], "old input");
}
