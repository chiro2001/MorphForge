use axum::{
    body::Body,
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use morphforge_app::{
    capture::CaptureBroker,
    harness::{Conversation, Harness, Run},
    provider::{Cancellation, Provider, ProviderSettings},
    store::{write_json, Workspace},
    transport::{Connection, Control, State as TransportState, Transport},
};
use morphforge_core::{Box3, Operation};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, VecDeque},
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tempfile::TempDir;
use tokio::{net::TcpListener, task::JoinHandle};

#[derive(Clone)]
enum Reply {
    Http(u16, Value),
    Sse(String),
    Delay(Duration, Box<Reply>),
}

#[derive(Clone)]
struct FakeState {
    replies: Arc<Mutex<VecDeque<Reply>>>,
    requests: Arc<Mutex<Vec<Value>>>,
}

struct FakeResponses {
    base: String,
    state: FakeState,
    task: JoinHandle<()>,
}

impl FakeResponses {
    async fn start(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address: SocketAddr = listener.local_addr().unwrap();
        let state = FakeState {
            replies: Arc::new(Mutex::new(replies.into())),
            requests: Arc::new(Mutex::new(Vec::new())),
        };
        let router = Router::new()
            .route("/responses", post(responses_handler))
            .with_state(state.clone());
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        Self {
            base: format!("http://127.0.0.1:{}", address.port()),
            state,
            task,
        }
    }

    fn request_count(&self) -> usize {
        self.state.requests.lock().unwrap().len()
    }

    fn requests(&self) -> Vec<Value> {
        self.state.requests.lock().unwrap().clone()
    }
}

impl Drop for FakeResponses {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn responses_handler(
    State(state): State<FakeState>,
    Json(body): Json<Value>,
) -> Response<Body> {
    state.requests.lock().unwrap().push(body);
    let reply = state.replies.lock().unwrap().pop_front();
    match reply {
        Some(Reply::Http(status, body)) => {
            (StatusCode::from_u16(status).unwrap(), Json(body)).into_response()
        }
        Some(Reply::Sse(body)) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/event-stream")
            .body(Body::from(body))
            .unwrap(),
        Some(Reply::Delay(delay, reply)) => {
            tokio::time::sleep(delay).await;
            render_reply(*reply)
        }
        None => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":{"message":"fake response script exhausted"}})),
        )
            .into_response(),
    }
}

fn render_reply(reply: Reply) -> Response<Body> {
    match reply {
        Reply::Http(status, body) => {
            (StatusCode::from_u16(status).unwrap(), Json(body)).into_response()
        }
        Reply::Sse(body) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/event-stream")
            .body(Body::from(body))
            .unwrap(),
        Reply::Delay(_, _) => panic!("nested delay reply is unsupported"),
    }
}

fn sse(events: impl IntoIterator<Item = Value>) -> Reply {
    Reply::Sse(
        events
            .into_iter()
            .map(|event| format!("data: {event}\n\n"))
            .collect(),
    )
}

fn completed(output: Vec<Value>) -> Reply {
    sse([json!({
        "type":"response.completed",
        "response":{
            "id":"fake-response",
            "status":"completed",
            "output":output,
            "usage":{"input_tokens":12,"output_tokens":4,"total_tokens":16}
        }
    })])
}

fn incomplete_tool_response() -> Reply {
    sse([
        json!({
            "type":"response.output_item.added",
            "output_index":0,
            "item":{"type":"function_call","call_id":"partial-call","name":"run_recipe","arguments":""}
        }),
        json!({
            "type":"response.function_call_arguments.delta",
            "output_index":0,
            "delta":"{\"code\":\"mf.box([0,0,0],[1,1,1]);\""
        }),
        json!({"type":"response.incomplete","response":{"id":"fake-response","status":"incomplete"}}),
    ])
}

fn workspace(dir: &TempDir) -> Arc<Mutex<Workspace>> {
    Arc::new(Mutex::new(
        Workspace::open(dir.path().to_path_buf()).unwrap(),
    ))
}

fn inert_transport() -> Transport {
    Transport {
        inner: Arc::new(Mutex::new(TransportState {
            config: Connection::default(),
            sent: vec![],
            desired: vec![],
            desired_revision: 0,
            sent_revision: None,
            seq: 0,
            brush: None,
            resets: 0,
            connected: false,
            last_error: String::new(),
            sent_count: 0,
            generation: 0,
            controls: BTreeMap::<String, Control>::new(),
            reset_pulse: 0,
            last_refresh: Instant::now(),
            stop: true,
        })),
    }
}

