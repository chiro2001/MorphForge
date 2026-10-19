use morphforge_app::{
    server::App,
    store::write_json,
    target::{AvatarCatalog, SURFACE_CAPABILITY},
};
use morphforge_core::{Box3, Operation};
use serde_json::{json, Value};
fn op(min: [i32; 3], size: [u32; 3], style: u32, group: &str) -> Operation {
    let mut b = Box3::new(min, size, style);
    b.group = group.into();
    Operation::Box { value: b }
}
#[tokio::test]
async fn transform_and_theme_preserve_sources_protection_and_undo() {
    let dir = tempfile::tempdir().unwrap();
    let app = App::open(dir.path().into(), Some(dir.path().join("none.env"))).unwrap();
    app.workspace
        .lock()
        .unwrap()
        .apply(
            &[op([-13, -8, 5], [7, 9, 3], 1, "body")],
            Some(0),
            None,
            None,
        )
        .unwrap();
    app.harness.tool("recipe_sources",&json!({"action":"write","path":"main.js","content":"mf.box([-13,-8,5],[7,9,3],{style:1});","baseRevision":1}),None).await.unwrap();
    let sources = app
        .harness
        .tool("recipe_sources", &json!({"action":"list"}), None)
        .await
        .unwrap();
    let dry = app
        .harness
        .tool(
            "transform_scene",
            &json!({"action":"center_on_ground","baseRevision":1,"dryRun":true}),
            None,
        )
        .await
        .unwrap();
    assert_eq!(dry["offset"], json!([10, 8, -6]));
    assert_eq!(app.workspace.lock().unwrap().project.scene.revision, 1);
    let moved = app
        .harness
        .tool(
            "transform_scene",
            &json!({"action":"center_on_ground","baseRevision":1}),
            None,
        )
        .await
        .unwrap();
    assert_eq!(moved["revision"], 2);
    assert!(moved["after"]["fits"].as_bool().unwrap());
    assert_eq!(
        sources,
        app.harness
            .tool("recipe_sources", &json!({"action":"list"}), None)
            .await
            .unwrap()
    );
    app.workspace.lock().unwrap().undo(false, None).unwrap();
    assert_eq!(
        app.workspace.lock().unwrap().project.scene.boxes[0].min,
        [-13, -8, 5]
    );
    let revision = app.workspace.lock().unwrap().project.scene.revision;
    let original =
        serde_json::to_value(&app.workspace.lock().unwrap().project.scene.boxes).unwrap();
    let applied = app
        .harness
        .tool(
            "builtin_materials",
            &json!({"action":"apply","id":"pixel-workshop","baseRevision":revision}),
            None,
        )
        .await
        .unwrap();
    assert_eq!(applied["materialsAdded"], 19);
    assert_eq!(
        original,
        serde_json::to_value(&app.workspace.lock().unwrap().project.scene.boxes).unwrap()
    );
    let revision = app.workspace.lock().unwrap().project.scene.revision;
    let again = app
        .harness
        .tool(
            "builtin_materials",
            &json!({"action":"apply","id":"pixel-workshop","baseRevision":revision}),
            None,
        )
        .await
        .unwrap();
    assert_eq!(again["unchanged"], true);
    {
        let mut w = app.workspace.lock().unwrap();
        w.set_protected_groups(vec!["body".into()]).unwrap();
    }
    let revision = app.workspace.lock().unwrap().project.scene.revision;
    let error = app
        .harness
        .tool(
            "transform_scene",
            &json!({"action":"center_on_ground","baseRevision":revision,"dryRun":true}),
            None,
        )
        .await
        .unwrap_err();
    assert!(error.to_string().starts_with("PROTECTED_GROUP_CONFLICT"));
    assert_eq!(
        app.workspace.lock().unwrap().project.scene.revision,
        revision
    );
    app.transport.inner.lock().unwrap().stop = true;
}
#[tokio::test]
async fn target_catalog_is_explicit_and_surface_projection_does_not_mutate_art() {
    let dir = tempfile::tempdir().unwrap();
    let app = App::open(dir.path().into(), Some(dir.path().join("none.env"))).unwrap();
    {
        let mut w = app.workspace.lock().unwrap();
        w.apply(
            &[
                op([0, 0, 0], [1, 2, 2], 16, "a"),
                op([1, 0, 0], [1, 2, 2], 16, "b"),
            ],
            Some(0),
            None,
            None,
        )
        .unwrap();
        w.project.scene.boxes[0].motion_phase = 17;
    }
    let w = app.workspace.lock().unwrap();
    let source = w.project.scene.clone();
    let plain = AvatarCatalog::from_bytes(
        &serde_json::to_vec(
            &json!({"format":"MorphForgeMaterialCatalog/1","materials":w.project.materials}),
        )
        .unwrap(),
    )
    .unwrap();
    let legacy =
        morphforge_app::target::project(&source, &w.project.materials, Some(&plain)).unwrap();
    assert_eq!(legacy.boxes[0].motion_phase, 17);
    let masked=AvatarCatalog::from_bytes(&serde_json::to_vec(&json!({"format":"MorphForgeMaterialCatalog/1","materials":w.project.materials,"capabilities":[SURFACE_CAPABILITY]})).unwrap()).unwrap();
    let projected =
        morphforge_app::target::project(&source, &w.project.materials, Some(&masked)).unwrap();
    assert_eq!(projected.boxes[0].motion_phase, 1);
    assert_eq!(projected.boxes[1].motion_phase, 2);
    assert_eq!(source.boxes[0].motion_phase, 17);
    let mut bad = serde_json::to_value(&masked).unwrap();
    let m = bad["materials"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|m| m["id"] == 16)
        .unwrap();
    m["opacity"] = json!(0.12);
    let bad = AvatarCatalog::from_bytes(&serde_json::to_vec(&bad).unwrap()).unwrap();
    assert!(morphforge_app::target::project(&source, &w.project.materials, Some(&bad)).is_err());
    assert_eq!(
        morphforge_app::target::compatibility(&source, &w.project.materials, None)["status"],
        "unknown"
    );
    drop(w);
    app.transport.inner.lock().unwrap().stop = true;
}
#[tokio::test]
async fn corrupted_target_catalog_stays_blocking_and_never_becomes_unknown() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("avatar-catalog.json"), "{bad").unwrap();
    let app = App::open(dir.path().into(), Some(dir.path().join("none.env"))).unwrap();
    let mut w = app.workspace.lock().unwrap();
    assert_eq!(
        app.transport
            .material_report(&w.project.scene, &w.project.materials)["status"],
        "invalid"
    );
    assert!(app
        .transport
        .configure(&json!({"enabled":true,"port":1}), &mut w)
        .is_err());
    assert!(app
        .transport
        .publish_with_materials(&w.project.scene, &w.project.materials, false)
        .is_err());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("avatar-catalog.json")).unwrap(),
        "{bad"
    );
    app.transport.inner.lock().unwrap().stop = true;
}
#[tokio::test]
async fn automatic_finish_center_requires_a_new_capture_before_completion() {
    let dir = tempfile::tempdir().unwrap();
    let app = App::open(dir.path().into(), Some(dir.path().join("none.env"))).unwrap();
    {
        let mut w = app.workspace.lock().unwrap();
        w.apply(
            &[op([30, -2, 5], [4, 3, 6], 1, "body")],
            Some(0),
            None,
            None,
        )
        .unwrap();
        w.project.auto_center_on_finish = true;
        w.persist().unwrap();
    }
    let error = app
        .harness
        .tool(
            "finish_artwork",
            &json!({"title":"test","summary":"test"}),
            None,
        )
        .await
        .unwrap_err();
    assert!(error.to_string().starts_with("CENTERED_NEEDS_REVIEW"));
    let revision = app.workspace.lock().unwrap().project.scene.revision;
    assert_eq!(revision, 2);
    let error = app
        .harness
        .tool(
            "finish_artwork",
            &json!({"title":"test","summary":"test"}),
            None,
        )
        .await
        .unwrap_err();
    assert!(error.to_string().starts_with("FINISH_CAPTURE_REQUIRED"));
    assert_eq!(app.workspace.lock().unwrap().project.scene.revision, 2);
    app.transport.inner.lock().unwrap().stop = true;
}
#[test]
fn display_preferences_are_typed_and_persisted_separately() {
    let dir = tempfile::tempdir().unwrap();
    let defaults = morphforge_app::view_settings::load(dir.path()).unwrap();
    assert_eq!(defaults["voxelGrid"], false);
    let changed = morphforge_app::view_settings::patch(
        &defaults,
        &json!({"quality":"standard","showTargetBounds":true}),
    )
    .unwrap();
    write_json(&dir.path().join("view-settings.json"), &changed).unwrap();
    assert_eq!(
        morphforge_app::view_settings::load(dir.path()).unwrap(),
        changed
    );
    assert!(morphforge_app::view_settings::patch(&defaults, &json!({"quality":"fake"})).is_err());
    assert!(morphforge_app::view_settings::patch(&defaults, &json!({"voxelGrid":"true"})).is_err());
    let _: Value = defaults;
}

