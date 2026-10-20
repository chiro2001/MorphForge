use crate::store::Workspace;
use anyhow::{bail, Context, Result};
use morphforge_core::{
    material_wire::Surface,
    materials::{Material, Procedural},
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    base_revision: u64,
    materials: Vec<Definition>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Definition {
    name: String,
    color: String,
    kind: String,
    metallic: Option<f32>,
    roughness: Option<f32>,
    opacity: Option<f32>,
    emission: Option<f32>,
    noise_scale: Option<f32>,
    seed: Option<u32>,
}
pub(super) fn define(w: &mut Workspace, args: &Value, run: Option<&str>) -> Result<Value> {
    let request: Request = serde_json::from_value(args.clone())?;
    if request.base_revision != w.project.scene.revision {
        bail!("REVISION_CONFLICT");
    }
    if request.materials.is_empty() || request.materials.len() > 32 {
        bail!("一次定义1至32种材质");
    }
    let mut materials = w.project.materials.clone();
    let mut used: BTreeSet<_> = materials.iter().map(|m| m.id).collect();
    let mut added = 0;
    let mut entries = vec![];
    for d in request.materials {
        let name = d.name.trim();
        if name.is_empty() || name.len() > 128 {
            bail!("材质名长度须为1至128字节");
        }
        if !["color", "pbr", "water", "cloud", "wood", "stone"].contains(&d.kind.as_str()) {
            bail!("不支持的实时材质类型");
        }
        let hex = d
            .color
            .strip_prefix('#')
            .filter(|v| v.len() == 6 && v.bytes().all(|c| c.is_ascii_hexdigit()))
            .context("颜色须为#RRGGBB")?;
        let color =
            [0, 2, 4].map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap() as f32 / 255.);
        let procedural = matches!(d.kind.as_str(), "water" | "cloud" | "wood" | "stone");
        let metallic = d
            .metallic
            .unwrap_or(if d.kind == "water" { 0.2 } else { 0. });
        let roughness = d.roughness.unwrap_or(match d.kind.as_str() {
            "water" => 0.12,
            "cloud" => 1.,
            "wood" => 0.8,
            _ => 0.9,
        });
        let opacity = d.opacity.unwrap_or(match d.kind.as_str() {
            "water" => 0.65,
            "cloud" => 0.5,
            _ => 1.,
        });
        let emission = d.emission.unwrap_or(0.);
        if ![metallic, roughness, opacity]
            .iter()
            .all(|v| v.is_finite() && (0.0..=1.).contains(v))
            || !emission.is_finite()
            || !(0.0..=3.).contains(&emission)
        {
            bail!("材质参数范围：金属度/粗糙度/不透明度0..1，发光0..3");
        }
        if d.kind == "color"
            && (metallic != 0. || roughness != 0.9 || opacity != 1. || emission != 0.)
        {
            bail!("纯色为固定不透明哑光；其他表面参数请选择pbr");
        }
        if procedural && metallic != if d.kind == "water" { 0.2 } else { 0. } {
            bail!("程序化材质金属度由类型决定；自定义金属度请选择pbr");
        }
        let noise_scale = d.noise_scale.unwrap_or(1.);
        if !noise_scale.is_finite() || !(0.25..=8.).contains(&noise_scale) {
            bail!("噪声比例范围0.25..8");
        }
        if !procedural && (d.noise_scale.is_some() || d.seed.is_some()) {
            bail!("噪声参数仅适用于water/cloud/wood/stone");
        }
        let mut m = Material {
            id: 0,
            uid: String::new(),
            name: name.into(),
            family: if d.kind == "pbr" {
                if metallic > 0.5 {
                    "metal"
                } else {
                    "matte"
                }
            } else if d.kind == "color" {
                "matte"
            } else {
                &d.kind
            }
            .into(),
            color,
            metallic,
            roughness,
            opacity,
            emission,
            render_mode: if opacity < 1. {
                "transparent"
            } else {
                "opaque"
            }
            .into(),
            textures: vec![],
            frames: 1,
            frame_time: 0.1,
            pack: None,
            source: Some("MorphForge runtime material/1".into()),
            procedural: if procedural {
                Some(Procedural {
                    kind: d.kind,
                    scale: noise_scale,
                    seed: d.seed.unwrap_or(0),
                })
            } else {
                None
            },
        };
        let identity = json!({"color":m.color,"metallic":m.metallic,"roughness":m.roughness,"opacity":m.opacity,"emission":m.emission,"procedural":m.procedural});
        m.uid = format!(
            "mf:runtime-{}",
            crate::store::hash(&serde_json::to_vec(&identity)?)
        );
        if let Some(existing) = materials.iter().find(|v| v.uid == m.uid) {
            if existing.color != m.color
                || existing.metallic != m.metallic
                || existing.roughness != m.roughness
                || existing.opacity != m.opacity
                || existing.emission != m.emission
                || existing.procedural != m.procedural
                || !existing.textures.is_empty()
            {
                bail!("RUNTIME_MATERIAL_UID_CONFLICT");
            }
            m = existing.clone();
        } else {
            m.id = (25..=65535)
                .find(|id| !used.contains(id))
                .context("材质目录已满")?;
            used.insert(m.id);
            materials.push(m.clone());
            added += 1;
        }
        let encoded = Surface::runtime(&m, 0).context("该材质无法编为实时表面")?;
        entries.push(json!({"id":m.id,"uid":m.uid,"name":m.name,"wireMode":encoded.mode,"vrcEffective":encoded.decode(),"mf5RebuildRequired":false}));
    }
    if added > 0 {
        w.replace_with_materials(
            w.project.scene.clone(),
            materials,
            Some(request.base_revision),
            run,
        )?;
    }
    let receipt = json!({"ok":true,"revision":w.project.scene.revision,"materials":entries,"added":added,"geometryChanged":false,"note":"MF5实时材质可随绘制发送，不需重传头像。PBR/程序材质按vrcEffective量化；原始艺术材质保持不变。MF4及更早仍需目录打包。"});
    w.event("materials.runtime.defined", run, receipt.clone())?;
    Ok(receipt)
}
