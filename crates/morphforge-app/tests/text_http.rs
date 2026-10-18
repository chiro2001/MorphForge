//! Exercise the real HTTP/transaction boundary, including retries and edits
//! arriving while a font preview is being prepared. No provider or OSC calls.
use axum::http::StatusCode;
use morphforge_app::server::App;
use reqwest::Client;
use serde_json::{json, Value};
use std::path::Path;
use tokio::{net::TcpListener, task::JoinHandle};

struct Server {
    base: String,
    client: Client,
    app: App,
    task: JoinHandle<()>,
}

impl Server {
    async fn start(root: &Path) -> Self {
        let app = App::open(root.into(), None).unwrap();
        let router = app.router();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self {
            base,
            client: Client::new(),
            app,
            task,
        }
    }

    async fn post(&self, path: &str, body: Value) -> (StatusCode, Value) {
        let response = self
            .client
            .post(format!("{}{path}", self.base))
            .json(&body)
            .send()
            .await
            .unwrap();
        (response.status(), response.json().await.unwrap())
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.app.transport.inner.lock().unwrap().stop = true;
        self.task.abort();
    }
}

fn text_request(id: &str, base: u64) -> Value {
    json!({"text":"云港 Forge", "fontId":"builtin:noto-sans-sc",
        "fontSize":16, "depth":2, "origin":[-50,8,0], "plane":"xy",
        "styleId":1, "group":"sign", "requestId":id, "baseRevision":base})
}

