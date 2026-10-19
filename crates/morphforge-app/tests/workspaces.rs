use morphforge_app::{server::App, store::write_json};
use morphforge_core::{Box3, Operation};
use serde_json::{json, Value};

async fn post(
    client: &reqwest::Client,
    base: &str,
    path: &str,
    value: Value,
) -> (reqwest::StatusCode, Value) {
    let response = client
        .post(format!("{base}/api/{path}"))
        .json(&value)
        .send()
        .await
        .unwrap();
    (response.status(), response.json().await.unwrap())
}
fn request(app: &App, action: &str) -> Value {
    json!({"action":action,"baseProjectId":app.workspace.lock().unwrap().project.id,
        "baseSessionId":app.harness.inner.lock().unwrap().conversation.id})
}
#[tokio::test]
async fn project_and_context_transitions_preserve_sources_history_and_cache_guards() {
    let dir = tempfile::tempdir().unwrap();
    write_json(
        &dir.path().join("cache-monitor.json"),
        &json!({"alert":"保留这条费用告警","mode":"strict"}),
    )
    .unwrap();
    let app = App::open(dir.path().into(), Some(dir.path().join("no-key.env"))).unwrap();
    let first_project = app.workspace.lock().unwrap().project.id.clone();
    let first_session = app.harness.inner.lock().unwrap().conversation.id.clone();
    app.workspace
        .lock()
        .unwrap()
        .apply(
            &[Operation::Box {
                value: Box3::new([-3, 0, -4], [2, 3, 2], 2),
            }],
            Some(0),
            None,
            None,
        )
        .unwrap();
    app.harness.inner.lock().unwrap().conversation.input =
        vec![json!({"role":"user","content":"保留此历史"})];
    app.harness.inner.lock().unwrap().conversation.goals = vec!["我的旧对话".into()];
    app.harness.tool("recipe_sources",&json!({"action":"write","path":"main.js","content":"mf.box([0,0,0],[1,1,1]);","baseRevision":1}),None).await.unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let router = app.router();
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = reqwest::Client::new();
    let (status, new) = post(&client, &base, "workspaces", request(&app, "new_session")).await;
    assert!(status.is_success(), "{new}");
    assert_eq!(app.workspace.lock().unwrap().project.id, first_project);
    assert_eq!(app.workspace.lock().unwrap().project.scene.boxes.len(), 1);
    assert!(app
        .harness
        .inner
        .lock()
        .unwrap()
        .conversation
        .input
        .is_empty());
    assert_ne!(new["sessionId"], first_session);
    assert_eq!(app.harness.provider.cache_status()["status"], "warming");
    assert_eq!(
        app.harness
            .tool("recipe_sources", &json!({"action":"list"}), None)
            .await
            .unwrap()["files"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let second_session = new["sessionId"].clone();
    let mut open = request(&app, "open_session");
    open["id"] = json!(first_session);
    let (status, value) = post(&client, &base, "workspaces", open).await;
    assert!(status.is_success(), "{value}");
    assert_eq!(
        app.harness.inner.lock().unwrap().conversation.input[0]["content"],
        "保留此历史"
    );
    assert_eq!(
        app.harness.provider.cache_status()["alert"],
        "保留这条费用告警"
    );
    let mut create = request(&app, "new_project");
    create["name"] = json!("第二件作品");
    let (status, second) = post(&client, &base, "workspaces", create).await;
    assert!(status.is_success(), "{second}");
    assert_ne!(second["projectId"], first_project);
    assert_eq!(app.workspace.lock().unwrap().project.scene.revision, 0);
    assert!(app.workspace.lock().unwrap().project.scene.boxes.is_empty());
    assert!(app.workspace.lock().unwrap().undo.is_empty());
    assert!(!app.transport.config().enabled && !app.transport.config().auto);
    assert!(app
        .harness
        .tool("recipe_sources", &json!({"action":"list"}), None)
        .await
        .unwrap()["files"]
        .as_array()
        .unwrap()
        .is_empty());
    let mut illegal = request(&app, "open_session");
    illegal["id"] = second_session;
    let (status, _) = post(&client, &base, "workspaces", illegal).await;
    assert_eq!(status, reqwest::StatusCode::BAD_REQUEST);
    let mut open = request(&app, "open_project");
    open["id"] = json!(first_project);
    let (status, value) = post(&client, &base, "workspaces", open).await;
    assert!(status.is_success(), "{value}");
    assert_eq!(value["sessionId"], first_session);
    assert_eq!(
        app.workspace.lock().unwrap().project.scene.boxes[0].min,
        [-3, 0, -4]
    );
    assert_eq!(app.workspace.lock().unwrap().undo, vec![0]);
    assert_eq!(
        app.harness.provider.cache_status()["alert"],
        "保留这条费用告警"
    );
    let (_, value) = post(
        &client,
        &base,
        "workspaces",
        json!({"action":"new_project","baseProjectId":"stale","baseSessionId":first_session}),
    )
    .await;
    assert!(value["error"]
        .as_str()
        .unwrap()
        .starts_with("WORKSPACE_CONFLICT"));
    let timeline: Value = client
        .get(format!("{base}/api/timeline"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(timeline["events"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["projectId"] == first_project));
    let catalog: Value = client
        .get(format!("{base}/api/workspaces"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(catalog["projects"].as_array().unwrap().len(), 2);
    assert_eq!(catalog["sessions"].as_array().unwrap().len(), 2);
    server.abort();
    app.transport.inner.lock().unwrap().stop = true;
    drop(app);
    let reopened = App::open(dir.path().into(), Some(dir.path().join("no-key.env"))).unwrap();
    assert_eq!(reopened.workspace.lock().unwrap().project.id, first_project);
    assert_eq!(
        reopened.harness.inner.lock().unwrap().conversation.id,
        first_session
    );
    assert_eq!(
        reopened.harness.inner.lock().unwrap().conversation.input[0]["content"],
        "保留此历史"
    );
    reopened.transport.inner.lock().unwrap().stop = true;
}
#[tokio::test]
async fn active_and_queued_work_cannot_cross_a_context_boundary() {
    let dir = tempfile::tempdir().unwrap();
    let app = App::open(dir.path().into(), Some(dir.path().join("no-key.env"))).unwrap();
    let original = app.workspace.lock().unwrap().project.id.clone();
    let value = request(&app, "new_project");
    for phase in ["running", "paused", "stopping", "compacting"] {
        app.harness.inner.lock().unwrap().phase = phase.into();
        assert!(app
            .harness
            .workspace_action(&value)
            .unwrap_err()
            .to_string()
            .starts_with("RUN_CONFLICT"));
        assert_eq!(app.workspace.lock().unwrap().project.id, original);
    }
    app.harness.inner.lock().unwrap().phase = "idle".into();
    app.harness.inner.lock().unwrap().worker_epoch = Some(1);
    assert!(app.harness.workspace_action(&value).is_err());
    app.transport.inner.lock().unwrap().stop = true;
}
#[tokio::test]
async fn legacy_context_is_migrated_and_corrupt_context_is_never_silently_erased() {
    let dir = tempfile::tempdir().unwrap();
    write_json(&dir.path().join("runs/conversation.json"),&json!({"input":[{"role":"user","content":"旧版本历史"}],"goals":[],"summary":"摘要","boundaries":[],"compactCount":1})).unwrap();
    let app = App::open(dir.path().into(), Some(dir.path().join("no-key.env"))).unwrap();
    let id = app.harness.inner.lock().unwrap().conversation.id.clone();
    assert!(!id.is_empty());
    assert_eq!(
        app.harness.state()["context"]["schemaUpgradeRequired"],
        true
    );
    assert_eq!(
        app.harness.inner.lock().unwrap().conversation.summary,
        "摘要"
    );
    app.transport.inner.lock().unwrap().stop = true;
    drop(app);
    std::fs::write(dir.path().join("runs/conversation.json"), b"{broken").unwrap();
    assert!(App::open(dir.path().into(), Some(dir.path().join("no-key.env"))).is_err());
    assert_eq!(
        std::fs::read(dir.path().join("runs/conversation.json")).unwrap(),
        b"{broken"
    );
}

#[tokio::test]
async fn interrupted_transition_recovers_one_consistent_project_context_and_cache_state() {
    let dir = tempfile::tempdir().unwrap();
    let app = App::open(dir.path().into(), Some(dir.path().join("no-key.env"))).unwrap();
    let old_project = app.workspace.lock().unwrap().project.id.clone();
    app.transport.inner.lock().unwrap().stop = true;
    drop(app);
    let project = morphforge_app::store::Project {
        name: "恢复的工程".into(),
        ..Default::default()
    };
    let session_id = uuid::Uuid::new_v4().to_string();
    let conversation = morphforge_app::harness::Conversation {
        id: session_id.clone(),
        project_id: project.id.clone(),
        created_at: "2026-10-19T00:00:00Z".into(),
        input: vec![json!({"role":"user","content":"恢复此上下文"})],
        ..Default::default()
    };
    write_json(&dir.path().join("workspace-transition.json"), &json!({"project":project,"session":{
        "conversation":conversation,"run":null,"providerState":{"prefix":{},"guard":{"mode":"strict","alert":"恢复费用告警"}}
    }})).unwrap();
    let reopened = App::open(dir.path().into(), Some(dir.path().join("no-key.env"))).unwrap();
    assert_eq!(reopened.workspace.lock().unwrap().project.id, project.id);
    assert_eq!(
        reopened.harness.inner.lock().unwrap().conversation.id,
        session_id
    );
    assert_eq!(
        reopened.harness.inner.lock().unwrap().conversation.input[0]["content"],
        "恢复此上下文"
    );
    assert_eq!(
        reopened.harness.provider.cache_status()["alert"],
        "恢复费用告警"
    );
    assert!(!dir.path().join("workspace-transition.json").exists());
    assert!(reopened
        .workspace
        .lock()
        .unwrap()
        .project_archive_path(&old_project)
        .exists());
    assert!(!reopened.transport.config().enabled);
    reopened.transport.inner.lock().unwrap().stop = true;
}
