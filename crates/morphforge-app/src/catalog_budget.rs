//! Estimate the same cell sampling/deduplication and resident textures as Unity.
use crate::store::Workspace;
use anyhow::{bail, Context, Result};
use morphforge_core::materials::Material;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
pub fn selected(w: &Workspace, used_only: bool) -> Vec<Material> {
    let used: BTreeSet<_> = w.project.scene.boxes.iter().map(|b| b.style_id).collect();
    let mut materials = w
        .project
        .materials
        .iter()
        .filter(|m| !used_only || used.contains(&m.id))
        .cloned()
        .collect::<Vec<_>>();
    if materials.is_empty() {
        materials.push(morphforge_core::materials::builtins().remove(0));
    }
    materials
}
pub fn estimate(w: &Workspace, materials: &[Material], cell: u32) -> Result<Value> {
    if ![16, 32, 64].contains(&cell) {
        bail!("材质格分辨率须为16/32/64")
    }
    let mut images = BTreeMap::new();
    let mut groups = BTreeSet::new();
    let mut tiles = BTreeSet::new();
    let (mut x, mut y, mut row, mut width, mut height, mut map_count, mut sampled) =
        (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0usize);
    let mut animated = 0;
    for m in materials {
        if m.textures.is_empty() {
            continue;
        }
        if m.textures.len() != 6 {
            bail!("目录材质{}必须包含六面纹理", m.name)
        }
        let frames = if m.frames > 0 {
            m.frames
        } else {
            let name = m.textures[0]
                .strip_prefix("/artifacts/")
                .context("纹理必须为本机artifact")?;
            crate::store::validate_id(name)?;
            let (wi, he) = image::image_dimensions(w.artifact_path(name))?;
            if wi == 0 || he % wi != 0 {
                bail!("无效动画条带")
            };
            he / wi
        };
        if frames > 1024 {
            bail!("动画帧超过1024")
        }
        if frames > 1 {
            animated += 1
        }
        let group = format!("{frames}:{}", m.textures.join("|"));
        if !groups.insert(group) {
            continue;
        }
        if x + 6 * cell as u64 > 8192 {
            x = 0;
            y += row;
            row = 0;
        }
        width = width.max(x + 6 * cell as u64);
        height = height.max(y + frames as u64 * cell as u64);
        row = row.max(frames as u64 * cell as u64);
        x += 6 * cell as u64;
        map_count += frames as u64 * 6;
        for path in &m.textures {
            let name = path
                .strip_prefix("/artifacts/")
                .context("纹理必须为本机artifact")?;
            crate::store::validate_id(name)?;
            if !images.contains_key(name) {
                let bytes = std::fs::read(w.artifact_path(name))?;
                let image = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)?
                    .to_rgba8();
                if image.width() == 0 || image.height() != image.width() * frames {
                    bail!("材质{}帧高不匹配", m.name)
                }
                images.insert(name.to_owned(), image);
            }
            let image = &images[name];
            for frame in 0..frames {
                sampled = sampled.saturating_add(cell as usize * cell as usize);
                if sampled > 128 * 1024 * 1024 {
                    bail!("材质预算采样超过128M像素，请选仅当前作品")
                }
                let mut pixels = Vec::with_capacity((cell * cell * 4) as usize);
                for yy in 0..cell {
                    for xx in 0..cell {
                        pixels.extend_from_slice(
                            &image
                                .get_pixel(
                                    xx * image.width() / cell,
                                    frame * image.width() + yy * image.width() / cell,
                                )
                                .0,
                        );
                    }
                }
                tiles.insert(crate::store::hash(&pixels));
            }
        }
    }
    let mut level = cell as u64;
    let mut per_layer = 0;
    loop {
        per_layer += level * level * 4;
        if level == 1 {
            break;
        }
        level = (level / 2).max(1);
    }
    let layer_bytes = tiles.len() as u64 * per_layer;
    let legacy_atlas_bytes = width.max(1) * height.max(1) * 4;
    let atlas_bytes = 4;
    let face_bytes = if map_count > 0 {
        256 * map_count.div_ceil(256) * 4
    } else {
        0
    };
    let total = layer_bytes + atlas_bytes + face_bytes + 256 * 64 * 16;
    Ok(
        json!({"cellPixels":cell,"styles":materials.len(),"textureGroups":groups.len(),"uniqueSourcePngs":images.len(),"animatedStyles":animated,"uniqueSlices":tiles.len(),"layerBanks":tiles.len().div_ceil(2048),"faceMapEntries":map_count,"atlasSize":[1,1],"legacyAtlasSize":[width.max(1),height.max(1)],"legacyAtlasBytes":legacy_atlas_bytes,"bytes":{"atlas":atlas_bytes,"layerMipmaps":layer_bytes,"faceMap":face_bytes,"metadata":256*64*16,"totalGpuEstimate":total},"fits":materials.len()<=4095&&tiles.len()<=8192&&layer_bytes<=64*1024*1024,"additionalSyncedBits":0,"additionalAnimatorLayers":0,"additionalParticleSystems":0,"note":"预估GPU纹理数据；Unity原生报告与实际SDK包体另测，不含CPU可读副本、平台分配和原人物纹理。"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_faces_rotations_and_animation_share_only_two_real_slices() {
        let root = tempfile::tempdir().unwrap();
        let mut w = Workspace::open(root.path().into()).unwrap();
        let mut pixels = image::RgbaImage::new(16, 32);
        for (x, y, p) in pixels.enumerate_pixels_mut() {
            *p = image::Rgba(if y < 16 {
                [x as u8, 1, 2, 255]
            } else {
                [3, 4, 5, 255]
            });
        }
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(pixels)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        let name = w.artifact(out.get_ref(), "png").unwrap();
        let mut m = w.project.materials[0].clone();
        m.frames = 2;
        m.frame_time = 0.05;
        m.textures = vec![format!("/artifacts/{name}"); 6];
        let mut rotated = m.clone();
        rotated.id = 100;
        rotated.texture_rotation = 1;
        let r = estimate(&w, &[m, rotated], 16).unwrap();
        assert_eq!(r["uniqueSlices"], 2);
        assert_eq!(r["faceMapEntries"], 12);
        assert_eq!(r["bytes"]["atlas"], 4);
        assert_eq!(r["fits"], true);
        assert!(r["bytes"]["totalGpuEstimate"].as_u64().unwrap() < 1024 * 1024);
    }
}