#[tokio::test]
async fn old_context_requires_explicit_schema_upgrade_before_any_model_request() {
    use morphforge_app::{harness::Conversation, store::Workspace};
    let dir = tempfile::tempdir().unwrap();
    {
        let mut w = Workspace::open(dir.path().into()).unwrap();
        w.event(
            "harness.started",
            None,
            json!({"toolsVersion":"morphforge-tools/3.5","promptVersion":"morphforge-system/4.8"}),
        )
        .unwrap();
    }
    let input = vec![json!({"role":"user","content":"保留旧上下文"})];
    let conversation = Conversation {
        input: input.clone(),
        ..Default::default()
    };
    write_json(&dir.path().join("runs/conversation.json"), &conversation).unwrap();
    let app = App::open(dir.path().into(), Some(dir.path().join("none.env"))).unwrap();
    assert_eq!(
        app.harness.state()["context"]["schemaUpgradeRequired"],
        true
    );
    assert!(app
        .harness
        .message(&json!({"prompt":"继续"}))
        .unwrap_err()
        .to_string()
        .starts_with("HARNESS_UPGRADE_REQUIRED"));
    app.harness.control("upgrade_context").unwrap();
    assert_eq!(
        app.harness.state()["context"]["schemaUpgradeRequired"],
        false
    );
    assert_eq!(app.harness.inner.lock().unwrap().conversation.input, input);
    let prefix: Value =
        serde_json::from_slice(&std::fs::read(dir.path().join("provider-prefix.json")).unwrap())
            .unwrap();
    assert_eq!(prefix["authorized_reset"], "explicit_schema_upgrade");
    assert!(!app
        .workspace
        .lock()
        .unwrap()
        .events
        .iter()
        .any(|e| e.kind == "provider.request"));
    app.transport.inner.lock().unwrap().stop = true;
}

#[tokio::test]
async fn invalid_capture_quality_is_rejected_before_geometry_commits() {
    let dir = tempfile::tempdir().unwrap();
    let app = App::open(dir.path().into(), Some(dir.path().join("none.env"))).unwrap();
    for capture in [
        json!({"views":["hero"],"quality":"bad"}),
        json!({"views":["hero"],"supersampling":1.5}),
        json!({"views":["hero"],"mode":"unknown"}),
    ] {
        let result=app.harness.tool("run_recipe",&json!({"code":"mf.box([0,0,0],[3,3,3]);","mode":"apply","baseRevision":0,"seed":1,"capture":capture}),None).await;
        assert!(result.is_err());
        assert_eq!(app.workspace.lock().unwrap().project.scene.revision, 0);
    }
    app.transport.inner.lock().unwrap().stop = true;
}