fn harness(workspace: Arc<Mutex<Workspace>>, base: &str) -> Harness {
    let root = workspace.lock().unwrap().root.clone();
    let provider = Provider::new_for_loopback_test(&root, base).unwrap();
    Harness::new(workspace, provider, CaptureBroker::new(), inert_transport())
}

async fn terminal_run(harness: &Harness) -> Run {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            if let Some(run) = harness.inner.lock().unwrap().run.clone() {
                if !["running", "compacting"].contains(&run.status.as_str()) {
                    return run;
                }
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("harness run should reach a terminal state")
}

#[tokio::test]
async fn incomplete_sse_tool_call_never_reaches_tool_executor() {
    let fake = FakeResponses::start(vec![incomplete_tool_response()]).await;
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(&dir);
    let harness = harness(workspace.clone(), &fake.base);

    harness.start("请做一个小盒子").unwrap();
    let run = terminal_run(&harness).await;

    assert_eq!(run.status, "failed");
    let workspace = workspace.lock().unwrap();
    assert_eq!(workspace.project.scene.revision, 0);
    assert!(!workspace
        .events
        .iter()
        .any(|event| event.kind == "tool.request"));
    assert!(workspace
        .events
        .iter()
        .any(|event| event.kind == "provider.incomplete"));
    assert_eq!(fake.request_count(), 1);
}

#[tokio::test]
async fn provider_retries_429_then_accepts_completed_sse() {
    let fake = FakeResponses::start(vec![
        Reply::Http(429, json!({"error":{"message":"rate limit"}})),
        completed(vec![]),
    ])
    .await;
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(&dir);
    let root = workspace.lock().unwrap().root.clone();
    let provider = Provider::new_for_loopback_test(&root, &fake.base).unwrap();

    let response = provider
        .response(
            json!({"model":"deepseek-flash","input":[],"stream":true}),
            workspace,
            "retry-run",
            &Cancellation::new(),
        )
        .await
        .unwrap();

    assert_eq!(response["status"], "completed");
    assert_eq!(fake.request_count(), 2);
}

#[tokio::test]
async fn invalid_tool_arguments_are_reported_and_the_harness_recovers() {
    let invalid = json!({
        "code":"mf.box([0,0,0],[1,1,1]);",
        "mode":"not-a-mode",
        "baseRevision":0,
        "seed":7
    });
    let fake = FakeResponses::start(vec![
        completed(vec![json!({
            "type":"function_call",
            "call_id":"invalid-call",
            "name":"run_recipe",
            "arguments":serde_json::to_string(&invalid).unwrap()
        })]),
        completed(vec![json!({
            "type":"message",
            "content":[{"type":"output_text","text":"参数错误已处理"}]
        })]),
    ])
    .await;
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(&dir);
    let harness = harness(workspace.clone(), &fake.base);

    harness.start("建一个盒子").unwrap();
    let run = terminal_run(&harness).await;

    assert_eq!(run.status, "completed");
    assert_eq!(run.message, "参数错误已处理");
    assert_eq!(fake.request_count(), 2);
    let requests = fake.requests();
    assert!(requests[1]["input"].as_array().unwrap().iter().any(|item| {
        item["type"] == "function_call_output" && item["call_id"] == "invalid-call"
    }));
    let workspace = workspace.lock().unwrap();
    assert_eq!(workspace.project.scene.revision, 0);
    assert!(workspace.events.iter().any(|event| {
        event.kind == "tool.result"
            && event.data["result"]["ok"] == false
            && event.data["callId"] == "invalid-call"
    }));
}

#[tokio::test]
async fn duplicate_recipe_request_id_returns_cached_result_before_validation() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(&dir);
    let cached = {
        let mut workspace = workspace.lock().unwrap();
        workspace
            .apply(
                &[Operation::Box {
                    value: Box3::new([0, 0, 0], [1, 1, 1], 1),
                }],
                Some(0),
                Some("same-request-id"),
                None,
            )
            .unwrap()
    };
    let harness = harness(workspace.clone(), "http://127.0.0.1:1");
    let duplicate = morphforge_app::recipe::RecipeRequest {
        code: "this must never be executed".into(),
        seed: 1,
        mode: "invalid-after-the-first-success".into(),
        base_revision: Some(999),
        request_id: Some("same-request-id".into()),
        max_ops: 1,
        dry_run: false,
    };

    let result = harness.run_recipe(duplicate, None).await.unwrap();

    assert_eq!(result, cached);
    let workspace = workspace.lock().unwrap();
    assert_eq!(workspace.project.scene.revision, 1);
    assert_eq!(
        workspace
            .events
            .iter()
            .filter(|event| event.kind == "geometry.applied")
            .count(),
        1
    );
    assert!(!workspace
        .events
        .iter()
        .any(|event| event.kind == "recipe.completed"));
}

