use morphforge_app::server::App;
use reqwest::StatusCode;
use serde_json::{json, Value};

#[tokio::test]
async fn builtin_gallery_is_offline_forkable_and_preserves_licensed_sources() {
    let dir = tempfile::tempdir().unwrap();
    let app = App::open(dir.path().into(), Some(dir.path().join("no-provider.env"))).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let router = app.router();
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = reqwest::Client::new();
    let library: Value = client
        .get(format!("{base}/api/artworks"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(library["artworks"].as_array().unwrap().is_empty());
    assert_eq!(library["examples"].as_array().unwrap().len(), 7);
    assert_eq!(app.workspace.lock().unwrap().project.scene.revision, 0);
    for artwork in library["examples"].as_array().unwrap() {
        let image = client
            .get(format!(
                "{base}/artifacts/{}",
                artwork["thumbnail"].as_str().unwrap()
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(image.status(), StatusCode::OK);
        assert_eq!(image.headers()["content-type"], "image/webp");
    }
    let original: Value = client
        .get(format!("{base}/api/artworks/builtin:whale-maid"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        original["project"]["scene"]["boxes"]
            .as_array()
            .unwrap()
            .len(),
        223
    );
    assert_eq!(
        original["project"]["sourceCredits"][0]["license"],
        "CC BY-NC-SA 4.0"
    );
    let fork = client
        .post(format!("{base}/api/artworks"))
        .json(&json!({"action":"fork","id":"builtin:whale-maid","baseRevision":0}))
        .send()
        .await
        .unwrap();
    assert_eq!(fork.status(), StatusCode::OK);
    assert_eq!(app.workspace.lock().unwrap().project.scene.boxes.len(), 223);
    let saved: Value = client
        .post(format!("{base}/api/artworks"))
        .json(&json!({"action":"save","title":"我的副本"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = saved["artwork"]["id"].as_str().unwrap();
    assert!(!id.starts_with("builtin:"));
    let copy: Value = client
        .get(format!("{base}/api/artworks/{id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        copy["project"]["sourceCredits"],
        original["project"]["sourceCredits"]
    );
    assert_eq!(
        client
            .post(format!("{base}/api/artworks"))
            .json(&json!({"action":"fork","id":"builtin:copper-lighthouse","baseRevision":0}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    let group = app.workspace.lock().unwrap().project.scene.boxes[0]
        .group
        .clone();
    let protected = client
        .post(format!("{base}/api/project"))
        .json(&json!({"baseRevision":1,"protectedGroups":[group]}))
        .send()
        .await
        .unwrap();
    assert_eq!(protected.status(), StatusCode::OK);
    let revision = app.workspace.lock().unwrap().project.scene.revision;
    assert_eq!(
        client
            .post(format!("{base}/api/artworks"))
            .json(
                &json!({"action":"fork","id":"builtin:copper-lighthouse","baseRevision":revision})
            )
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(app.workspace.lock().unwrap().project.scene.boxes.len(), 223);
    assert_eq!(
        client
            .get(format!("{base}/api/artworks/builtin:unknown"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert!(app
        .workspace
        .lock()
        .unwrap()
        .events
        .iter()
        .all(|e| e.kind != "provider.request"));
    app.transport.inner.lock().unwrap().stop = true;
    server.abort();
}