#[tokio::test]
async fn project_protection_rejects_overwrites_and_stale_unlocks_over_http() {
    let root = tempfile::tempdir().unwrap();
    let server = Server::start(root.path()).await;
    let original = json!({"op":"box","min":[0,0,0],"size":[4,4,4],"styleId":1,"group":"building"});
    assert_eq!(
        server
            .post(
                "/api/edit",
                json!({"operations":[original],"baseRevision":0,"requestId":"old-house"})
            )
            .await
            .0,
        StatusCode::OK
    );
    let original_boxes = server
        .app
        .workspace
        .lock()
        .unwrap()
        .project
        .scene
        .boxes
        .clone();
    let (status, locked) = server
        .post(
            "/api/project",
            json!({"protectedGroups":["building"],"baseRevision":1}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(locked["revision"], 2);
    assert_eq!(locked["project"]["protectedGroups"], json!(["building"]));
    let overwrite = json!({"operations":[{"op":"box","min":[1,1,1],"size":[2,2,2],"styleId":2,"group":"sign"}],"baseRevision":2,"requestId":"overlap"});
    let (status, rejected) = server.post("/api/edit", overwrite.clone()).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(rejected["error"]
        .as_str()
        .unwrap()
        .starts_with("PROTECTED_GROUP_CONFLICT"));
    assert_eq!(
        serde_json::to_value(&server.app.workspace.lock().unwrap().project.scene.boxes).unwrap(),
        serde_json::to_value(original_boxes).unwrap()
    );
    assert_eq!(
        server
            .post(
                "/api/project",
                json!({"protectedGroups":[],"baseRevision":1})
            )
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(server.post("/api/edit",json!({"operations":[{"op":"box","min":[20,0,0],"size":[2,2,2],"styleId":2,"group":"sign"}],"baseRevision":2,"requestId":"outside"})).await.0, StatusCode::OK);
    assert_eq!(
        server
            .post(
                "/api/project",
                json!({"protectedGroups":[],"baseRevision":3})
            )
            .await
            .0,
        StatusCode::OK
    );
    let (status, same) = server
        .post(
            "/api/project",
            json!({"protectedGroups":[],"baseRevision":4}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        same["revision"], 4,
        "setting the same protection policy is idempotent"
    );
    let mut allowed = overwrite;
    allowed["baseRevision"] = json!(4);
    let (status, result) = server.post("/api/edit", allowed).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["revision"], 5);
}

#[tokio::test]
async fn preview_apply_undo_and_durable_idempotent_receipt() {
    let root = tempfile::tempdir().unwrap();
    let server = Server::start(root.path()).await;
    let request = text_request("text-1", 0);
    let (status, preview) = server.post("/api/text/preview", request.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(preview["requestId"], "text-1");
    assert_eq!(preview["revision"], 0);
    assert!(!preview["operations"].as_array().unwrap().is_empty());
    assert_eq!(
        server.app.workspace.lock().unwrap().project.scene.revision,
        0
    );
    let mut apply = request.clone();
    apply["previewFingerprint"] = preview["requestFingerprint"].clone();
    let (status, applied) = server.post("/api/text/apply", apply.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(applied["revision"], 1);
    assert_eq!(applied["font"]["sha256"], preview["font"]["sha256"]);
    let scene = applied["scene"].clone();
    assert!(scene["boxes"]
        .as_array()
        .unwrap()
        .iter()
        .all(|b| b["group"] == "sign"));
    let (status, undone) = server.post("/api/undo", json!({"baseRevision":1})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(undone["revision"], 2);
    assert!(server
        .app
        .workspace
        .lock()
        .unwrap()
        .project
        .scene
        .boxes
        .is_empty());
    let (status, receipt) = server.post("/api/text/apply", apply.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        receipt["scene"], scene,
        "retry must return the original snapshot"
    );
    assert_eq!(
        server.app.workspace.lock().unwrap().project.scene.revision,
        2
    );
    drop(server);
    let restarted = Server::start(root.path()).await;
    let (status, receipt) = restarted.post("/api/text/apply", apply.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(receipt["scene"], scene);
    apply["text"] = json!("different");
    apply.as_object_mut().unwrap().remove("previewFingerprint");
    let (status, conflict) = restarted.post("/api/text/apply", apply).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(conflict["error"]
        .as_str()
        .unwrap()
        .starts_with("REQUEST_ID_CONFLICT"));
    assert_eq!(
        restarted
            .app
            .workspace
            .lock()
            .unwrap()
            .project
            .scene
            .revision,
        2
    );
    {
        let mut workspace = restarted.app.workspace.lock().unwrap();
        let mut materials = workspace.project.materials.clone();
        materials.retain(|material| material.id != 1);
        workspace.update_materials(materials).unwrap();
    }
    let (status, receipt) = restarted.post("/api/text/apply", request).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a receipt does not depend on today's material catalog"
    );
    assert_eq!(receipt["scene"], scene);
    assert_eq!(
        restarted
            .app
            .workspace
            .lock()
            .unwrap()
            .project
            .scene
            .revision,
        3
    );
}

#[tokio::test]
async fn stale_or_changed_previews_and_concurrent_edits_cannot_overwrite_drafts() {
    let root = tempfile::tempdir().unwrap();
    let server = Server::start(root.path()).await;
    let mut mismatch = text_request("mismatch", 0);
    mismatch["previewFingerprint"] = json!("old-preview");
    assert_eq!(
        server.post("/api/text/apply", mismatch).await.0,
        StatusCode::CONFLICT
    );
    let (left, right) = tokio::join!(
        server.post("/api/text/apply", text_request("left", 0)),
        server.post("/api/text/apply", text_request("right", 0))
    );
    let statuses = [left.0, right.0];
    assert_eq!(statuses.iter().filter(|s| **s == StatusCode::OK).count(), 1);
    assert_eq!(
        statuses
            .iter()
            .filter(|s| **s == StatusCode::CONFLICT)
            .count(),
        1
    );
    assert_eq!(
        server.app.workspace.lock().unwrap().project.scene.revision,
        1
    );
    let (status, stale) = server
        .post("/api/text/apply", text_request("stale", 0))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(stale["error"]
        .as_str()
        .unwrap()
        .starts_with("REVISION_CONFLICT"));
}

#[tokio::test]
async fn validation_exposes_missing_glyphs_without_geometry_and_rejects_id_reuse() {
    let root = tempfile::tempdir().unwrap();
    let server = Server::start(root.path()).await;
    let mut missing = text_request("missing", 0);
    missing["text"] = json!("\u{10ffff}");
    let (status, error) = server.post("/api/text/preview", missing).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["code"], "FONT_MISSING_GLYPHS");
    assert_eq!(error["missingGlyphs"], json!(["\u{10ffff}"]));
    let mut invalid = text_request("invalid", 0);
    invalid["fontSize"] = json!(100000);
    assert_eq!(
        server.post("/api/text/preview", invalid).await.0,
        StatusCode::BAD_REQUEST
    );
    let mut typo = text_request("typo", 0);
    typo["fontSiz"] = json!(24);
    assert_eq!(
        server.post("/api/text/preview", typo).await.0,
        StatusCode::BAD_REQUEST
    );
    {
        let mut workspace = server.app.workspace.lock().unwrap();
        workspace
            .apply(&[], Some(0), Some("already-edit"), None)
            .unwrap();
    }
    let (status, error) = server
        .post("/api/text/apply", text_request("already-edit", 1))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(error["error"]
        .as_str()
        .unwrap()
        .starts_with("REQUEST_ID_CONFLICT"));
    assert!(!server.app.transport.config().enabled);
}