#[tokio::test]
async fn failed_compact_keeps_old_context_and_records_failure() {
    let fake = FakeResponses::start(vec![sse([json!({
        "type":"response.incomplete",
        "response":{"id":"compact-response","status":"incomplete"}
    })])])
    .await;
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(&dir);
    let original = Conversation {
        input: vec![
            json!({"role":"user","content":"请保留整件作品的轮廓"}),
            json!({"type":"message","content":[{"type":"output_text","text":"目前完成底座"}]}),
        ],
        goals: vec!["保留完整轮廓".into()],
        summary: "compact前摘要".into(),
        boundaries: vec![2],
        compact_count: 4,
    };
    let root = workspace.lock().unwrap().root.clone();
    write_json(&root.join("runs/conversation.json"), &original).unwrap();
    let harness = harness(workspace.clone(), &fake.base);

    harness.control("compact").unwrap();
    let run = terminal_run(&harness).await;

    assert_eq!(run.status, "failed");
    assert_eq!(
        serde_json::to_value(&harness.inner.lock().unwrap().conversation).unwrap(),
        serde_json::to_value(&original).unwrap()
    );
    let workspace = workspace.lock().unwrap();
    assert!(workspace
        .events
        .iter()
        .any(|event| event.kind == "compact.started"));
    assert!(workspace
        .events
        .iter()
        .any(|event| { event.kind == "compact.failed" && event.data["contextPreserved"] == true }));
    assert!(workspace
        .events
        .iter()
        .any(|event| event.kind == "provider.incomplete"));
    let persisted: Conversation =
        serde_json::from_slice(&std::fs::read(root.join("runs/conversation.json")).unwrap())
            .unwrap();
    assert_eq!(
        serde_json::to_value(persisted).unwrap(),
        serde_json::to_value(original).unwrap()
    );
    let event_log = std::fs::read_to_string(root.join("events/events.jsonl")).unwrap();
    assert!(event_log.contains("compact.failed"));
}

#[tokio::test]
async fn reset_context_is_rejected_while_manual_compact_is_running() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(&dir);
    let harness = harness(workspace, "http://127.0.0.1:1");
    let original = json!({"role":"user","content":"保留当前上下文"});
    {
        let mut state = harness.inner.lock().unwrap();
        state.conversation.input.push(original.clone());
        state.run_deadline = Some(tokio::time::Instant::now() + Duration::from_secs(900));
        state.run = Some(Run {
            id: "compact-run".into(),
            status: "compacting".into(),
            prompt: "手动compact".into(),
            started_at: "test".into(),
            finished_at: None,
            steps: 0,
            tokens: 0,
            message: String::new(),
            error: String::new(),
        });
    }

    assert!(harness.control("reset_context").is_err());
    assert_eq!(
        harness.inner.lock().unwrap().conversation.input,
        vec![original]
    );
}

#[tokio::test]
async fn run_deadline_includes_paused_time() {
    let fake = FakeResponses::start(vec![]).await;
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(&dir);
    let harness = harness(workspace, &fake.base);
    harness.inner.lock().unwrap().settings.max_run_seconds = 1;

    harness.start("等待暂停预算结束").unwrap();
    harness.control("pause").unwrap();
    let run = terminal_run(&harness).await;

    assert_eq!(run.status, "failed");
    assert!(run.error.starts_with("RUN_TIME_BUDGET"));
    assert_eq!(
        fake.request_count(),
        0,
        "paused run should expire without a provider call"
    );
}

#[tokio::test]
async fn run_deadline_cancels_a_slow_provider_request() {
    let fake = FakeResponses::start(vec![Reply::Delay(
        Duration::from_secs(3),
        Box::new(completed(vec![])),
    )])
    .await;
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(&dir);
    let harness = harness(workspace, &fake.base);
    harness.inner.lock().unwrap().settings.max_run_seconds = 1;

    harness.start("请求应在运行期限内被取消").unwrap();
    let run = terminal_run(&harness).await;

    assert_eq!(run.status, "failed");
    assert!(run.error.starts_with("RUN_TIME_BUDGET"));
    assert_eq!(fake.request_count(), 1);
}

#[test]
fn old_harness_settings_default_to_a_900_second_run_budget() {
    let settings: ProviderSettings = serde_json::from_value(json!({
        "model":"deepseek-flash",
        "reasoning":"high",
        "maxOutputTokens":16384,
        "maxSteps":24,
        "maxRunTokens":250000,
        "compactTokens":60000,
        "autoCompact":true,
        "uploadImages":true
    }))
    .unwrap();
    assert_eq!(settings.max_run_seconds, 900);
}
