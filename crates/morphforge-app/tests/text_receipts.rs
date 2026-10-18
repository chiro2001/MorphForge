use morphforge_app::store::{Event, Workspace};
use morphforge_core::{Box3, Operation};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    fs,
    io::{BufWriter, Write},
    path::Path,
};

fn operations() -> Vec<Operation> {
    vec![Operation::Box {
        value: Box3::new([0, 0, 0], [2, 2, 1], 1),
    }]
}

fn retry_operations() -> Vec<Operation> {
    vec![Operation::Box {
        value: Box3::new([4, 0, 0], [1, 3, 1], 1),
    }]
}

fn font() -> Value {
    json!({
        "id":"builtin:noto-sans-sc",
        "family":"Noto Sans SC",
        "style":"Regular",
        "source":"builtin",
        "sha256":"faa6c9df652116dde789d351359f3d7e5d2285a2b2a1f04a2d7244df706d5ea9",
        "faceIndex":0,
        "license":"OFL-1.1"
    })
}

fn layout() -> Value {
    json!({
        "width":2,
        "height":2,
        "depth":1,
        "voxelCount":4,
        "boxCount":1,
        "bounds":{"min":[0,0,0],"max":[2,2,1]},
        "missingGlyphs":[]
    })
}

fn assert_receipt_shape(receipt: &Value) {
    let object = receipt.as_object().unwrap();
    let keys = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    assert_eq!(
        keys,
        BTreeSet::from([
            "font",
            "layout",
            "requestFingerprint",
            "requestId",
            "revision",
            "stats"
        ])
    );
    assert!(object.get("scene").is_none());
}

fn append_noise_events(workspace: &Workspace, count: u64) {
    let path = workspace.root.join("events/events.jsonl");
    let file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    let mut writer = BufWriter::new(file);
    for offset in 0..count {
        let event = Event {
            seq: workspace.next_seq + offset,
            time: "2026-10-19T00:00:00.000Z".into(),
            kind: "test.noise".into(),
            run_id: None,
            revision: workspace.project.scene.revision,
            data: json!({"offset":offset}),
        };
        serde_json::to_writer(&mut writer, &event).unwrap();
        writer.write_all(b"\n").unwrap();
    }
    writer.flush().unwrap();
}

#[test]
fn receipt_survives_event_log_failure_and_replays_after_restart() {
    let directory = tempfile::tempdir().unwrap();
    let mut workspace = Workspace::open(directory.path().to_path_buf()).unwrap();
    let log = directory.path().join("events/events.jsonl");
    if log.exists() {
        fs::remove_file(&log).unwrap();
    }
    fs::create_dir(&log).unwrap();

    let first = workspace.apply_text(
        &operations(),
        0,
        "durable-text",
        "fingerprint-a",
        &font(),
        &layout(),
        Some("run-a"),
    );
    assert!(first.is_err(), "forced event-log failure must surface");
    assert_eq!(workspace.project.scene.revision, 1);
    assert_eq!(
        workspace
            .text_receipt("durable-text", "fingerprint-a")
            .unwrap()
            .unwrap()["revision"],
        1
    );

    fs::remove_dir(&log).unwrap();
    drop(workspace);
    let mut restarted = Workspace::open(directory.path().to_path_buf()).unwrap();
    assert_eq!(restarted.project.scene.revision, 1);
    let receipt = restarted
        .apply_text(
            &operations(),
            0,
            "durable-text",
            "fingerprint-a",
            &font(),
            &layout(),
            None,
        )
        .unwrap();
    assert_receipt_shape(&receipt);
    assert_eq!(receipt["requestId"], "durable-text");
    assert_eq!(receipt["requestFingerprint"], "fingerprint-a");
    assert_eq!(restarted.project.scene.revision, 1);
}

#[test]
fn receipt_survives_more_than_the_event_window_and_rejects_id_reuse() {
    let directory = tempfile::tempdir().unwrap();
    let mut workspace = Workspace::open(directory.path().to_path_buf()).unwrap();
    let original = workspace
        .apply_text(
            &operations(),
            0,
            "windowed-text",
            "fingerprint-a",
            &font(),
            &layout(),
            None,
        )
        .unwrap();
    append_noise_events(&workspace, 6001);
    let expected_next = workspace.next_seq + 6001;
    drop(workspace);

    let mut reopened = Workspace::open(directory.path().to_path_buf()).unwrap();
    assert_eq!(reopened.events.len(), 5000);
    assert_eq!(reopened.next_seq, expected_next);
    assert!(!reopened
        .events
        .iter()
        .any(|event| event.kind == "text.applied"));
    let replay = reopened
        .text_receipt("windowed-text", "fingerprint-a")
        .unwrap()
        .unwrap();
    assert_eq!(replay, original);
    assert_receipt_shape(&replay);

    let unchanged = reopened
        .apply_text(
            &operations(),
            12345,
            "windowed-text",
            "fingerprint-a",
            &font(),
            &layout(),
            None,
        )
        .unwrap();
    assert_eq!(unchanged, original);
    let conflict = reopened.apply_text(
        &operations(),
        0,
        "windowed-text",
        "fingerprint-b",
        &font(),
        &layout(),
        None,
    );
    assert!(conflict
        .unwrap_err()
        .to_string()
        .starts_with("REQUEST_ID_CONFLICT"));
    assert_eq!(reopened.project.scene.revision, 1);
}

