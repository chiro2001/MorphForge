//! Offline Avatar material assets. Export never publishes or uploads an avatar.
use crate::store::Workspace;
use anyhow::{bail, Context, Result};
use serde_json::json;
use std::{
    collections::BTreeSet,
    io::{Cursor, Write},
};
pub fn export(w: &Workspace) -> Result<Vec<u8>> {
    export_with_materials(w, &w.project.materials)
}
pub fn export_with_materials(
    w: &Workspace,
    source_materials: &[morphforge_core::materials::Material],
) -> Result<Vec<u8>> {
    export_configured(w, source_materials, 32)
}
pub fn export_configured(
    w: &Workspace,
    source_materials: &[morphforge_core::materials::Material],
    cell_pixels: u32,
) -> Result<Vec<u8>> {
    let budget = crate::catalog_budget::estimate(w, source_materials, cell_pixels)?;
    if budget["fits"] != true {
        bail!("Avatar材质预算超限，请减少材质、动画或使用16像素格：{budget}")
    }
    let mut ids = BTreeSet::new();
    let mut textures = BTreeSet::new();
    let mut materials: Vec<_> = source_materials
        .iter()
        .filter(|m| {
            m.id <= 4095 || morphforge_core::material_wire::Surface::runtime(m, 0).is_none()
        })
        .cloned()
        .collect();
    // A runtime-only project still exports a valid, nonempty static lookup.
    if materials.is_empty() {
        materials.push(morphforge_core::materials::builtins().remove(0));
    }
    if materials.len() > 4095 {
        bail!("头像静态材质超过4095项，请减少静态目录；纯色可以实时发送");
    }
    let mut assigned: BTreeSet<_> = materials
        .iter()
        .filter(|m| m.id > 0 && m.id <= 4095)
        .map(|m| m.id)
        .collect();
    let mut source_mapping = serde_json::Map::new();
    for material in &mut materials {
        let source = material.id;
        if source == 0 || source > 4095 {
            material.id = (1..=4095)
                .find(|id| !assigned.contains(id))
                .context("头像材质槽位已满")?;
            assigned.insert(material.id);
        }
        source_mapping.insert(source.to_string(), json!(material.id));
    }
    for m in &materials {
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
    zip.write_all(serde_json::to_string_pretty(&json!({"format":"MorphForgeMaterialCatalog/1","textureOptions":{"cellPixels":cell_pixels,"deduplicateFaces":true,"maxLayerBanks":4,"layeredOnly":true},"version":crate::store::hash(&serde_json::to_vec(source_materials)?),"capabilities":["virtual-light/1",crate::target::SURFACE_CAPABILITY,"texture-scale/1","texture-rotation/1"],"virtualLighting":{"direction":[0.4,0.8,0.6],"voxelGrid":false},"colorSpace":"sRGB","cellSize":0.25,"materials":materials,"sourceStyleIdMap":source_mapping,"sourceProject":w.project.id,"sourceRevision":w.project.scene.revision}))?.as_bytes())?;
    zip.start_file("build-budget.json", options)?;
    zip.write_all(serde_json::to_string_pretty(&budget)?.as_bytes())?;
    zip.start_file("licenses/RESOURCE-SOURCES.json", options)?;
    zip.write_all(serde_json::to_string_pretty(&json!({"sourceCredits":w.project.source_credits,"materials":materials.iter().map(|m|json!({"uid":m.uid,"source":m.source,"pack":m.pack})).collect::<Vec<_>>(),"note":"原始资源保留来源许可；MorphForge原创像素材质的MIT声明仅适用于原创像素材质。"}))?.as_bytes())?;
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
