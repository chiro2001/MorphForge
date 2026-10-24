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

#[tokio::test]
async fn manual_publish_includes_selected_motion_and_rejects_invalid_selections_without_publishing()
{
    let root = tempfile::tempdir().unwrap();
    let app = App::open(root.path().into(), Some(root.path().join("no-key.env"))).unwrap();
    let (project_id, revision) = {
        let mut w = app.workspace.lock().unwrap();
        let mut value = Box3::new([-2, 24, -2], [4, 4, 4], 1);
        value.group = "rotor".into();
        w.apply(&[Operation::Box { value }], Some(0), None, None)
            .unwrap();
        w.project.creative.clips.insert(
            "spin".into(),
            serde_json::from_value(json!({
                "name":"测试旋转", "duration":4, "looping":true,
                "tracks":[{"group":"rotor", "pivot":[0,26,0], "interpolation":"linear", "keys":[
                    {"time":0,"translation":[0,0,0],"rotation":[0,0,0],"scale":[1,1,1]},
                    {"time":2,"translation":[0,0,0],"rotation":[0,180,0],"scale":[1,1,1]},
                    {"time":4,"translation":[0,0,0],"rotation":[0,360,0],"scale":[1,1,1]}
                ]}]
            }))
            .unwrap(),
        );
        app.transport
            .configure(
                &json!({"profile":"mf6","enabled":false,"port":19012}),
                &mut w,
            )
            .unwrap();
        (w.project.id.clone(), w.project.scene.revision)
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let router = app.router();
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = reqwest::Client::new();
    let body = json!({
        "projectId":project_id,"baseRevision":revision,"baseCreativeRevision":0,
        "animation":{"id":"spin","speed":1,"position":0}
    });
    let unsupported = client
        .post(format!("{base}/api/publish"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert!(unsupported.status().is_client_error());
    assert_eq!(app.transport.state()["motion"]["attached"], false);
    assert!(app.workspace.lock().unwrap().project.commits.is_empty());
    let mut catalog =
        AvatarCatalog::from_bytes(include_bytes!("../../../materials/mf6/catalog.json")).unwrap();
    catalog
        .capabilities
        .push(morphforge_app::avatar_motion::CAPABILITY.into());
    app.transport
        .set_catalog(Some(catalog), root.path())
        .unwrap();

    // Publish and finish the static baseline first. Identical geometry must still
    // be rebound when the user first selects an animation in the main UI.
    let static_response = client
        .post(format!("{base}/api/publish"))
        .json(&json!({"animation":null}))
        .send()
        .await
        .unwrap();
    assert!(static_response.status().is_success());
    {
        let mut state = app.transport.inner.lock().unwrap();
        while let Some(plan) = state.unified.next("volume").unwrap() {
            state.unified.commit(&plan);
        }
        state.sent_revision = Some(revision);
    }
    let response = client
        .post(format!("{base}/api/publish"))
        .json(&body)
        .send()
        .await
        .unwrap();
    let status = response.status();
    let published: Value = response.json().await.unwrap();
    assert!(status.is_success(), "{published}");
    assert_eq!(published["motion"]["clipId"], "spin");
    assert_eq!(published["motion"]["attached"], true);
    assert_eq!(published["motion"]["state"], 1);
    assert_eq!(published["motion"]["starting"], true);
    assert_eq!(published["enabled"], false);
    {
        let state = app.transport.inner.lock().unwrap();
        assert_eq!(state.unified.motion_groups["rotor"], 1);
        assert!(
            state.unified.next("volume").unwrap().is_some(),
            "static geometry needs new group tags"
        );
    }
    let commits = app.workspace.lock().unwrap().project.commits.len();
    for (field, value) in [
        ("projectId", json!("another-project")),
        ("baseRevision", json!(revision + 1)),
        ("baseCreativeRevision", json!(1)),
        ("animation", json!({"id":"missing","speed":1,"position":0})),
        ("animation", json!({"id":"spin","speed":0.3,"position":0})),
        ("animation", json!({"id":"spin","speed":1,"position":5})),
        ("animation", json!({"id":"spin"})),
        ("repair", json!(true)),
        ("resync", json!(true)),
    ] {
        let before = app.transport.state();
        let mut invalid = body.clone();
        invalid[field] = value;
        let response = client
            .post(format!("{base}/api/publish"))
            .json(&invalid)
            .send()
            .await
            .unwrap();
        assert!(
            response.status().is_client_error(),
            "invalid {field} must fail"
        );
        let after = app.transport.state();
        assert_eq!(
            after["motion"], before["motion"],
            "failed selection must preserve the active clip"
        );
        assert_eq!(after["pending"], before["pending"]);
        assert_eq!(app.workspace.lock().unwrap().project.commits.len(), commits);
    }
    // A later geometry-only publication preserves the attached animation.
    let response = client
        .post(format!("{base}/api/publish"))
        .json(&json!({"animation":null}))
        .send()
        .await
        .unwrap();
    let result: Value = response.json().await.unwrap();
    assert_eq!(result["motion"]["clipId"], "spin");
    assert_eq!(result["motion"]["state"], 1);
    app.transport.motion_control("stop", None).unwrap();
    let response = client
        .post(format!("{base}/api/publish"))
        .json(&json!({"projectId":project_id,"baseRevision":revision}))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    let result: Value = response.json().await.unwrap();
    assert_eq!(
        result["motion"]["clipId"], "spin",
        "the only saved clip is the default"
    );
    assert_eq!(
        result["motion"]["state"], 1,
        "default publication starts playback"
    );
    {
        let mut w = app.workspace.lock().unwrap();
        let clip = w.project.creative.clips["spin"].clone();
        w.project.creative.clips.insert("other".into(), clip);
    }
    app.transport.motion_control("stop", None).unwrap();
    let response = client
        .post(format!("{base}/api/publish"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    let result: Value = response.json().await.unwrap();
    assert_eq!(
        result["motion"]["state"], 0,
        "multiple saved clips require a selection"
    );
    app.transport.inner.lock().unwrap().stop = true;
    task.abort();
}
