//! User-declared Avatar catalog; a successful comparison is not a remote Avatar handshake.
use anyhow::{bail, Context, Result};
use morphforge_core::{materials::Material, Scene};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read},
};
pub const SURFACE_CAPABILITY: &str = "transparent-face-mask/1";
#[derive(Clone, Serialize, Deserialize)]
pub struct AvatarCatalog {
    pub format: String,
    #[serde(default)]
    pub version: String,
    pub materials: Vec<Material>,
    #[serde(default)]
    pub capabilities: Vec<String>,
}
fn rendering(m: &Material) -> Value {
    json!({"id":m.id,"uid":m.uid,"color":m.color,"metallic":m.metallic,"roughness":m.roughness,"opacity":m.opacity,"emission":m.emission,"renderMode":m.render_mode,"textures":m.textures,"frames":m.frames,"frameTime":m.frame_time})
}
impl AvatarCatalog {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 16 * 1024 * 1024 {
            bail!("目标目录文件超过16MiB");
        }
        let catalog = if bytes.starts_with(b"PK") {
            let mut zip = zip::ZipArchive::new(Cursor::new(bytes))?;
            let f = zip.by_name("catalog.json").context("ZIP缺少catalog.json")?;
            if f.size() > 4 * 1024 * 1024 {
                bail!("catalog.json过大");
            }
            let mut data = Vec::new();
            f.take(4 * 1024 * 1024 + 1).read_to_end(&mut data)?;
            serde_json::from_slice(&data)?
        } else {
            serde_json::from_slice(bytes)?
        };
        Self::validate(catalog)
    }
    fn validate(mut value: Self) -> Result<Self> {
        if value.format != "MorphForgeMaterialCatalog/1"
            || value.materials.is_empty()
            || value.materials.len() > 4095
        {
            bail!("无效的Avatar材质目录");
        }
        let mut ids = BTreeSet::new();
        for m in &value.materials {
            if m.id == 0
                || m.id > 4095
                || !ids.insert(m.id)
                || !m
                    .color
                    .iter()
                    .all(|x| x.is_finite() && (0.0..=1.0).contains(x))
                || !m.opacity.is_finite()
                || !(0.0..=1.0).contains(&m.opacity)
                || !m.metallic.is_finite()
                || !m.roughness.is_finite()
                || !m.emission.is_finite()
                || !m.frame_time.is_finite()
                || !["opaque", "cutout", "transparent"].contains(&m.render_mode.as_str())
            {
                bail!("目录包含越界/重复/非有限材质");
            }
        }
        value.materials.sort_by_key(|m| m.id);
        value.version = crate::store::hash(&serde_json::to_vec(
            &value.materials.iter().map(rendering).collect::<Vec<_>>(),
        )?);
        Ok(value)
    }
    pub fn surface_masks(&self) -> bool {
        self.capabilities.iter().any(|c| c == SURFACE_CAPABILITY)
    }
}
pub fn compatibility(
    scene: &Scene,
    materials: &[Material],
    profile: Option<&AvatarCatalog>,
) -> Value {
    let used: BTreeSet<_> = scene.boxes.iter().map(|b| b.style_id).collect();
    let Some(profile) = profile else {
        return json!({"status":"unknown","matched":null,"usedStyles":used.len(),"note":"未导入目标Avatar目录；OSC不传贴图，未核实上传的Avatar。","remoteVerified":false});
    };
    let lookup: BTreeMap<_, _> = profile.materials.iter().map(|m| (m.id, m)).collect();
    let source: BTreeMap<_, _> = materials.iter().map(|m| (m.id, m)).collect();
    let mut missing = vec![];
    let mut different = vec![];
    for id in &used {
        match (source.get(id), lookup.get(id)) {
            (Some(a), Some(b)) if rendering(a) == rendering(b) => {}
            (_, None) => missing.push(*id),
            _ => different.push(*id),
        }
    }
    json!({"status":if missing.is_empty()&&different.is_empty(){"matched"}else{"mismatch"},"matched":missing.is_empty()&&different.is_empty(),"usedStyles":used.len(),"targetStyles":profile.materials.len(),"fingerprint":profile.version,"missingIds":missing,"differentIds":different,"surfaceMasks":profile.surface_masks(),"remoteVerified":false,"note":"匹配用户导入的目录；没有远端Avatar确认。新增材质需导出并重新打包Avatar。"})
}
pub fn project(
    scene: &Scene,
    materials: &[Material],
    profile: Option<&AvatarCatalog>,
) -> Result<Scene> {
    let report = compatibility(scene, materials, profile);
    if report["matched"] == false {
        bail!(
            "MATERIAL_CATALOG_MISMATCH：目标Avatar缺少材质或定义不同：{}",
            report
        );
    }
    let mut result = scene.clone();
    if profile.is_some_and(AvatarCatalog::surface_masks) {
        let surfaces = morphforge_core::surfaces::transparent_interfaces(scene, materials);
        let transparent: BTreeSet<_> = materials
            .iter()
            .filter(|m| m.render_mode == "transparent")
            .map(|m| m.id)
            .collect();
        result.boxes.retain_mut(|b| {
            if transparent.contains(&b.style_id) {
                b.motion_phase = surfaces.masks.get(&b.id).copied().unwrap_or(0);
                b.motion_phase != 63
            } else {
                true
            }
        });
    }
    Ok(result)
}
