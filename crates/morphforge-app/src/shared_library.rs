//! Shared immutable artworks; active projects, conversation histories and provider
//! ledgers remain owned by one instance. Assets are copied before publication.
use crate::store::{validate_id, Workspace};
use anyhow::{bail, Result};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};
fn references(v: &Value, names: &mut BTreeSet<String>) {
    match v {
        Value::String(s) => {
            let name = s.strip_prefix("/artifacts/").unwrap_or(s);
            if name.len() > 8 && name.len() < 160 && name.contains('.') && validate_id(name).is_ok()
            {
                names.insert(name.into());
            }
        }
        Value::Array(a) => {
            for v in a {
                references(v, names)
            }
        }
        Value::Object(o) => {
            for v in o.values() {
                references(v, names)
            }
        }
        _ => {}
    }
}
fn transfer(value: &Value, source: &Path, destination: &Path) -> Result<()> {
    if source == destination {
        return Ok(());
    }
    fs::create_dir_all(destination.join("artifacts"))?;
    let mut names = BTreeSet::new();
    references(value, &mut names);
    if names.len() > 16384 {
        bail!("作品资源引用超过限制")
    }
    let mut total = 0u64;
    for name in names {
        let src = source.join("artifacts").join(&name);
        if !src.is_file() {
            continue;
        }
        total = total.saturating_add(fs::metadata(&src)?.len());
        if total > 256 * 1024 * 1024 {
            bail!("作品资源超过256MiB")
        }
        let dst = destination.join("artifacts").join(&name);
        if dst.is_file() {
            if fs::metadata(&src)?.len() != fs::metadata(&dst)?.len()
                || crate::store::hash(&fs::read(&src)?) != crate::store::hash(&fs::read(&dst)?)
            {
                bail!("共享资源同名内容冲突：{name}")
            }
            continue;
        }
        // Content files become visible atomically; no partial PNG on another
        // window, including a simultaneous save of the same content hash.
        let data = fs::read(&src)?;
        if let Some(expected) = name
            .split('.')
            .next()
            .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            if crate::store::hash(&data) != expected {
                bail!("共享资源内容哈希不符：{name}")
            }
        }
        let temp = dst.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        fs::write(&temp, &data)?;
        let published = fs::hard_link(&temp, &dst);
        let _ = fs::remove_file(&temp);
        match published {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if fs::read(&dst)? != data {
                    bail!("共享资源并发同名内容冲突：{name}")
                }
            }
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
impl Workspace {
    pub fn use_shared_library(&mut self, root: PathBuf) -> Result<()> {
        fs::create_dir_all(root.join("artworks"))?;
        fs::create_dir_all(root.join("artifacts"))?;
        self.library_root = root;
        Ok(())
    }
    pub fn artwork_path(&self, id: &str) -> Result<PathBuf> {
        validate_id(id)?;
        Ok(self
            .library_root
            .join("artworks")
            .join(format!("{id}.json")))
    }
    /// Remove only the shared catalog entry. Keep a recoverable archive and all
    /// content-addressed resources; other projects may still reference them.
    pub fn remove_artwork(&mut self, id: &str) -> Result<Value> {
        if id.starts_with("builtin:") {
            bail!("内置示例只读，请先保存到我的作品");
        }
        let source = self.artwork_path(id)?;
        let trash = self.library_root.join("artwork-trash");
        fs::create_dir_all(&trash)?;
        let destination = trash.join(format!("{}-{}.json", id, uuid::Uuid::new_v4()));
        let removed = match fs::rename(&source, &destination) {
            Ok(()) => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => return Err(e.into()),
        };
        if removed {
            self.event("artwork.removed", None, serde_json::json!({"id":id}))?;
        }
        Ok(
            serde_json::json!({"ok":true,"removed":removed,"id":id,"currentProjectChanged":false,"resourcesRetained":true}),
        )
    }
    pub fn read_artwork(&self, id: &str) -> Result<Value> {
        let value: Value = serde_json::from_slice(&fs::read(self.artwork_path(id)?)?)?;
        transfer(&value, &self.library_root, &self.root)?;
        crate::references::restore_artwork_records(self, &value["referenceLibrary"])?;
        Ok(value)
    }
    pub(crate) fn publish_library_assets(&self, value: &Value) -> Result<()> {
        transfer(value, &self.root, &self.library_root)
    }
    pub fn artifact_path(&self, id: &str) -> PathBuf {
        let own = self.root.join("artifacts").join(id);
        if own.is_file() {
            own
        } else {
            self.library_root.join("artifacts").join(id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use morphforge_core::{Box3, Operation};
    #[test]
    fn independent_instances_share_artworks_and_textures_but_not_projects() {
        let root = tempfile::tempdir().unwrap();
        let library = root.path().join("library");
        let mut a = Workspace::open(root.path().join("a")).unwrap();
        a.use_shared_library(library.clone()).unwrap();
        let mut b = Workspace::open(root.path().join("b")).unwrap();
        b.use_shared_library(library).unwrap();
        let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([10, 20, 30, 255]));
        let mut buffer = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut buffer, image::ImageFormat::Png)
            .unwrap();
        let pixel_bytes = buffer.into_inner();
        let texture = a.artifact(&pixel_bytes, "png").unwrap();
        a.project.materials[0].textures = vec![format!("/artifacts/{texture}"); 6];
        a.apply(
            &[Operation::Box {
                value: Box3::new([0; 3], [3; 3], 1),
            }],
            Some(0),
            None,
            None,
        )
        .unwrap();
        let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([10, 20, 30, 255]));
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let prepared = crate::references::prepare(png.get_ref()).unwrap();
        let reference = crate::references::import(&mut a, "shared reference", prepared).unwrap();
        let reference_id = reference["id"].as_str().unwrap().to_string();
        a.event(
            "artwork.reference_sources",
            Some("shared-run"),
            serde_json::json!({"references":[reference]}),
        )
        .unwrap();
        let artwork = a
            .save_artwork("shared", vec![], Some(texture.clone()), Some("shared-run"))
            .unwrap();
        assert_eq!(b.library().unwrap().len(), 1);
        assert_eq!(b.project.scene.boxes.len(), 0);
        assert_eq!(
            fs::read(b.artifact_path(&texture)).unwrap(),
            pixel_bytes.as_slice()
        );
        b.load_artwork(&artwork.id).unwrap();
        assert_eq!(
            crate::references::image(&a, &reference_id, false)
                .unwrap()
                .0,
            crate::references::image(&b, &reference_id, false)
                .unwrap()
                .0
        );
        assert_eq!(b.project.scene.boxes.len(), 1);
        assert_ne!(a.project.id, b.project.id);
        assert_eq!(
            fs::read(b.root.join("artifacts").join(texture)).unwrap(),
            pixel_bytes.as_slice()
        );
        b.apply(
            &[Operation::Clear],
            Some(b.project.scene.revision),
            None,
            None,
        )
        .unwrap();
        assert_eq!(a.project.scene.boxes.len(), 1);
    }
}
