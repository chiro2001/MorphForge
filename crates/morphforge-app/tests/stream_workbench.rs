use morphforge_app::{server::App, store::Workspace};
use morphforge_core::{Box3, Operation};
use serde_json::{json, Value};

#[test]
fn deleting_shared_artwork_is_recoverable_and_preserves_other_projects_and_assets() {
    let root = tempfile::tempdir().unwrap();
    let shared = root.path().join("library");
    let mut a = Workspace::open(root.path().join("a")).unwrap();
    let mut b = Workspace::open(root.path().join("b")).unwrap();
    a.use_shared_library(shared.clone()).unwrap();
    b.use_shared_library(shared.clone()).unwrap();
    a.apply(
        &[Operation::Box {
            value: Box3::new([0; 3], [3; 3], 1),
        }],
        Some(0),
        None,
        None,
    )
    .unwrap();
    let artwork = a.save_artwork("可删除", vec![], None, None).unwrap();
    b.load_artwork(&artwork.id).unwrap();
    let before = serde_json::to_value(&b.project).unwrap();
    assert_eq!(a.remove_artwork(&artwork.id).unwrap()["removed"], true);
    assert!(b.library().unwrap().is_empty());
    assert_eq!(serde_json::to_value(&b.project).unwrap(), before);
    assert_eq!(
        std::fs::read_dir(shared.join("artwork-trash"))
            .unwrap()
            .count(),
        1
    );
    assert_eq!(a.remove_artwork(&artwork.id).unwrap()["removed"], false);
    assert!(a.remove_artwork("builtin:windmill").is_err());
    assert!(a.remove_artwork("../../project").is_err());
}

#[tokio::test]
async fn creation_preferences_are_persistent_validated_and_dynamic_context_only() {
    let root = tempfile::tempdir().unwrap();
    let app = App::open(root.path().into(), Some(root.path().join("none.env"))).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let router = app.clone().router();
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = reqwest::Client::new();
    let r = client
        .post(format!("{base}/api/project"))
        .json(&json!({"modelingStyle":"lowpoly","modelingExtent":[40,80,24],"baseRevision":0}))
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success(), "{}", r.text().await.unwrap());
    let context = app
        .harness
        .tool("get_context", &json!({}), None)
        .await
        .unwrap();
    assert_eq!(context["modelingStyle"], "lowpoly");
    assert_eq!(context["modelingExtent"], json!([40, 80, 24]));
    let before = serde_json::to_value(&app.workspace.lock().unwrap().project).unwrap();
    let r = client
        .post(format!("{base}/api/project"))
        .json(&json!({"name":"must-not-change","modelingExtent":[0,3,4]}))
        .send()
        .await
        .unwrap();
    assert!(!r.status().is_success());
    assert_eq!(
        serde_json::to_value(&app.workspace.lock().unwrap().project).unwrap(),
        before
    );
    let saved: Value =
        serde_json::from_slice(&std::fs::read(root.path().join("project.json")).unwrap()).unwrap();
    assert_eq!(saved["modelingExtent"], json!([40, 80, 24]));
    let r = client
        .post(format!("{base}/api/project"))
        .json(&json!({"modelingStyle":"auto","modelingExtent":null}))
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success());
    assert!(app
        .workspace
        .lock()
        .unwrap()
        .project
        .modeling_extent
        .is_none());
    let artifact = app
        .workspace
        .lock()
        .unwrap()
        .save_artwork("HTTP删除", vec![], None, None)
        .unwrap();
    let r = client
        .delete(format!("{base}/api/artworks/{}", artifact.id))
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success());
    assert_eq!(r.json::<Value>().await.unwrap()["removed"], true);
    task.abort();
    app.transport.inner.lock().unwrap().stop = true;
}
