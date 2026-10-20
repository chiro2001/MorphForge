//! Offline, immutable examples. Loading always creates a new workspace revision.
use crate::store::{Artwork, Workspace};
use anyhow::{Context, Result};
use include_dir::{include_dir, Dir};
use serde_json::Value;

static EXAMPLES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../examples");

fn manifest() -> Result<Value> {
    Ok(serde_json::from_slice(
        EXAMPLES
            .get_file("manifest.json")
            .context("内置示例清单缺失")?
            .contents(),
    )?)
}
fn entry(id: &str) -> Result<Value> {
    manifest()?["examples"]
        .as_array()
        .context("内置示例清单无效")?
        .iter()
        .find(|entry| entry["id"].as_str() == Some(id))
        .cloned()
        .context("未知内置示例ID")
}
fn metadata(workspace: &mut Workspace, entry: &Value) -> Result<Artwork> {
    let mut artwork: Artwork = serde_json::from_value(entry["artwork"].clone())?;
    let mut views = Vec::new();
    let mut thumbnail = None;
    for image in entry["images"].as_array().context("内置示例缺少截图")? {
        let file = EXAMPLES
            .get_file(image["file"].as_str().context("截图路径无效")?)
            .context("内置示例截图缺失")?;
        let artifact = workspace.artifact(file.contents(), "webp")?;
        if image["view"] == "hero" {
            thumbnail = Some(artifact.clone());
        }
        views.push(artifact);
    }
    artwork.thumbnail = thumbnail.or_else(|| views.first().cloned());
    artwork.views = views;
    Ok(artwork)
}
pub fn list(workspace: &mut Workspace) -> Result<Vec<Artwork>> {
    manifest()?["examples"]
        .as_array()
        .context("内置示例清单无效")?
        .iter()
        .map(|entry| metadata(workspace, entry))
        .collect()
}
pub fn get(workspace: &mut Workspace, id: &str) -> Result<Value> {
    let entry = entry(id)?;
    let file = EXAMPLES
        .get_file(entry["projectFile"].as_str().context("示例路径无效")?)
        .context("内置示例工程缺失")?;
    let mut value: Value = serde_json::from_slice(file.contents())?;
    value["artwork"] = serde_json::to_value(metadata(workspace, &entry)?)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Project;
    #[test]
    fn all_embedded_examples_are_complete_and_do_not_seed_or_replace_user_projects() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = Workspace::open(dir.path().to_path_buf()).unwrap();
        let original = serde_json::to_value(&w.project).unwrap();
        let entries = list(&mut w).unwrap();
        assert_eq!(entries.len(), 7);
        assert!(w.library().unwrap().is_empty());
        assert_eq!(serde_json::to_value(&w.project).unwrap(), original);
        for artwork in entries {
            let value = get(&mut w, &artwork.id).unwrap();
            let project: Project = serde_json::from_value(value["project"].clone()).unwrap();
            project.scene.validate(&project.limits).unwrap();
            project.creative.validate(&project.limits).unwrap();
            assert_eq!(project.scene.boxes.len(), artwork.box_count);
            assert_eq!(project.scene.triangles.len(), artwork.triangle_count);
            assert!(!project.source_credits.is_empty());
            assert_eq!(artwork.views.len(), 3);
            for view in &artwork.views {
                let bytes = std::fs::read(w.root.join("artifacts").join(view)).unwrap();
                image::load_from_memory(&bytes).unwrap();
            }
        }
        assert!(get(&mut w, "builtin:../../project").is_err());
    }
    #[test]
    fn loading_a_builtin_is_editable_undoable_and_preserves_source_credits() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = Workspace::open(dir.path().to_path_buf()).unwrap();
        w.load_artwork("builtin:whale-maid").unwrap();
        assert_eq!(w.project.scene.boxes.len(), 223);
        assert_eq!(w.project.source_credits[0]["license"], "CC BY-NC-SA 4.0");
        let saved = w.save_artwork("副本", vec![], None, None).unwrap();
        assert_ne!(saved.id, "builtin:whale-maid");
        w.undo(false, Some(0)).unwrap();
        assert!(w.project.scene.boxes.is_empty());
        assert!(w.project.source_credits.is_empty());
        assert_eq!(
            get(&mut w, "builtin:whale-maid").unwrap()["project"]["scene"]["boxes"]
                .as_array()
                .unwrap()
                .len(),
            223
        );
        w.load_artwork(&saved.id).unwrap();
        assert_eq!(w.project.source_credits[0]["license"], "CC BY-NC-SA 4.0");
    }
    #[test]
    fn protected_content_and_existing_snapshots_survive_rejected_example_loads() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = Workspace::open(dir.path().to_path_buf()).unwrap();
        w.load_artwork("builtin:wooden-chair").unwrap();
        let group = w.project.scene.boxes[0].group.clone();
        w.set_protected_groups(vec![group]).unwrap();
        let before = serde_json::to_value(&w.project).unwrap();
        assert!(w
            .load_artwork("builtin:whale-maid")
            .unwrap_err()
            .to_string()
            .contains("PROTECTED_GROUP_CONFLICT"));
        assert_eq!(serde_json::to_value(&w.project).unwrap(), before);
        w.set_protected_groups(Vec::new()).unwrap();
        let revision = w.project.scene.revision + 1;
        let path = w.snapshot_path(revision);
        let bytes = b"immutable future snapshot";
        std::fs::write(&path, bytes).unwrap();
        let before = serde_json::to_value(&w.project).unwrap();
        assert!(w.load_artwork("builtin:copper-lighthouse").is_err());
        assert_eq!(std::fs::read(path).unwrap(), bytes);
        assert_eq!(serde_json::to_value(&w.project).unwrap(), before);
    }
}
