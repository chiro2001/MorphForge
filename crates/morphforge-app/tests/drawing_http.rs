//! Real transaction, persistence and export checks for demonstration geometry.
use axum::http::StatusCode;
use morphforge_app::{drawing, server::App, store::Workspace};
use reqwest::Client;
use serde_json::{json, Value};
use std::path::Path;
use tokio::{net::TcpListener, task::JoinHandle};

struct Server {
    app: App,
    base: String,
    task: JoinHandle<()>,
    client: Client,
}
impl Server {
    async fn new(root: &Path) -> Self {
        let app = App::open(root.into(), None).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let router = app.router();
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self {
            app,
            base,
            task,
            client: Client::new(),
        }
    }
    async fn post(&self, path: &str, value: Value) -> (StatusCode, Value) {
        let r = self
            .client
            .post(format!("{}{path}", self.base))
            .json(&value)
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
fn request(base: u64) -> Value {
    json!({"action":"apply","baseRevision":base,"requestId":"drawing-one","group":"plot","primitives":[{"kind":"point","position":[0,24,0],"size":8,"color":[255,80,40]},{"kind":"segment","start":[0,24,0],"end":[32,40,0],"width":4}]})
}

#[tokio::test]
async fn preview_apply_retry_restart_and_undo_share_the_actual_mesh() {
    let root = tempfile::tempdir().unwrap();
    let server = Server::new(root.path()).await;
    let mut preview = request(0);
    preview["action"] = json!("preview");
    let (status, p) = server.post("/api/draw-primitives", preview).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(p["dryRun"], true);
    assert!(p["triangles"].as_array().unwrap().len() > 20);
    assert_eq!(
        server.app.workspace.lock().unwrap().project.scene.revision,
        0
    );
    let (status, result) = server.post("/api/draw-primitives", request(0)).await;
    assert_eq!(status, StatusCode::OK);
    let before = server
        .app
        .workspace
        .lock()
        .unwrap()
        .project
        .scene
        .triangles
        .clone();
    assert_eq!(before.len(), p["triangles"].as_array().unwrap().len());
    for (saved, raw) in before.iter().zip(p["triangles"].as_array().unwrap()) {
        assert_eq!(json!(saved.vertices), raw["vertices"]);
    }
    let (_, retry) = server.post("/api/draw-primitives", request(0)).await;
    assert_eq!(retry["replayed"], true);
    assert_eq!(retry["revision"], result["revision"]);
    assert_eq!(
        server.app.workspace.lock().unwrap().project.scene.triangles,
        before
    );
    let mut reopened = Workspace::open(root.path().into()).unwrap();
    assert_eq!(
        drawing::execute(&mut reopened, &request(0), None).unwrap()["replayed"],
        true
    );
    assert_eq!(reopened.project.scene.triangles, before);
    let mut changed = request(0);
    changed["primitives"][0]["size"] = json!(16);
    assert_eq!(
        server.post("/api/draw-primitives", changed).await.0,
        StatusCode::CONFLICT
    );
    assert_eq!(server.post("/api/undo", json!({})).await.0, StatusCode::OK);
    assert!(server
        .app
        .workspace
        .lock()
        .unwrap()
        .project
        .scene
        .triangles
        .is_empty());
}

#[tokio::test]
async fn replacement_preserves_other_work_and_respects_protection_and_versions() {
    let root = tempfile::tempdir().unwrap();
    let server = Server::new(root.path()).await;
    server.post("/api/edit",json!({"baseRevision":0,"operations":[{"op":"box","min":[50,0,0],"size":[4,4,4],"styleId":1,"group":"house"}]})).await;
    assert_eq!(
        server.post("/api/draw-primitives", request(1)).await.0,
        StatusCode::OK
    );
    let mut replacement = request(2);
    replacement["mode"] = json!("replace_group");
    replacement["requestId"] = json!("drawing-two");
    replacement["primitives"] = json!([{"kind":"axes","position":[0,8,0],"length":32,"width":2}]);
    assert_eq!(
        server.post("/api/draw-primitives", replacement).await.0,
        StatusCode::OK
    );
    assert_eq!(
        server
            .app
            .workspace
            .lock()
            .unwrap()
            .project
            .scene
            .boxes
            .len(),
        1
    );
    server
        .post(
            "/api/project",
            json!({"baseRevision":3,"protectedGroups":["plot"]}),
        )
        .await;
    let mut overwrite = request(4);
    overwrite["requestId"] = json!("protected");
    overwrite["mode"] = json!("replace_group");
    assert_eq!(
        server.post("/api/draw-primitives", overwrite).await.0,
        StatusCode::CONFLICT
    );
    let mut stale = request(1);
    stale["requestId"] = json!("stale");
    assert_eq!(
        server.post("/api/draw-primitives", stale).await.0,
        StatusCode::CONFLICT
    );
    let mut foreign = request(4);
    foreign["projectId"] = json!("another-project");
    assert_eq!(
        server.post("/api/draw-primitives", foreign).await.0,
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn manual_apply_is_locked_during_ai_but_preview_stays_available() {
    let root = tempfile::tempdir().unwrap();
    let server = Server::new(root.path()).await;
    server.app.harness.inner.lock().unwrap().phase = "running".into();
    assert_eq!(
        server.post("/api/draw-primitives", request(0)).await.0,
        StatusCode::CONFLICT
    );
    let mut p = request(0);
    p["action"] = json!("preview");
    assert_eq!(
        server.post("/api/draw-primitives", p).await.0,
        StatusCode::OK
    );
    assert_eq!(
        server.app.workspace.lock().unwrap().project.scene.revision,
        0
    );
    server.app.harness.inner.lock().unwrap().phase = "idle".into();
}

#[tokio::test]
async fn demonstration_surfaces_export_and_report_avatar_bounds_without_starting_osc() {
    let root = tempfile::tempdir().unwrap();
    let server = Server::new(root.path()).await;
    let (_, result) = server.post("/api/draw-primitives", request(0)).await;
    assert_eq!(result["target"]["fits"], true);
    for face in &server.app.workspace.lock().unwrap().project.scene.triangles {
        face.wire_vertices().unwrap();
    }
    let r = server
        .client
        .get(format!("{}/api/export/model?format=glb", server.base))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    assert!(r.bytes().await.unwrap().starts_with(b"glTF"));
    let mut outside = request(1);
    outside["requestId"] = json!("outside");
    outside["primitives"] = json!([{"kind":"point","position":[1000,24,0],"size":8}]);
    let (status, r) = server.post("/api/draw-primitives", outside).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(r["target"]["fits"], false);
    assert_eq!(server.app.transport.config().enabled, false);
}
