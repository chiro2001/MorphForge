//! glTF group transforms share the exact pivot/TRS conventions of the viewer.
use crate::creative::{quaternion, validate_clip, Clip};
use anyhow::{bail, Context, Result};
use morphforge_core::{materials::Material, Scene};
use serde_json::{json, Value};
use std::path::Path;

fn accessor(doc: &mut Value, bin: &mut Vec<u8>, values: &[f32], width: usize) -> usize {
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    let offset = bin.len();
    for n in values {
        bin.extend_from_slice(&n.to_le_bytes());
    }
    let views = doc["bufferViews"].as_array_mut().unwrap();
    let view = views.len();
    views.push(json!({"buffer":0,"byteOffset":offset,"byteLength":values.len()*4}));
    let accessors = doc["accessors"].as_array_mut().unwrap();
    let id = accessors.len();
    let mut a = json!({"bufferView":view,"componentType":5126,"count":values.len()/width,"type":match width{1=>"SCALAR",3=>"VEC3",_=>"VEC4"}});
    if width == 1 {
        a["min"] = json!([values[0]]);
        a["max"] = json!([values[values.len() - 1]]);
    }
    accessors.push(a);
    id
}
pub fn export(scene: &Scene, materials: &[Material], root: &Path, clip: &Clip) -> Result<Vec<u8>> {
    validate_clip(clip)?;
    let static_glb = crate::model_export::export(
        scene,
        materials,
        root,
        &crate::model_export::Options {
            format: "glb".into(),
            cell_size: None,
        },
    )?
    .bytes;
    let json_len = u32::from_le_bytes(static_glb[12..16].try_into().unwrap()) as usize;
    let mut doc: Value = serde_json::from_slice(&static_glb[20..20 + json_len])?;
    let mut bin = static_glb[28 + json_len..].to_vec();
    let mut roots = doc["scenes"][0]["nodes"].as_array().unwrap().clone();
    let mut channels = vec![];
    let mut samplers = vec![];
    for track in &clip.tracks {
        let children: Vec<_> = doc["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .filter(|(_, n)| n["name"] == track.group && n.get("mesh").is_some())
            .map(|(i, _)| i)
            .collect();
        if children.is_empty() {
            bail!("动画分组没有可导出的几何：{}", track.group)
        }
        let pivot = [
            track.pivot[0] * scene.cell_size,
            track.pivot[1] * scene.cell_size,
            -track.pivot[2] * scene.cell_size,
        ];
        let node = doc["nodes"].as_array().unwrap().len();
        for &child in &children {
            doc["nodes"][child]["translation"] = json!(pivot.map(|v| -v));
        }
        roots.retain(|v| !children.iter().any(|&n| v.as_u64() == Some(n as u64)));
        roots.push(json!(node));
        doc["nodes"].as_array_mut().unwrap().push(json!({"name":format!("motion:{}",track.group),"children":children,"translation":pivot}));
        let mut keys = track.keys.clone();
        if keys[0].time > 0. {
            let mut key = keys[0].clone();
            key.time = 0.;
            keys.insert(0, key)
        }
        if keys.last().unwrap().time < clip.duration {
            let mut key = keys.last().unwrap().clone();
            key.time = clip.duration;
            keys.push(key)
        }
        let times: Vec<_> = keys.iter().map(|k| k.time as f32).collect();
        if times.windows(2).any(|t| t[0] >= t[1]) {
            bail!("关键帧时间间距小于GLB浮点精度")
        }
        let input = accessor(&mut doc, &mut bin, &times, 1);
        let mut previous: Option<[f64; 4]> = None;
        let rotations: Vec<f32> = keys
            .iter()
            .flat_map(|key| {
                let mut q = quaternion(key.rotation);
                if previous.is_some_and(|p| p.iter().zip(q).map(|(a, b)| a * b).sum::<f64>() < 0.) {
                    q = q.map(|v| -v)
                }
                previous = Some(q);
                q.map(|v| v as f32)
            })
            .collect();
        for (path, width, values) in [
            (
                "translation",
                3,
                keys.iter()
                    .flat_map(|key| {
                        [
                            pivot[0] + key.translation[0] * scene.cell_size,
                            pivot[1] + key.translation[1] * scene.cell_size,
                            pivot[2] - key.translation[2] * scene.cell_size,
                        ]
                        .map(|v| v as f32)
                    })
                    .collect::<Vec<_>>(),
            ),
            ("rotation", 4, rotations),
            (
                "scale",
                3,
                keys.iter()
                    .flat_map(|key| key.scale.map(|v| v as f32))
                    .collect(),
            ),
        ] {
            let output = accessor(&mut doc, &mut bin, &values, width);
            let sampler = samplers.len();
            samplers.push(json!({"input":input,"output":output,"interpolation":if track.interpolation=="step"{"STEP"}else{"LINEAR"}}));
            channels.push(json!({"sampler":sampler,"target":{"node":node,"path":path}}));
        }
    }
    doc["scenes"][0]["nodes"] = json!(roots);
    doc["animations"] = json!([{"name":clip.name,"samplers":samplers,"channels":channels,"extras":{"loop":clip.looping,"duration":clip.duration}}]);
    doc["buffers"][0]["byteLength"] = json!(bin.len());
    doc["extras"]["note"]=json!("Group TRS animation; XYZ Euler authoring, shortest-path quaternion interpolation, Y up. Textures use first frame; procedural materials use base color. Loop is player-defined.");
    let mut text = serde_json::to_vec(&doc)?;
    while !text.len().is_multiple_of(4) {
        text.push(b' ')
    }
    while !bin.len().is_multiple_of(4) {
        bin.push(0)
    }
    let len = 12 + 8 + text.len() + 8 + bin.len();
    let len = u32::try_from(len).context("GLB过大")?;
    let mut out = Vec::new();
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&(text.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend(text);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend(bin);
    Ok(out)
}