#[test]
fn receipt_lookup_does_not_depend_on_current_font_or_material_catalog() {
    let directory = tempfile::tempdir().unwrap();
    let mut workspace = Workspace::open(directory.path().to_path_buf()).unwrap();
    workspace
        .apply_text(
            &operations(),
            0,
            "catalog-independent",
            "fingerprint-a",
            &font(),
            &layout(),
            None,
        )
        .unwrap();
    workspace.project.materials.clear();
    workspace.project.limits.target_boxes = 0;
    let receipt = workspace
        .text_receipt("catalog-independent", "fingerprint-a")
        .unwrap()
        .unwrap();
    assert_eq!(receipt["requestId"], "catalog-independent");
    assert_receipt_shape(&receipt);
}

#[test]
fn failed_project_persistence_rolls_back_scene_and_receipt() {
    let directory = tempfile::tempdir().unwrap();
    let mut workspace = Workspace::open(directory.path().to_path_buf()).unwrap();
    let project_path = directory.path().join("project.json");
    let backup = directory.path().join("project.before-test.json");
    let old_material_snapshot = workspace.material_snapshot_path(0);
    let old_scene_snapshot = workspace.snapshot_path(0);
    let material_snapshot = workspace.material_snapshot_path(1);
    let scene_snapshot = workspace.snapshot_path(1);
    assert!(old_material_snapshot.exists());
    assert!(old_scene_snapshot.exists());
    assert!(!material_snapshot.exists());
    assert!(!scene_snapshot.exists());
    assert!(old_material_snapshot.exists());
    assert!(old_scene_snapshot.exists());
    fs::rename(&project_path, &backup).unwrap();
    fs::create_dir(&project_path).unwrap();

    let result = workspace.apply_text(
        &operations(),
        0,
        "persist-failure",
        "fingerprint-a",
        &font(),
        &layout(),
        None,
    );
    assert!(result.is_err());
    assert_eq!(workspace.project.scene.revision, 0);
    assert!(workspace.project.text_receipts.is_empty());
    assert!(workspace
        .text_receipt("persist-failure", "fingerprint-a")
        .unwrap()
        .is_none());
    assert!(!workspace.requests.contains_key("persist-failure"));
    assert!(!workspace.undo.contains(&0));
    assert!(!material_snapshot.exists());
    assert!(!scene_snapshot.exists());

    fs::remove_dir(&project_path).unwrap();
    fs::rename(&backup, &project_path).unwrap();
    assert!(Path::new(&project_path).is_file());
    let retried = workspace
        .apply_text(
            &retry_operations(),
            0,
            "persist-failure",
            "fingerprint-b",
            &font(),
            &layout(),
            None,
        )
        .unwrap();
    assert_eq!(retried["revision"], 1);
    assert_eq!(retried["requestFingerprint"], "fingerprint-b");
    assert!(material_snapshot.exists());
    assert!(scene_snapshot.exists());
    assert_eq!(workspace.project.scene.revision, 1);
}

#[test]
fn ordinary_apply_rejects_text_request_id_after_restart() {
    let directory = tempfile::tempdir().unwrap();
    let mut workspace = Workspace::open(directory.path().to_path_buf()).unwrap();
    workspace
        .apply_text(
            &operations(),
            0,
            "text-owned-id",
            "fingerprint-a",
            &font(),
            &layout(),
            None,
        )
        .unwrap();
    drop(workspace);

    let mut reopened = Workspace::open(directory.path().to_path_buf()).unwrap();
    let before = reopened.project.scene.clone();
    let error = reopened
        .apply(&retry_operations(), Some(1), Some("text-owned-id"), None)
        .unwrap_err();
    assert!(error.to_string().starts_with("REQUEST_ID_CONFLICT"));
    assert_eq!(reopened.project.scene.revision, 1);
    assert_eq!(reopened.project.scene.boxes.len(), before.boxes.len());
    assert!(reopened
        .text_receipt("text-owned-id", "fingerprint-a")
        .unwrap()
        .is_some());
}
