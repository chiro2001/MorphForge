//! Offline Avatar material assets. Export never publishes or uploads an avatar.
use crate::store::Workspace;
use anyhow::{bail, Context, Result};
use serde_json::json;
use std::{
    collections::BTreeSet,
    io::{Cursor, Write},
};
pub fn export(w: &Workspace) -> Result<Vec<u8>> {
    let mut ids = BTreeSet::new();
    let mut textures = BTreeSet::new();
    for m in &w.project.materials {
        if m.id > 4095 || !ids.insert(m.id) {
            bail!("Avatar材质目录需要唯一的0..4095 styleId")
        };
        for t in &m.textures {
            let name = t
                .strip_prefix("/artifacts/")
                .context("目录纹理须为本机artifact")?;
            crate::store::validate_id(name)?;
            if !name.ends_with(".png") {
                bail!("Avatar材质导出仅支持PNG")
            };
            textures.insert(name.to_owned());
        }
    }
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("catalog.json", options)?;
    zip.write_all(serde_json::to_string_pretty(&json!({"format":"MorphForgeMaterialCatalog/1","version":crate::store::hash(&serde_json::to_vec(&w.project.materials)?),"capabilities":["virtual-light/1",crate::target::SURFACE_CAPABILITY],"virtualLighting":{"direction":[0.4,0.8,0.6],"voxelGrid":false},"colorSpace":"sRGB","cellSize":0.25,"materials":w.project.materials,"sourceProject":w.project.id,"sourceRevision":w.project.scene.revision}))?.as_bytes())?;
    zip.start_file("licenses/MorphForge-Pixel-MIT.txt", options)?;
    zip.write_all(crate::builtin_pixels::LICENSE.as_bytes())?;
    let mut total = 0usize;
    for name in textures {
        let path = w.root.join("artifacts").join(&name);
        let metadata = std::fs::metadata(&path)?;
        total += metadata.len() as usize;
        if total > 128 * 1024 * 1024 {
            bail!("Avatar材质贴图导出超过128MiB保护值")
        };
        zip.start_file(format!("artifacts/{name}"), options)?;
        zip.write_all(&std::fs::read(path)?)?;
    }
    Ok(zip.finish()?.into_inner())
}
