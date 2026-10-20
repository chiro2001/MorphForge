//! Exercise the actual manual publish endpoint with sending disabled.
use morphforge_app::{server::App, target::AvatarCatalog};
use morphforge_core::{mf6::Frame, Box3, Operation};
use serde_json::{json, Value};

#[tokio::test]
async fn mf6_manual_repair_replays_sent_baseline_without_a_clear_or_remote_ack() {
    let root = tempfile::tempdir().unwrap();
    let app = App::open(root.path().into(), Some(root.path().join("no-key.env"))).unwrap();
    app.transport
        .set_catalog(
            Some(
                AvatarCatalog::from_bytes(include_bytes!("../../../materials/mf6/catalog.json"))
                    .unwrap(),
            ),
            root.path(),
        )
        .unwrap();
    {
        let mut w = app.workspace.lock().unwrap();
        w.apply(
            &[Operation::Box {
                value: Box3::new([-2, 0, -2], [4, 2, 4], 1),
            }],
            Some(0),
            None,
            None,
        )
        .unwrap();
        app.transport
            .configure(&json!({"profile":"mf6","enabled":false}), &mut w)
            .unwrap();
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let router = app.router();
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = reqwest::Client::new();
    let first = client
        .post(format!("{base}/api/publish"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert!(first.status().is_success());
    {
        let mut state = app.transport.inner.lock().unwrap();
        while let Some(plan) = state.unified.next("volume").unwrap() {
            state.unified.commit(&plan);
        }
        state.sent_revision = Some(1);
    }
    let response = client
        .post(format!("{base}/api/publish"))
        .json(&json!({"repair":true}))
        .send()
        .await
        .unwrap();
    let status = response.status();
    let result: Value = response.json().await.unwrap();
    // Stop the worker even when a regression assertion below fails.
    app.transport.inner.lock().unwrap().stop = true;
    assert!(
        status.is_success(),
        "MF6 repair must be allowed by HTTP: {result}"
    );
    assert_eq!(result["repairing"], true);
    assert_eq!(result["sentObjects"]["boxes"], 1);
    assert_eq!(result["baselineKind"], "local_udp_sent");
    assert_eq!(result["remoteAcknowledged"], false);
    assert_eq!(result["enabled"], false);
    {
        let state = app.transport.inner.lock().unwrap();
        assert_eq!(state.resets, 0);
        let plan = state.unified.next("volume").unwrap().unwrap();
        assert!(!matches!(plan.frame, Frame::Clear));
    }
    let bad = client
        .post(format!("{base}/api/publish"))
        .json(&json!({"repair":true,"resync":true}))
        .send()
        .await
        .unwrap();
    assert!(bad.status().is_client_error());
    assert_eq!(app.transport.inner.lock().unwrap().resets, 0);
    task.abort();
}
