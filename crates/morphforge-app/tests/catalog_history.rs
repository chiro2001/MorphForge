use morphforge_app::store::Workspace;
#[test]
fn historical_materials_are_immutable_and_undo_restores_the_catalog() {
    let folder = tempfile::tempdir().unwrap();
    let mut w = Workspace::open(folder.path().into()).unwrap();
    let first = w.project.materials.clone();
    let mut changed = first.clone();
    changed[0].color = [0.1, 0.2, 0.3];
    w.update_materials(changed.clone()).unwrap();
    assert_eq!(w.project.scene.revision, 1);
    assert_eq!(w.materials_at(0).unwrap()[0].color, first[0].color);
    assert_eq!(w.materials_at(1).unwrap()[0].color, changed[0].color);
    w.undo(false, Some(0)).unwrap();
    assert_eq!(w.project.materials[0].color, first[0].color);
    assert_eq!(w.project.scene.revision, 2);
    w.project.materials[0].color = [1.; 3];
    assert!(w.persist().is_err());
    assert_eq!(w.materials_at(2).unwrap()[0].color, first[0].color);
}
#[test]
fn diagnostics_omit_credentials_and_binary_browser_data() {
    use std::{fs, io::Read};
    let folder = tempfile::tempdir().unwrap();
    let mut w = Workspace::open(folder.path().into()).unwrap();
    w.event(
        "test.event",
        None,
        serde_json::json!({"test":"fake-secret-123"}),
    )
    .unwrap();
    fs::write(folder.path().join("provider.env"), "key=must-never-export").unwrap();
    fs::create_dir(folder.path().join("capture-browser")).unwrap();
    fs::write(folder.path().join("capture-browser/Cookies"), "private").unwrap();
    let bytes = morphforge_app::diagnostics::bundle(folder.path(), |s| {
        s.replace("fake-secret-123", "[REDACTED]")
    })
    .unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    assert!(archive.by_name("provider.env").is_err());
    assert!(archive.by_name("capture-browser/Cookies").is_err());
    let mut text = String::new();
    archive
        .by_name("events/events.jsonl")
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    assert!(!text.contains("fake-secret-123"));
    assert!(text.contains("[REDACTED]"));
}
