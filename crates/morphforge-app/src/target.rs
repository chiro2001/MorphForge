//! User-declared Avatar catalog; a successful comparison is not a remote Avatar handshake.
use anyhow::{bail, Context, Result};
use morphforge_core::{
    material_wire::{Surface, CAPABILITY},
    materials::Material,
    protocol::{self, GeometryKey},
    Scene,
};
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
    let mut v = json!({"id":m.id,"uid":m.uid,"family":m.family,"color":m.color,"metallic":m.metallic,"roughness":m.roughness,"opacity":m.opacity,"emission":m.emission,"renderMode":m.render_mode,"textures":m.textures,"frames":m.frames,"frameTime":m.frame_time});
    if let Some(p) = &m.procedural {
        v["procedural"] = json!(p);
    }
    v
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
    pub fn runtime_materials(&self) -> bool {
        self.capabilities.iter().any(|c| c == CAPABILITY)
    }
    pub fn wire_protocol(&self) -> Option<&str> {
        if self.runtime_materials() {
            Some("mf5")
        } else if self.wide_protocol() {
            Some("mf4")
        } else {
            None
        }
    }
    pub fn wide_protocol(&self) -> bool {
        self.capabilities.iter().any(|c| c == "mf4-three-lane/1")
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
        let missing: Vec<_> = used.iter().copied().filter(|id| *id > 24).collect();
        if !missing.is_empty() {
            return json!({"status":"unverified_custom","matched":false,"missingIds":missing,"differentIds":[],"remoteVerified":false,"note":"自定义材质不能只按编号发送：请导入实际上传头像的材质目录，避免编号碰撞和动画闪烁。"});
        }
        return json!({"status":"unknown","matched":null,"usedStyles":used.len(),"note":"未导入目标Avatar目录；OSC不传贴图，未核实上传的Avatar。","remoteVerified":false});
    };
    if profile.runtime_materials() {
        return runtime_compatibility(scene, materials, profile);
    }
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
pub fn legacy_project(
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

fn signature(m: &Material, ignore_color: bool) -> Value {
    let mut v = rendering(m);
    v.as_object_mut().unwrap().remove("id");
    v.as_object_mut().unwrap().remove("uid");
    if ignore_color {
        v.as_object_mut().unwrap().remove("color");
    }
    v
}
fn resolve(m: &Material, tint: u8, motion: u8, target: &AvatarCatalog) -> Result<Surface> {
    if m.procedural.is_some()
        || m.uid.starts_with("mf:palette-")
        || m.uid.starts_with("mf:runtime-")
    {
        if let Some(surface) = Surface::runtime(m, tint) {
            return Ok(surface);
        }
    }
    // UID disambiguates IDs allocated separately by each artwork. Content must
    // still match: never let a reused UID silently substitute another texture.
    if let Some(t) = target
        .materials
        .iter()
        .find(|t| t.uid == m.uid && signature(t, false) == signature(m, false))
    {
        return Ok(Surface::catalog(t.id, tint, motion));
    }
    if let Some(surface) = Surface::runtime(m, tint) {
        return Ok(surface);
    }
    if !m.textures.is_empty() {
        if let Some(t) = target
            .materials
            .iter()
            .find(|t| signature(t, true) == signature(m, true))
        {
            return Ok(Surface::textured_tint(
                t.id,
                morphforge_core::material_wire::tinted(m.color, tint),
            ));
        }
    }
    bail!("MF5_TEXTURE_NOT_BUILTIN：材质 {} 的贴图/裁切定义不在头像中；颜色、PBR和程序材质可实时发送，新图片仍需打包",m.name)
}
fn runtime_compatibility(scene: &Scene, materials: &[Material], target: &AvatarCatalog) -> Value {
    let used: BTreeSet<_> = scene.boxes.iter().map(|b| b.style_id).collect();
    let lookup: BTreeMap<_, _> = materials.iter().map(|m| (m.id, m)).collect();
    let mut missing = vec![];
    let mut runtime = vec![];
    let mut remapped = vec![];
    let mut details = vec![];
    for id in &used {
        match lookup.get(id).and_then(|m| resolve(m, 0, 0, target).ok()) {
            Some(s) => {
                if s.mode > 0 {
                    runtime.push(*id);
                } else {
                    let target_id = morphforge_core::material_wire::bits(s.bytes) & 4095;
                    if target_id != *id {
                        remapped.push(json!({"sourceId":id,"targetId":target_id}));
                    }
                }
                details.push(json!({"id":id,"mode":s.mode,"runtime":s.mode>0}));
            }
            None => missing.push(*id),
        }
    }
    json!({"status":if missing.is_empty(){"matched"}else{"mismatch"},"matched":missing.is_empty(),"usedStyles":used.len(),"targetStyles":target.materials.len(),"fingerprint":target.version,"missingIds":missing,"differentIds":[],"runtimeIds":runtime,"remappedIds":remapped,"details":details,"surfaceMasks":target.surface_masks(),"runtimeMaterials":true,"remoteVerified":false,"note":"MF5：纯色、PBR、程序化材质实时发送；内置贴图按身份映射，不依赖作品编号。新图片贴图需重新打包。"})
}
#[derive(Clone)]
pub struct Projection {
    pub scene: Scene,
    pub surfaces: BTreeMap<GeometryKey, Surface>,
}
pub fn project_wire(
    scene: &Scene,
    materials: &[Material],
    profile: Option<&AvatarCatalog>,
) -> Result<Projection> {
    if !profile.is_some_and(AvatarCatalog::runtime_materials) {
        return Ok(Projection {
            scene: legacy_project(scene, materials, profile)?,
            surfaces: BTreeMap::new(),
        });
    }
    let target = profile.unwrap();
    let lookup: BTreeMap<_, _> = materials.iter().map(|m| (m.id, m)).collect();
    let mut result = scene.clone();
    let mut surfaces = BTreeMap::new();
    let masks = morphforge_core::surfaces::transparent_interfaces(scene, materials);
    result.boxes.retain_mut(|b| {
        // Resolution errors are reported below without silently omitting boxes.
        if let Some(m) = lookup.get(&b.style_id) {
            if let Ok(mut surface) = resolve(m, b.tint_id, b.motion_phase, target) {
                if surface.mode == 0 && target.surface_masks() && m.render_mode == "transparent" {
                    let mask = masks.masks.get(&b.id).copied().unwrap_or(0);
                    if mask == 63 {
                        return false;
                    }
                    surface = Surface::catalog(
                        morphforge_core::material_wire::bits(surface.bytes) & 4095,
                        b.tint_id,
                        mask,
                    );
                }
                surfaces.insert(protocol::key(b), surface);
            }
        }
        true
    });
    for b in &result.boxes {
        if !surfaces.contains_key(&protocol::key(b)) {
            let m = lookup.get(&b.style_id).context("作品引用不存在的材质")?;
            resolve(m, b.tint_id, b.motion_phase, target)?;
        }
    }
    Ok(Projection {
        scene: result,
        surfaces,
    })
}
pub fn project(
    scene: &Scene,
    materials: &[Material],
    profile: Option<&AvatarCatalog>,
) -> Result<Scene> {
    Ok(project_wire(scene, materials, profile)?.scene)
}

/// Presentation metadata only; canonical scene/material history is unchanged.
pub fn preview_materials(materials: &[Material], target: Option<&AvatarCatalog>) -> Vec<Value> {
    materials.iter().map(|m|{
        let mut v=serde_json::to_value(m).unwrap();
        if let Some(t)=target.filter(|t|t.runtime_materials()) {if let Ok(s)=resolve(m,0,0,t) {
            v["vrcMode"]=json!(s.mode);
            if let Some(d)=s.decode() {
                let mut effective=json!({"color":d.color,"metallic":d.metallic,"roughness":d.roughness,"opacity":d.opacity,"emission":d.emission,"family":if s.mode<=2{"matte"}else{m.family.as_str()},"renderMode":if d.opacity<1.{"transparent"}else{"opaque"}});
                if let Some(p)=&m.procedural {effective["procedural"]=json!({"kind":p.kind,"scale":d.noise_scale,"seed":d.seed});}
                v["vrcEffective"]=effective;
            }else if s.mode==7 {
                let bits=morphforge_core::material_wire::bits(s.bytes);v["vrcEffective"]=json!({"color":[((bits>>12)&15) as f32/15.,((bits>>16)&15) as f32/15.,((bits>>20)&15) as f32/15.]});
            }
        }}v
    }).collect()
}
pub fn decorate_preview(
    scene: &Scene,
    materials: &[Material],
    target: Option<&AvatarCatalog>,
) -> Value {
    let mut value = serde_json::to_value(scene).unwrap();
    let Some(target) = target.filter(|t| t.runtime_materials()) else {
        return value;
    };
    let lookup: BTreeMap<_, _> = materials.iter().map(|m| (m.id, m)).collect();
    for b in value["boxes"].as_array_mut().unwrap() {
        let Some(m) = b["styleId"]
            .as_u64()
            .and_then(|id| lookup.get(&(id as u32)))
        else {
            continue;
        };
        if let Ok(s) = resolve(
            m,
            b["tintId"].as_u64().unwrap_or(0) as u8,
            b["motionPhase"].as_u64().unwrap_or(0) as u8,
            target,
        ) {
            if s.mode > 0 {
                if let Some(d) = s.decode() {
                    b["vrcColor"] = json!(d.color);
                } else if s.mode == 7 {
                    let bits = morphforge_core::material_wire::bits(s.bytes);
                    b["vrcColor"] = json!([
                        ((bits >> 12) & 15) as f32 / 15.,
                        ((bits >> 16) & 15) as f32 / 15.,
                        ((bits >> 20) & 15) as f32 / 15.
                    ]);
                }
                b["vrcHiddenFaces"] = json!(0);
            }
        }
    }
    value
}
