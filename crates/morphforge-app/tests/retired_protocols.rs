use morphforge_app::{store::Workspace, target::AvatarCatalog, transport::Transport};
use serde_json::json;
use std::sync::{Arc, Mutex};
#[test]
fn retired_osc_profiles_and_catalogs_are_rejected_without_mutation() {
    let root = tempfile::tempdir().unwrap();
    let w = Arc::new(Mutex::new(Workspace::open(root.path().into()).unwrap()));
    let t = Transport::new(w.clone());
    let before = serde_json::to_value(&w.lock().unwrap().project).unwrap();
    for profile in ["mf2", "mf3", "mf4", "mf5"] {
        assert!(t
            .configure(
                &json!({"profile":profile,"enabled":false}),
                &mut w.lock().unwrap()
            )
            .is_err());
    }
    for bytes in [
        include_bytes!("../../../materials/mf4/catalog.json").as_slice(),
        include_bytes!("../../../materials/mf5/catalog.json").as_slice(),
    ] {
        let c = AvatarCatalog::from_bytes(bytes).unwrap();
        assert!(t.set_catalog(Some(c), root.path()).is_err());
    }
    assert_eq!(t.config().profile, "mf6");
    assert_eq!(
        serde_json::to_value(&w.lock().unwrap().project).unwrap(),
        before
    );
    assert_eq!(t.state()["sentCommands"], 0);
    t.inner.lock().unwrap().stop = true;
}

#[test]
fn first_repair_of_revision_zero_does_not_skip_an_unpublished_scene() {
    let root = tempfile::tempdir().unwrap();
    let w = Arc::new(Mutex::new(Workspace::open(root.path().into()).unwrap()));
    let t = Transport::new(w);
    let c =
        AvatarCatalog::from_bytes(include_bytes!("../../../materials/mf6/catalog.json")).unwrap();
    t.set_catalog(Some(c), root.path()).unwrap();
    let scene = morphforge_core::Scene {
        revision: 0,
        boxes: vec![morphforge_core::Box3::new([0; 3], [2; 3], 1)],
        ..Default::default()
    };
    t.publish_for_repair(&scene, &morphforge_core::materials::builtins())
        .unwrap();
    let state = t.inner.lock().unwrap();
    assert!(state.unified.repairing());
    assert!(!state.unified.target_empty());
    assert!(state.unified.next("volume").unwrap().is_some());
    drop(state);
    t.inner.lock().unwrap().stop = true;
}
