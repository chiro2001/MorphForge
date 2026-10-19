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
    fonts::FontCatalog,
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
            .route("/chat/completions", post(responses_handler))
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

fn function_call(call_id: &str, name: &str, arguments: Value) -> Value {
    json!({
        "type":"function_call",
        "call_id":call_id,
        "name":name,
        "arguments":serde_json::to_string(&arguments).unwrap(),
    })
}

async fn wait_for_event(workspace: &Arc<Mutex<Workspace>>, kind: &str) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if workspace
                .lock()
                .unwrap()
                .events
                .iter()
                .any(|event| event.kind == kind)
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {kind}"));
}

async fn complete_pending_capture(broker: CaptureBroker, workspace: Arc<Mutex<Workspace>>) {
    use base64::Engine;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let job = broker.claim();
            if !job.is_null() {
                let bytes = b"\x89PNG\r\n\x1a\nfixture";
                let image = format!(
                    "data:image/png;base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(bytes)
                );
                let result = json!({
                    "id":job["id"],
                    "revision":job["revision"],
                    "images":[{"view":"hero","data":image}],
                });
                broker
                    .complete(&mut workspace.lock().unwrap(), &result)
                    .unwrap();
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("capture job should be claimed and completed")
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

fn seed_capture(workspace: &Arc<Mutex<Workspace>>) -> String {
    let mut workspace = workspace.lock().unwrap();
    let artifact = workspace
        .artifact(b"\x89PNG\r\n\x1a\nfixture", "png")
        .unwrap();
    let revision = workspace.project.scene.revision;
    workspace
        .event(
            "capture.completed",
            None,
            json!({
                "ok":true,
                "revision":revision,
                "images":[{"view":"hero","artifact":artifact}],
            }),
        )
        .unwrap();
    artifact
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
                if ![
                    "running",
                    "compacting",
                    "interrupting",
                    "pausing",
                    "stopping",
                ]
                .contains(&run.status.as_str())
                {
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
async fn token_budget_uses_provider_usage_and_allows_one_input_overrun() {
    let fake = FakeResponses::start(vec![sse([json!({
        "type":"response.completed",
        "response":{
            "id":"overrun-response",
            "status":"completed",
            "output":[{"type":"message","content":[{"type":"output_text","text":"先保存当前进度。"}]}],
            "usage":{"input_tokens":1800,"output_tokens":20,"total_tokens":1820}
        }
    })])])
    .await;
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(&dir);
    let harness = harness(workspace.clone(), &fake.base);
    {
        let mut state = harness.inner.lock().unwrap();
        state.settings.max_run_tokens = 1000;
        state.settings.max_output_tokens = 4096;
        state.settings.auto_compact = false;
    }

    harness.start("检查当前作品并收尾").unwrap();
    let run = terminal_run(&harness).await;

    assert_eq!(run.status, "completed");
    assert_eq!(run.tokens, 1820);
    assert_eq!(fake.request_count(), 1);
    assert_eq!(fake.requests()[0]["max_output_tokens"], 1000);
    let prepared = workspace
        .lock()
        .unwrap()
        .events
        .iter()
        .find(|event| event.kind == "harness.request.prepared")
        .unwrap()
        .data
        .clone();
    assert!(prepared["estimatedInputTokens"].as_u64().unwrap() > 1000);
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
async fn max_output_incomplete_retries_compactly_without_dispatching_partial_tools() {
    let partial = Reply::Sse(format!(
        "data: {}\n\n",
        json!({
            "type":"response.incomplete",
            "response":{
                "id":"truncated-response",
                "status":"incomplete",
                "incomplete_details":{"reason":"max_output_tokens"},
                "usage":{"input_tokens":100,"output_tokens":16384,"reasoning_tokens":16000,"total_tokens":16484},
                "output":[{
                    "type":"function_call",
                    "call_id":"partial-recipe",
                    "name":"run_recipe",
                    "arguments":"{\"code\":\"mf.box([0,0,0],[1,1,1]);"
                }]
            }
        })
    ));
    let fake = FakeResponses::start(vec![
        partial,
        completed(vec![json!({
            "type":"message",
            "content":[{"type":"output_text","text":"已用较短请求完成。"}]
        })]),
    ])
    .await;
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(&dir);
    let harness = harness(workspace.clone(), &fake.base);

    harness.start("先画一个大厅，然后检查视图").unwrap();
    let run = terminal_run(&harness).await;

    assert_eq!(run.status, "completed");
    assert_eq!(run.steps, 2, "incomplete responses spend a step");
    assert_eq!(
        run.tokens, 16_500,
        "incomplete usage counts toward the run budget"
    );
    assert_eq!(fake.request_count(), 2);
    assert_eq!(workspace.lock().unwrap().project.scene.revision, 0);
    assert!(!workspace
        .lock()
        .unwrap()
        .events
        .iter()
        .any(|event| event.kind == "tool.request"));
    let requests = fake.requests();
    assert!(requests[1]["instructions"]
        .as_str()
        .unwrap()
        .contains("max_output_tokens"));
    assert!(requests[1]["input"].to_string().contains("先画一个大厅"));
    assert!(!requests[1]["input"].to_string().contains("partial-recipe"));
    assert!(workspace
        .lock()
        .unwrap()
        .events
        .iter()
        .any(|event| event.kind == "provider.incomplete"));
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
async fn steer_applies_after_tool_batch_while_queue_waits_for_turn_end() {
    let first_call = function_call(
        "capture-call",
        "capture_views",
        json!({"revision":0,"views":["hero"]}),
    );
    let fake = FakeResponses::start(vec![
        Reply::Delay(
            Duration::from_millis(120),
            Box::new(completed(vec![first_call])),
        ),
        completed(vec![json!({
            "type":"message",
            "content":[{"type":"output_text","text":"当前阶段完成"}]
        })]),
        completed(vec![json!({
            "type":"message",
            "content":[{"type":"output_text","text":"排队请求完成"}]
        })]),
    ])
    .await;
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(&dir);
    let harness = harness(workspace.clone(), &fake.base);
    harness.inner.lock().unwrap().settings.model = "deepseek-v4-pro".into();
    harness.start("搭建大厅").unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while fake.request_count() == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("first request should start");
    let run_id = harness
        .inner
        .lock()
        .unwrap()
        .run
        .as_ref()
        .unwrap()
        .id
        .clone();

    let steer = harness
        .message(&json!({
            "prompt":"把门廊向左扩一格",
            "mode":"steer",
            "requestId":"steer-1",
            "runId":run_id
        }))
        .unwrap();
    let queued = harness
        .message(&json!({
            "prompt":"再加一个钟面",
            "mode":"queue",
            "requestId":"queue-1",
            "runId":run_id
        }))
        .unwrap();
    assert_eq!(steer["accepted"], true);
    assert_eq!(queued["accepted"], true);
    assert_eq!(
        harness
            .message(&json!({
                "prompt":"把门廊向左扩一格",
                "mode":"steer",
                "requestId":"steer-1",
                "runId":run_id,
            }))
            .unwrap(),
        steer
    );
    assert!(harness
        .message(&json!({
            "prompt":"不同内容",
            "mode":"steer",
            "requestId":"steer-1",
            "runId":run_id,
        }))
        .unwrap_err()
        .to_string()
        .starts_with("REQUEST_ID_CONFLICT"));
    assert!(harness
        .message(&json!({
            "prompt":"过期run",
            "mode":"steer",
            "requestId":"stale-run",
            "runId":"stale-run-id",
        }))
        .unwrap_err()
        .to_string()
        .starts_with("RUN_CONFLICT"));

    let broker = harness.capture.clone();
    let capture_workspace = workspace.clone();
    let capture_worker = tokio::spawn(async move {
        complete_pending_capture(broker, capture_workspace).await;
    });

    let run = terminal_run(&harness).await;
    assert_eq!(run.status, "completed");
    capture_worker.await.unwrap();
    assert_eq!(workspace.lock().unwrap().project.scene.revision, 0);
    let requests = fake.requests();
    assert_eq!(requests.len(), 3);
    assert!(requests[1]["input"]
        .to_string()
        .contains("把门廊向左扩一格"));
    assert!(!requests[1]["input"].to_string().contains("再加一个钟面"));
    assert!(requests[1]["input"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["type"] == "function_call_output" && item["call_id"] == "capture-call"));
    assert!(requests[2]["input"].to_string().contains("再加一个钟面"));
    let workspace = workspace.lock().unwrap();
    assert!(workspace.events.iter().any(|event| {
        event.kind == "harness.message.applied" && event.data["requestId"] == "steer-1"
    }));
    assert!(workspace.events.iter().any(|event| {
        event.kind == "harness.message.applied" && event.data["requestId"] == "queue-1"
    }));
}

#[tokio::test]
async fn interrupt_cancels_inflight_provider_request_and_continues_same_run() {
    let partial_call = function_call(
        "never-dispatch",
        "run_recipe",
        json!({"code":"mf.box([0,0,0],[1,1,1]);","mode":"apply","baseRevision":0,"seed":1}),
    );
    let fake = FakeResponses::start(vec![
        Reply::Delay(
            Duration::from_secs(2),
            Box::new(completed(vec![partial_call])),
        ),
        completed(vec![json!({
            "type":"message",
            "content":[{"type":"output_text","text":"根据插话完成"}]
        })]),
    ])
    .await;
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(&dir);
    let harness = harness(workspace.clone(), &fake.base);
    harness.start("建一个盒子").unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while fake.request_count() == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("first request should start");
    let (run_id, old_epoch) = {
        let state = harness.inner.lock().unwrap();
        let run = state.run.as_ref().unwrap();
        (run.id.clone(), run.epoch)
    };
    let ack = harness
        .message(&json!({
            "prompt":"先不要建盒子，改看一下大门比例",
            "mode":"interrupt",
            "requestId":"interrupt-request",
            "runId":run_id,
        }))
        .unwrap();
    assert_eq!(ack["phase"], "interrupting");
    assert!(ack["epoch"].as_u64().unwrap() > old_epoch);

    let run = terminal_run(&harness).await;
    assert_eq!(run.status, "completed");
    assert_eq!(run.id, run_id);
    assert!(run.epoch > old_epoch);
    assert_eq!(workspace.lock().unwrap().project.scene.revision, 0);
    assert_eq!(fake.request_count(), 2);
    let requests = fake.requests();
    assert!(requests[1]["input"].to_string().contains("先不要建盒子"));
    assert!(!requests[1]["input"].to_string().contains("never-dispatch"));
    let workspace = workspace.lock().unwrap();
    assert!(!workspace
        .events
        .iter()
        .any(|event| event.kind == "tool.request"));
    assert!(workspace
        .events
        .iter()
        .any(|event| event.kind == "provider.cancelled"));
    assert!(workspace.events.iter().any(|event| {
        event.kind == "harness.message.applied"
            && event.data["requestId"] == "interrupt-request"
            && event.data["epoch"].as_u64().unwrap() > old_epoch
    }));
}

#[tokio::test]
async fn interrupt_during_tool_batch_records_cancelled_result_before_continuation() {
    let capture_call = function_call(
        "capture-call",
        "capture_views",
        json!({"revision":0,"views":["hero"]}),
    );
    let fake = FakeResponses::start(vec![
        completed(vec![capture_call]),
        completed(vec![json!({
            "type":"message",
            "content":[{"type":"output_text","text":"已按新要求继续"}]
        })]),
    ])
    .await;
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(&dir);
    let harness = harness(workspace.clone(), &fake.base);
    harness.start("先截图").unwrap();
    wait_for_event(&workspace, "tool.request").await;
    let run_id = harness
        .inner
        .lock()
        .unwrap()
        .run
        .as_ref()
        .unwrap()
        .id
        .clone();
    harness
        .message(&json!({
            "prompt":"截图先停，先调整门廊",
            "mode":"interrupt",
            "requestId":"interrupt-tool",
            "runId":run_id,
        }))
        .unwrap();

    let run = terminal_run(&harness).await;
    assert_eq!(run.status, "completed");
    assert_eq!(workspace.lock().unwrap().project.scene.revision, 0);
    let requests = fake.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[1]["input"].to_string().contains("截图先停"));
    let outputs = requests[1]["input"].as_array().unwrap();
    assert!(outputs
        .iter()
        .any(|item| item["type"] == "function_call" && item["call_id"] == "capture-call"));
    assert!(outputs.iter().any(|item| {
        item["type"] == "function_call_output"
            && item["call_id"] == "capture-call"
            && item["output"].to_string().contains("CANCELLED")
    }));
    let workspace = workspace.lock().unwrap();
    assert!(workspace.events.iter().any(|event| {
        event.kind == "tool.result"
            && event.data["callId"] == "capture-call"
            && event.data["cancelled"] == true
    }));
    assert!(!workspace
        .events
        .iter()
        .any(|event| event.kind == "capture.completed"));
}

#[tokio::test]
async fn stop_during_tool_call_keeps_logs_and_pairs_the_call_without_late_geometry() {
    let capture_call = function_call(
        "stop-capture",
        "capture_views",
        json!({"revision":0,"views":["hero"]}),
    );
    let fake = FakeResponses::start(vec![completed(vec![capture_call])]).await;
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(&dir);
    let harness = harness(workspace.clone(), &fake.base);
    harness.start("捕获视图").unwrap();
    wait_for_event(&workspace, "tool.request").await;
    harness.control("stop").unwrap();
    let run = terminal_run(&harness).await;

    assert_eq!(run.status, "stopped");
    assert_eq!(workspace.lock().unwrap().project.scene.revision, 0);
    let requests = fake.requests();
    assert_eq!(requests.len(), 1);
    let events = workspace.lock().unwrap();
    assert!(events.events.iter().any(|event| {
        event.kind == "tool.result"
            && event.data["callId"] == "stop-capture"
            && event.data["cancelled"] == true
    }));
    assert!(events
        .events
        .iter()
        .any(|event| event.kind == "harness.finished"));
    assert!(!events
        .events
        .iter()
        .any(|event| event.kind == "capture.completed"));
}

#[tokio::test]
async fn voxel_text_tool_previews_compactly_and_applies_with_durable_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(&dir);
    let root = workspace.lock().unwrap().root.clone();
    let provider = Provider::new_for_loopback_test(&root, "http://127.0.0.1:1").unwrap();
    let harness = Harness::new_with_fonts(
        workspace.clone(),
        provider,
        CaptureBroker::new(),
        inert_transport(),
        Arc::new(FontCatalog::load_builtin().unwrap()),
    );
    let request = json!({
        "text":"A",
        "fontId":"builtin:noto-sans-sc",
        "fontSize":12,
        "depth":1,
        "origin":[0,0,0],
        "plane":"xy",
        "facing":"positive",
        "align":"left",
        "styleId":1,
        "group":"title",
    });
    let mut preview_args = request.clone();
    preview_args["mode"] = json!("preview");
    let preview = harness
        .tool("voxel_text", &preview_args, None)
        .await
        .unwrap();
    assert_eq!(preview["ok"], true);
    assert!(preview["previewFingerprint"].as_str().unwrap().len() == 64);
    assert!(preview["stats"]["boxCount"].as_u64().unwrap() > 0);
    assert!(preview["preview"].get("operations").is_none());
    let apply_template = preview["applyTemplate"].clone();
    assert_eq!(apply_template["mode"], "apply");
    assert_eq!(apply_template["baseRevision"], 0);
    assert_eq!(
        apply_template["previewFingerprint"],
        preview["previewFingerprint"]
    );
    assert_eq!(apply_template["origin"], json!([0, 0, 0]));
    assert_eq!(apply_template["align"], "left");
    assert_eq!(apply_template["group"], "title");
    assert!(apply_template.get("requestId").is_none());

    let mut apply_args = apply_template;
    apply_args["requestId"] = json!("text-apply-1");
    let applied = harness.tool("voxel_text", &apply_args, None).await.unwrap();
    assert_eq!(applied["ok"], true);
    assert_eq!(applied["result"]["revision"], 1);
    assert_eq!(applied["result"]["requestId"], "text-apply-1");
    assert_eq!(
        applied["result"]["requestFingerprint"],
        preview["previewFingerprint"]
    );
    assert!(applied["result"].get("scene").is_none());

    let replay = harness.tool("voxel_text", &apply_args, None).await.unwrap();
    assert_eq!(replay["replayed"], true);
    assert_eq!(workspace.lock().unwrap().project.scene.revision, 1);
    let mut changed_args = apply_args.clone();
    changed_args["text"] = json!("B");
    let mut changed_request = changed_args.clone();
    for key in ["mode", "baseRevision", "requestId", "previewFingerprint"] {
        changed_request.as_object_mut().unwrap().remove(key);
    }
    let changed_request: morphforge_core::text::VoxelTextRequest =
        serde_json::from_value(changed_request).unwrap();
    changed_args["previewFingerprint"] =
        json!(morphforge_app::fonts::request_fingerprint(&changed_request).unwrap());
    let conflict = harness
        .tool("voxel_text", &changed_args, None)
        .await
        .unwrap_err();
    assert!(conflict.to_string().starts_with("REQUEST_ID_CONFLICT"));
}

#[tokio::test]
async fn artworks_save_resolves_latest_current_capture_images() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(&dir);
    let (hero, front, top) = {
        let mut workspace = workspace.lock().unwrap();
        let old_hero = workspace.artifact(b"old-hero", "png").unwrap();
        workspace
            .event(
                "capture.completed",
                None,
                json!({
                    "ok":true,
                    "revision":0,
                    "images":[{"view":"hero","artifact":old_hero}]
                }),
            )
            .unwrap();
        let hero = workspace.artifact(b"hero", "png").unwrap();
        let front = workspace.artifact(b"front", "png").unwrap();
        let top = workspace.artifact(b"top", "png").unwrap();
        workspace
            .event(
                "capture.completed",
                None,
                json!({
                    "ok":true,
                    "revision":0,
                    "images":[
                        {"view":"hero","artifact":hero},
                        {"view":"front","artifact":front},
                        {"view":"top","artifact":top}
                    ]
                }),
            )
            .unwrap();
        (hero, front, top)
    };
    let harness = harness(workspace.clone(), "http://127.0.0.1:1");

    let explicit = harness
        .tool(
            "artworks",
            &json!({"action":"save","title":"视角解析","views":["hero","front","top"]}),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        explicit["artwork"]["views"],
        json!([hero.clone(), front.clone(), top.clone()])
    );
    assert_eq!(explicit["artwork"]["thumbnail"], hero);

    let defaulted = harness
        .tool(
            "artworks",
            &json!({"action":"save","title":"默认截图"}),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        defaulted["artwork"]["views"],
        json!([hero.clone(), front, top])
    );
    assert_eq!(defaulted["artwork"]["thumbnail"], hero);

    let missing_view = harness
        .tool("artworks", &json!({"action":"save","views":["side"]}), None)
        .await
        .unwrap_err();
    assert!(missing_view
        .to_string()
        .starts_with("ARTWORK_CAPTURE_MISSING"));
    let missing_artifact = harness
        .tool(
            "artworks",
            &json!({"action":"save","views":["does-not-exist.png"]}),
            None,
        )
        .await
        .unwrap_err();
    assert!(missing_artifact.to_string().contains("artifact不存在"));
}

#[tokio::test]
async fn finish_artwork_commits_saves_summary_and_skips_a_followup_request() {
    let summary = "作品已完成并保存。";
    let fake = FakeResponses::start(vec![sse([json!({
        "type":"response.completed",
        "response":{
            "id":"finish-response",
            "status":"completed",
            "output":[function_call("finish-call", "finish_artwork", json!({
                "title":"收尾作品",
                "summary":summary,
                "tags":["finish-test"]
            }))],
            "usage":{"input_tokens":1200,"output_tokens":80,"total_tokens":1280}
        }
    })])])
    .await;
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(&dir);
    let hero = seed_capture(&workspace);
    let harness = harness(workspace.clone(), &fake.base);
    {
        let mut state = harness.inner.lock().unwrap();
        state.settings.max_run_tokens = 1000;
        state.settings.max_output_tokens = 4096;
        state.settings.auto_compact = false;
    }

    harness.start("检查并完成作品").unwrap();
    let run = terminal_run(&harness).await;

    assert_eq!(run.status, "completed");
    assert_eq!(run.message, summary);
    assert_eq!(run.tokens, 1280);
    assert_eq!(fake.request_count(), 1);
    assert_eq!(fake.requests()[0]["max_output_tokens"], 1000);
    let workspace = workspace.lock().unwrap();
    assert_eq!(workspace.project.commits.len(), 1);
    let artwork = workspace.library().unwrap().remove(0);
    assert_eq!(artwork.title, "收尾作品");
    assert_eq!(artwork.thumbnail.as_deref(), Some(hero.as_str()));
    assert_eq!(artwork.views, vec![hero.clone()]);
    let finish_request = workspace
        .events
        .iter()
        .find(|event| event.kind == "tool.request" && event.data["callId"] == "finish-call")
        .unwrap();
    assert_eq!(finish_request.data["name"], "finish_artwork");
    let finish_result = workspace
        .events
        .iter()
        .find(|event| event.kind == "tool.result" && event.data["callId"] == "finish-call")
        .unwrap();
    assert_eq!(finish_result.data["result"]["finished"], true);
    let terminal = workspace
        .events
        .iter()
        .find(|event| event.kind == "harness.finished")
        .unwrap();
    assert_eq!(terminal.data["status"], "completed");
    assert_eq!(terminal.data["message"], summary);
}

#[tokio::test]
async fn failed_finish_artwork_stays_active_and_can_be_retried_after_capture() {
    let title = "可恢复收尾";
    let summary = "作品已经保存。";
    let failed_finish = json!({
        "type":"response.completed",
        "response":{
            "id":"finish-failed-response",
            "status":"completed",
            "output":[function_call("finish-failed", "finish_artwork", json!({
                "title":title,
                "summary":summary
            }))],
            "usage":{"input_tokens":100,"output_tokens":20,"total_tokens":120}
        }
    });
    let retry_finish = Reply::Delay(
        Duration::from_millis(250),
        Box::new(sse([json!({
            "type":"response.completed",
            "response":{
                "id":"finish-retry-response",
                "status":"completed",
                "output":[function_call("finish-retry", "finish_artwork", json!({
                    "title":title,
                    "summary":summary
                }))],
                "usage":{"input_tokens":100,"output_tokens":20,"total_tokens":120}
            }
        })])),
    );
    let fake = FakeResponses::start(vec![sse([failed_finish]), retry_finish]).await;
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(&dir);
    let harness = harness(workspace.clone(), &fake.base);

    harness.start("检查后保存作品").unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if fake.request_count() >= 2 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("finish failure should allow a recovery request");
    assert_eq!(
        harness.inner.lock().unwrap().run.as_ref().unwrap().status,
        "running"
    );
    {
        let workspace = workspace.lock().unwrap();
        let failed = workspace
            .events
            .iter()
            .find(|event| event.kind == "tool.result" && event.data["callId"] == "finish-failed")
            .unwrap();
        assert_eq!(failed.data["result"]["ok"], false);
        assert_eq!(failed.data["result"]["error"]["code"], "TOOL_ERROR");
        assert!(workspace.project.commits.is_empty());
        assert!(workspace.library().unwrap().is_empty());
    }

    seed_capture(&workspace);
    let run = terminal_run(&harness).await;

    assert_eq!(run.status, "completed");
    assert_eq!(run.message, summary);
    assert_eq!(fake.request_count(), 2);
    assert_eq!(workspace.lock().unwrap().project.commits.len(), 1);
}

#[tokio::test]
async fn queue_and_steer_messages_continue_after_finish_artwork() {
    for mode in ["queue", "steer"] {
        let title = format!("{mode}续作");
        let summary = "当前作品已保存。";
        let first_response = Reply::Delay(
            Duration::from_millis(150),
            Box::new(sse([json!({
                "type":"response.completed",
                "response":{
                    "id":"finish-before-inbox",
                    "status":"completed",
                    "output":[function_call("finish-before-inbox", "finish_artwork", json!({
                        "title":title,
                        "summary":summary
                    }))],
                    "usage":{"input_tokens":100,"output_tokens":20,"total_tokens":120}
                }
            })])),
        );
        let fake = FakeResponses::start(vec![
            first_response,
            completed(vec![json!({
                "type":"message",
                "content":[{"type":"output_text","text":"补充要求已处理。"}]
            })]),
        ])
        .await;
        let dir = tempfile::tempdir().unwrap();
        let workspace = workspace(&dir);
        seed_capture(&workspace);
        let harness = harness(workspace.clone(), &fake.base);

        harness.start("完成并保存当前作品").unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            while fake.request_count() == 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("finish request should start");
        let run_id = harness
            .inner
            .lock()
            .unwrap()
            .run
            .as_ref()
            .unwrap()
            .id
            .clone();
        let request_id = format!("{mode}-after-finish");
        harness
            .message(&json!({
                "prompt":"再检查入口灯是否清楚",
                "mode":mode,
                "requestId":request_id,
                "runId":run_id,
            }))
            .unwrap();
        let run = terminal_run(&harness).await;

        assert_eq!(run.status, "completed");
        assert_eq!(fake.request_count(), 2);
        assert!(fake.requests()[1]["input"]
            .to_string()
            .contains("再检查入口灯是否清楚"));
        assert!(workspace.lock().unwrap().events.iter().any(|event| {
            event.kind == "harness.message.applied" && event.data["requestId"] == request_id
        }));
    }
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
        goal: None,
        reference_images: Vec::new(),
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
        state.phase = "compacting".into();
        state.epoch = 1;
        state.worker_epoch = Some(1);
        state.run = Some(Run {
            id: "compact-run".into(),
            epoch: 1,
            status: "compacting".into(),
            prompt: "手动compact".into(),
            started_at: "test".into(),
            finished_at: None,
            steps: 0,
            tokens: 0,
            active_request_id: None,
            prompt_version: "test".into(),
            tools_version: "test".into(),
            language: "zh-CN".into(),
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
async fn pause_freezes_active_budget_and_resume_continues_the_same_run() {
    let fake = FakeResponses::start(vec![
        Reply::Delay(Duration::from_millis(1500), Box::new(completed(vec![]))),
        completed(vec![]),
    ])
    .await;
    let dir = tempfile::tempdir().unwrap();
    let workspace = workspace(&dir);
    let harness = harness(workspace, &fake.base);
    harness.inner.lock().unwrap().settings.max_run_seconds = 1;

    harness.start("暂停期间冻结预算并在恢复后继续").unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while fake.request_count() == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("provider request should start");
    harness.control("pause").unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if harness.state()["interaction"]["phase"] == "paused" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("cancelled request should settle into paused phase");
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let paused = harness.state();
    assert_eq!(paused["interaction"]["phase"], "paused");
    assert_eq!(paused["run"]["status"], "paused");
    assert_eq!(fake.request_count(), 1);

    harness.control("resume").unwrap();
    let run = terminal_run(&harness).await;

    assert_eq!(run.status, "completed");
    assert_eq!(run.epoch, 3, "pause and resume each advance the run epoch");
    assert_eq!(fake.request_count(), 2);
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

#[test]
fn presets_apply_budgets_and_explicit_overrides_without_restricting_models() {
    let dir = TempDir::new().unwrap();
    let harness = harness(workspace(&dir), "http://127.0.0.1:1");
    let response = harness
        .configure(&json!({"preset":"detailed","model":"my-local-vision-model","maxSteps":96}))
        .unwrap();
    assert_eq!(response["settings"]["maxSteps"], 96);
    assert_eq!(response["settings"]["maxRunTokens"], 2000000);
    assert_eq!(response["settings"]["goalMinReviewRounds"], 3);
    assert_eq!(response["settings"]["goalMode"], true);
    let sketch = harness.configure(&json!({"preset":"sketch"})).unwrap();
    assert_eq!(sketch["settings"]["maxSteps"], 12);
    assert_eq!(sketch["settings"]["goalMode"], false);
    assert!(harness
        .configure(&json!({"contextWindowTokens":8192}))
        .is_err());
    assert!(harness.configure(&json!({"preset":"unknown"})).is_err());
}

#[tokio::test]
async fn goal_continues_until_no_progress_is_bounded_and_persisted() {
    let fake = FakeResponses::start((0..4).map(|_|completed(vec![json!({"type":"message","role":"assistant","content":[{"type":"output_text","text":"完成了"}]} )])).collect()).await;
    let dir = TempDir::new().unwrap();
    let h = harness(workspace(&dir), &fake.base);
    h.configure(&json!({"goalMode":true})).unwrap();
    h.start("创建精细的作品").unwrap();
    let run = terminal_run(&h).await;
    assert_eq!(run.status, "blocked");
    assert_eq!(fake.request_count(), 4);
    assert_eq!(h.state()["context"]["goal"]["noProgressAttempts"], 3);
    let persisted: Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("runs/conversation.json")).unwrap())
            .unwrap();
    assert_eq!(persisted["goal"]["status"], "blocked");
    let before_goal = h.state()["context"]["goal"].clone();
    fake.state.replies.lock().unwrap().push_back(completed(vec![json!({"type":"message","role":"assistant","content":[{"type":"output_text","text":"保留目标和未完成原因的摘要。"}]} )]));
    h.control("compact").unwrap();
    assert_eq!(terminal_run(&h).await.status, "completed");
    assert_eq!(
        h.state()["context"]["goal"],
        before_goal,
        "compact must preserve independent Goal progress and acceptance criteria"
    );
}

#[tokio::test]
async fn goal_requires_changed_geometry_captures_then_saves_and_keeps_images_local() {
    let call = |id, name, args| completed(vec![function_call(id, name, args)]);
    let fake = FakeResponses::start(vec![]).await;
    let dir = TempDir::new().unwrap();
    let w = workspace(&dir);
    let h = harness(w.clone(), &fake.base);
    h.configure(&json!({"goalMode":true,"goalMinReviewRounds":2}))
        .unwrap();
    let preview = h
        .tool(
            "voxel_text",
            &json!({"mode":"preview","text":"A","fontSize":8,"styleId":1}),
            None,
        )
        .await
        .unwrap();
    let first = preview["applyTemplate"].clone();
    let preview = h
        .tool(
            "voxel_text",
            &json!({"mode":"preview","text":"B","fontSize":8,"origin":[12,0,0],"styleId":1}),
            None,
        )
        .await
        .unwrap();
    let mut second = preview["applyTemplate"].clone();
    second["baseRevision"] = json!(1);
    *fake.state.replies.lock().unwrap() = vec![
        call("build-1","voxel_text",first),
        call("capture-1","capture_views",json!({"revision":1,"views":["hero"]})),
        call("capture-repeat","capture_views",json!({"revision":1,"views":["hero"]})),
        call("finish-early","finish_artwork",json!({"title":"作品","summary":"应被拒绝","goalAssessment":"检查了结构"})),
        call("build-2","voxel_text",second),
        call("capture-2","capture_views",json!({"revision":2,"views":["hero"]})),
        call("finish-final","finish_artwork",json!({"title":"作品","summary":"完成两轮检查与修改","goalAssessment":"主体已建立，并根据截图补充连接结构；无声称额外细节。"})),
    ].into();
    let capture = h.capture.clone();
    let capture_workspace = w.clone();
    let worker = tokio::spawn(async move {
        for _ in 0..3 {
            complete_pending_capture(capture.clone(), capture_workspace.clone()).await;
        }
    });
    h.start("搭建有细节的体素作品").unwrap();
    let run = terminal_run(&h).await;
    worker.await.unwrap();
    assert_eq!(run.status, "completed", "{}", run.error);
    assert_eq!(fake.request_count(), 7);
    let state = h.state();
    let goal = &state["context"]["goal"];
    assert_eq!(goal["status"], "achieved");
    assert_eq!(goal["reviewedGeometryHashes"].as_array().unwrap().len(), 2);
    assert!(w
        .lock()
        .unwrap()
        .events
        .iter()
        .any(|e| e.kind == "tool.result" && e.data.to_string().contains("GOAL_REVIEW_REQUIRED")));
    let history = std::fs::read_to_string(dir.path().join("runs/conversation.json")).unwrap();
    assert!(!history.contains("data:image/"));
    assert!(history.contains("artifact"));
    assert!(fake
        .requests()
        .iter()
        .any(|v| v.to_string().contains("data:image/png;base64,")));
}

#[tokio::test]
async fn chat_json_and_sse_providers_keep_tool_loop_and_usage() {
    for streaming in [false, true] {
        let first = json!({"id":"chat-1","choices":[{"index":0,"message":{"role":"assistant","tool_calls":[{"id":"ctx-1","type":"function","function":{"name":"get_context","arguments":"{}"}}]},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":10,"completion_tokens":4,"total_tokens":14}});
        let second = json!({"id":"chat-2","choices":[{"index":0,"message":{"role":"assistant","content":"中文完成"},"finish_reason":"stop"}],"usage":{"prompt_tokens":15,"completion_tokens":5,"total_tokens":20}});
        let replies = if streaming {
            vec![
                Reply::Sse(format!(
                    "data: {}\r\n\r\ndata: {}\n\ndata: [DONE]\r\n\r\n",
                    json!({"id":"chat-1","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"ctx-1","function":{"name":"get_context","arguments":"{}"}}]},"finish_reason":"tool_calls"}]}),
                    json!({"choices":[],"usage":first["usage"]})
                )),
                Reply::Sse(format!(
                    "data: {}\r\n\r\ndata: [DONE]\n\n",
                    json!({"id":"chat-2","choices":[{"index":0,"delta":{"content":"中文完成"},"finish_reason":"stop"}],"usage":second["usage"]})
                )),
            ]
        } else {
            vec![Reply::Http(200, first), Reply::Http(200, second)]
        };
        let fake = FakeResponses::start(replies).await;
        let dir = TempDir::new().unwrap();
        let h = harness(workspace(&dir), &fake.base);
        h.configure_provider(&json!({"apiType":"chat","stream":streaming,"sendReasoning":false}))
            .unwrap();
        h.configure(&json!({"model":"custom-model"})).unwrap();
        h.start("读取当前状态").unwrap();
        let run = terminal_run(&h).await;
        assert_eq!(run.status, "completed", "{}", run.error);
        assert_eq!(run.tokens, 34);
        let requests = fake.requests();
        assert_eq!(requests[0]["model"], "custom-model");
        assert!(requests[0].get("reasoning_effort").is_none());
        assert!(requests[1]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["role"] == "tool" && m["tool_call_id"] == "ctx-1"));
    }
}

#[tokio::test]
async fn chat_stream_without_done_never_executes_partial_tools() {
    let fake=FakeResponses::start(vec![Reply::Sse(format!("data: {}\n\n",json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"unsafe","function":{"name":"run_recipe","arguments":"{\"code\":\"mf.clear();\"}"}}]},"finish_reason":"tool_calls"}]})))]).await;
    let dir = TempDir::new().unwrap();
    let w = workspace(&dir);
    let h = harness(w.clone(), &fake.base);
    h.configure_provider(&json!({"apiType":"chat"})).unwrap();
    h.start("创建作品").unwrap();
    let run = terminal_run(&h).await;
    assert_eq!(run.status, "failed");
    assert!(!w
        .lock()
        .unwrap()
        .events
        .iter()
        .any(|e| e.kind == "tool.request"));
}

#[tokio::test]
async fn provider_connection_is_frozen_during_running_and_paused_runs() {
    let fake = FakeResponses::start(vec![Reply::Delay(
        Duration::from_millis(1000),
        Box::new(completed(vec![])),
    )])
    .await;
    let dir = TempDir::new().unwrap();
    let h = harness(workspace(&dir), &fake.base);
    h.start("等待检查").unwrap();
    assert!(h.configure_provider(&json!({"apiType":"chat"})).is_err());
    h.control("pause").unwrap();
    tokio::time::sleep(Duration::from_millis(25)).await;
    assert!(h.configure_provider(&json!({"apiType":"chat"})).is_err());
    h.control("stop").unwrap();
}

#[tokio::test]
async fn missing_usage_is_estimated_and_sensitive_provider_echoes_are_redacted() {
    let fake=FakeResponses::start(vec![sse([
        json!({"type":"response.output_text.delta","delta":"local-secret-fixture"}),
        json!({"type":"response.completed","response":{"id":"echo","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"local-secret-fixture"}]}]}}),
    ])]).await;
    let dir = TempDir::new().unwrap();
    let w = workspace(&dir);
    let h = harness(w.clone(), &fake.base);
    h.configure_provider(&json!({"apiKey":"local-secret-fixture"}))
        .unwrap();
    h.start("读取当前状态").unwrap();
    let run = terminal_run(&h).await;
    assert_eq!(run.status, "completed");
    assert!(
        run.tokens > 1000,
        "missing usage must not be counted as zero"
    );
    assert_eq!(run.message, "[REDACTED]");
    assert!(
        !std::fs::read_to_string(dir.path().join("events/events.jsonl"))
            .unwrap()
            .contains("local-secret-fixture")
    );
    assert!(w
        .lock()
        .unwrap()
        .events
        .iter()
        .any(|e| e.kind == "harness.usage.estimated"));
}

#[tokio::test]
async fn usage_totals_include_incomplete_and_compact_once_and_survive_restart() {
    let fake=FakeResponses::start(vec![
        sse([json!({"type":"response.incomplete","response":{"status":"incomplete","incomplete_details":{"reason":"max_output_tokens"},"usage":{"input_tokens":100,"output_tokens":20,"total_tokens":120,"input_tokens_details":{"cached_tokens":50}}}})]),
        sse([json!({"type":"response.completed","response":{"status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"完成"}]}],"usage":{"input_tokens":900,"output_tokens":90,"total_tokens":990,"input_tokens_details":{"cached_tokens":0}}}})]),
        sse([json!({"type":"response.completed","response":{"status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"保留原目标的摘要"}]}],"usage":{"input_tokens":50,"output_tokens":10,"total_tokens":60}}})]),
    ]).await;
    let dir = TempDir::new().unwrap();
    let h = harness(workspace(&dir), &fake.base);
    h.start("检查状态").unwrap();
    let run = terminal_run(&h).await;
    assert_eq!(run.status, "completed");
    let stats = h.state()["usage"].clone();
    assert_eq!(stats["total"]["totalTokens"], 1110);
    assert_eq!(stats["total"]["requests"], 2);
    assert_eq!(stats["run"]["totalTokens"], 1110);
    assert_eq!(stats["total"]["prefixHitRate"], 0.05);
    h.control("compact").unwrap();
    assert_eq!(terminal_run(&h).await.status, "completed");
    let stats = h.state()["usage"].clone();
    assert_eq!(stats["total"]["totalTokens"], 1170);
    assert_eq!(stats["run"]["totalTokens"], 60);
    let restored = Provider::new_for_loopback_test(dir.path(), &fake.base).unwrap();
    assert_eq!(restored.usage(None)["total"], stats["total"]);
}

#[tokio::test]
async fn third_party_files_setting_is_forced_inline_and_chat_cache_usage_is_preserved() {
    let fake=FakeResponses::start(vec![Reply::Http(200,json!({"id":"usage","choices":[{"message":{"content":"ok"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1000,"completion_tokens":100,"total_tokens":1100,"prompt_cache_hit_tokens":600,"prompt_cache_miss_tokens":400}}))]).await;
    let dir = TempDir::new().unwrap();
    let w = workspace(&dir);
    let artifact = seed_capture(&w);
    let provider = Provider::new_for_loopback_test(dir.path(), &fake.base).unwrap();
    let status = provider
        .configure(&json!({"apiType":"responses","imageDelivery":"files"}))
        .unwrap();
    assert_eq!(status["imageDelivery"], "inline");
    assert_eq!(status["supportsFileUpload"], false);
    provider
        .configure(&json!({"apiType":"chat","stream":false}))
        .unwrap();
    let response=provider.response(json!({"model":"custom","instructions":"test","max_output_tokens":1000,"input":[{"role":"user","content":[{"type":"input_image","artifact":artifact}]}]}),w,"cache-run",&Cancellation::new()).await.unwrap();
    assert_eq!(
        response["usage"]["input_tokens_details"]["cached_tokens"],
        600
    );
    assert!(fake.requests()[0]["messages"]
        .to_string()
        .contains("data:image/png;base64,"));
    assert_eq!(
        provider.usage(Some("cache-run"))["total"]["prefixHitRate"],
        0.6
    );
}

#[tokio::test]
async fn reference_images_are_explicit_inputs_survive_compact_and_can_be_cleared() {
    let reply = || {
        completed(vec![
            json!({"type":"message","role":"assistant","content":[{"type":"output_text","text":"阶段说明"}]} ),
        ])
    };
    let fake = FakeResponses::start(vec![reply(), reply(), reply()]).await;
    let dir = TempDir::new().unwrap();
    let w = workspace(&dir);
    let h = harness(w.clone(), &fake.base);
    let message = json!({"requestId":"reference-1","prompt":"按照图片搭建椅子","referenceIds":["builtin:chair"]});
    let ack = h.message(&message).unwrap();
    assert_eq!(terminal_run(&h).await.status, "completed");
    assert_eq!(h.message(&message).unwrap(), ack);
    assert_eq!(fake.request_count(), 1);
    assert!(h.message(&json!({"requestId":"reference-1","prompt":"按照图片搭建椅子","referenceIds":["builtin:pine"]})).is_err());
    assert!(fake.requests()[0]["input"]
        .to_string()
        .contains("data:image/png;base64,"));
    let reference = h.state()["context"]["referenceImages"].clone();
    assert_eq!(reference[0]["id"], "builtin:chair");
    let history = std::fs::read_to_string(dir.path().join("runs/conversation.json")).unwrap();
    assert!(!history.contains("data:image/"));
    h.control("compact").unwrap();
    assert_eq!(terminal_run(&h).await.status, "completed");
    assert_eq!(h.state()["context"]["referenceImages"], reference);
    h.message(&json!({"prompt":"取消参考，继续修改","referenceIds":[]}))
        .unwrap();
    assert_eq!(terminal_run(&h).await.status, "completed");
    assert!(h.state()["context"]["referenceImages"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(!fake.requests()[2]["input"]
        .to_string()
        .contains("data:image/"));
    h.configure(&json!({"sendImages":false})).unwrap();
    assert!(h
        .message(&json!({"prompt":"按图做","referenceIds":["builtin:chair"]}))
        .unwrap_err()
        .to_string()
        .contains("关闭"));
}

#[tokio::test]
async fn a_reference_image_cannot_replace_a_real_current_revision_capture() {
    let fake = FakeResponses::start(vec![
        completed(vec![function_call(
            "finish",
            "finish_artwork",
            json!({"title":"test","summary":"不应成功"}),
        )]),
        completed(vec![]),
    ])
    .await;
    let dir = TempDir::new().unwrap();
    let w = workspace(&dir);
    let h = harness(w.clone(), &fake.base);
    h.message(&json!({"prompt":"看图搭建","referenceIds":["builtin:chair"]}))
        .unwrap();
    assert_eq!(terminal_run(&h).await.status, "completed");
    assert!(w.lock().unwrap().library().unwrap().is_empty());
    assert!(
        w.lock()
            .unwrap()
            .events
            .iter()
            .any(|e| e.kind == "tool.result"
                && e.data.to_string().contains("FINISH_CAPTURE_REQUIRED"))
    );
}
