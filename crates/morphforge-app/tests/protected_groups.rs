use morphforge_app::store::Workspace;
use morphforge_core::{Box3, Operation, Scene};
use serde_json::json;
use std::fs;

fn block(group: &str, min: [i32; 3], style_id: u32) -> Operation {
    let mut value = Box3::new(min, [2, 2, 1], style_id);
    value.group = group.into();
    Operation::Box { value }
}

fn scene_boxes(workspace: &Workspace) -> serde_json::Value {
    serde_json::to_value(&workspace.project.scene.boxes).unwrap()
}

fn apply_one(workspace: &mut Workspace, operation: Operation, base: u64) -> Result<(), String> {
    workspace
        .apply(&[operation], Some(base), None, None)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[test]
fn protected_group_blocks_all_geometry_transactions_and_text_receipts() {
    let directory = tempfile::tempdir().unwrap();
    let mut workspace = Workspace::open(directory.path().to_path_buf()).unwrap();
    apply_one(&mut workspace, block("plaque", [0, 0, 0], 1), 0).unwrap();
    workspace
        .set_protected_groups(vec!["plaque".into()])
        .unwrap();
    assert_eq!(workspace.project.scene.revision, 2);
    let original_boxes = scene_boxes(&workspace);
    let original_undo = workspace.undo.clone();

    for operation in [
        block("plaque", [0, 0, 0], 2),
        Operation::Erase {
            min: [0, 0, 0],
            size: [1, 1, 1],
        },
        Operation::Paint {
            min: [0, 0, 0],
            size: [1, 1, 1],
            style_id: 2,
            tint_id: 1,
        },
        Operation::Clear,
        Operation::RemoveGroup {
            name: "plaque".into(),
        },
    ] {
        let error = apply_one(&mut workspace, operation, 2).unwrap_err();
        assert!(error.starts_with("PROTECTED_GROUP_CONFLICT"));
        assert!(error.contains("plaque"));
        assert_eq!(workspace.project.scene.revision, 2);
        assert_eq!(scene_boxes(&workspace), original_boxes);
        assert_eq!(workspace.undo, original_undo);
    }

    let replacement = Scene {
        revision: 2,
        ..Scene::default()
    };
    let error = workspace
        .replace(replacement, Some(2), None)
        .unwrap_err()
        .to_string();
    assert!(error.starts_with("PROTECTED_GROUP_CONFLICT"));
    assert_eq!(workspace.project.scene.revision, 2);
    assert_eq!(scene_boxes(&workspace), original_boxes);
    assert_eq!(workspace.undo, original_undo);

    let error = workspace
        .apply_text(
            &[block("plaque", [4, 0, 0], 1)],
            2,
            "blocked-text",
            "fingerprint-a",
            &json!({"id":"builtin:noto-sans-sc"}),
            &json!({"boxCount":1}),
            None,
        )
        .unwrap_err()
        .to_string();
    assert!(error.starts_with("PROTECTED_GROUP_CONFLICT"));
    assert!(workspace
        .text_receipt("blocked-text", "fingerprint-a")
        .unwrap()
        .is_none());
    assert!(!workspace.requests.contains_key("blocked-text"));
    assert_eq!(workspace.undo, original_undo);
    assert_eq!(workspace.project.scene.revision, 2);

    apply_one(&mut workspace, block("decoration", [8, 0, 0], 1), 2).unwrap();
    assert_eq!(workspace.project.scene.revision, 3);
    assert_eq!(
        workspace
            .project
            .scene
            .boxes
            .iter()
            .filter(|item| item.group == "plaque")
            .count(),
        1
    );
    workspace.set_protected_groups(vec![]).unwrap();
    assert_eq!(workspace.project.scene.revision, 4);
    apply_one(
        &mut workspace,
        Operation::Erase {
            min: [0, 0, 0],
            size: [1, 1, 1],
        },
        4,
    )
    .unwrap();
    let plaque_voxels: u64 = workspace
        .project
        .scene
        .boxes
        .iter()
        .filter(|item| item.group == "plaque")
        .map(Box3::volume)
        .sum();
    assert!(plaque_voxels < 4);
}

#[test]
fn future_empty_groups_are_strict_and_unprotected_groups_remain_editable() {
    let directory = tempfile::tempdir().unwrap();
    let mut workspace = Workspace::open(directory.path().to_path_buf()).unwrap();
    workspace
        .set_protected_groups(vec!["future".into()])
        .unwrap();
    assert_eq!(workspace.project.scene.revision, 1);
    let error = apply_one(&mut workspace, block("future", [0, 0, 0], 1), 1).unwrap_err();
    assert!(error.starts_with("PROTECTED_GROUP_CONFLICT"));
    assert!(error.contains("future"));
    assert!(workspace.project.scene.boxes.is_empty());

    apply_one(&mut workspace, block("decoration", [4, 0, 0], 1), 1).unwrap();
    assert_eq!(workspace.project.scene.revision, 2);
    apply_one(&mut workspace, block("decoration", [8, 0, 0], 1), 2).unwrap();
    assert_eq!(workspace.project.scene.revision, 3);
    assert_eq!(workspace.project.scene.boxes.len(), 2);
}

#[test]
fn history_and_artwork_loads_cannot_remove_protected_geometry() {
    let directory = tempfile::tempdir().unwrap();
    let mut workspace = Workspace::open(directory.path().to_path_buf()).unwrap();
    workspace.project.name = "saved-empty".into();
    let empty_artwork = workspace
        .save_artwork("before protection", vec![], None, None)
        .unwrap();
    apply_one(&mut workspace, block("plaque", [0, 0, 0], 1), 0).unwrap();
    workspace
        .set_protected_groups(vec!["plaque".into()])
        .unwrap();
    assert_eq!(workspace.project.scene.revision, 2);
    workspace.project.name = "current".into();
    let original_boxes = scene_boxes(&workspace);
    let original_undo = workspace.undo.clone();

    let error = workspace.undo(false, Some(0)).unwrap_err().to_string();
    assert!(error.starts_with("PROTECTED_GROUP_CONFLICT"));
    assert_eq!(workspace.project.scene.revision, 2);
    assert_eq!(workspace.undo, original_undo);
    assert_eq!(scene_boxes(&workspace), original_boxes);

    let error = workspace
        .load_artwork(&empty_artwork.id)
        .unwrap_err()
        .to_string();
    assert!(error.starts_with("PROTECTED_GROUP_CONFLICT"));
    assert_eq!(workspace.project.scene.revision, 2);
    assert_eq!(workspace.project.name, "current");
    assert_eq!(workspace.undo, original_undo);
    assert_eq!(scene_boxes(&workspace), original_boxes);

    let protected_artwork = workspace
        .save_artwork("with protection", vec![], None, None)
        .unwrap();
    workspace.set_protected_groups(vec![]).unwrap();
    assert_eq!(workspace.project.scene.revision, 3);
    workspace.load_artwork(&protected_artwork.id).unwrap();
    assert!(workspace.project.protected_groups.is_empty());
    assert_eq!(workspace.project.scene.revision, 4);
}

#[test]
fn protection_revision_is_idempotent_persistent_and_does_not_replay_on_undo() {
    let directory = tempfile::tempdir().unwrap();
    let mut workspace = Workspace::open(directory.path().to_path_buf()).unwrap();
    workspace
        .set_protected_groups(vec!["b".into(), "a".into(), "a".into()])
        .unwrap();
    assert_eq!(workspace.project.scene.revision, 1);
    assert_eq!(
        workspace.state()["project"]["protectedGroups"],
        json!(["a", "b"])
    );
    assert!(workspace.snapshot_path(1).exists());
    assert!(workspace.material_snapshot_path(1).exists());

    workspace
        .set_protected_groups(vec!["a".into(), "b".into()])
        .unwrap();
    assert_eq!(workspace.project.scene.revision, 1);
    workspace.undo(false, Some(1)).unwrap();
    assert_eq!(workspace.project.scene.revision, 2);
    assert_eq!(
        workspace
            .project
            .protected_groups
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["a", "b"]
    );
    drop(workspace);

    let reopened = Workspace::open(directory.path().to_path_buf()).unwrap();
    assert_eq!(reopened.project.scene.revision, 2);
    assert_eq!(
        reopened
            .project
            .protected_groups
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["a", "b"]
    );
    assert_eq!(
        reopened.state()["project"]["protectedGroups"],
        json!(["a", "b"])
    );
}

#[test]
fn protection_persistence_failure_rolls_back_revision_and_new_snapshots() {
    let directory = tempfile::tempdir().unwrap();
    let mut workspace = Workspace::open(directory.path().to_path_buf()).unwrap();
    let project_path = directory.path().join("project.json");
    let backup = directory.path().join("project.before-test.json");
    let material_snapshot = workspace.material_snapshot_path(1);
    let scene_snapshot = workspace.snapshot_path(1);
    fs::rename(&project_path, &backup).unwrap();
    fs::create_dir(&project_path).unwrap();

    assert!(workspace
        .set_protected_groups(vec!["future".into()])
        .is_err());
    assert_eq!(workspace.project.scene.revision, 0);
    assert!(workspace.project.protected_groups.is_empty());
    assert!(!workspace.revisions.contains_key(&1));
    assert!(!material_snapshot.exists());
    assert!(!scene_snapshot.exists());

    fs::remove_dir(&project_path).unwrap();
    fs::rename(&backup, &project_path).unwrap();
    workspace
        .set_protected_groups(vec!["future".into()])
        .unwrap();
    assert_eq!(workspace.project.scene.revision, 1);
    assert_eq!(
        workspace
            .project
            .protected_groups
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["future"]
    );
    assert!(material_snapshot.exists());
    assert!(scene_snapshot.exists());
}
