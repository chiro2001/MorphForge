//! Update a private catalog from its actual PNG alpha; never downloads assets.
use morphforge_app::{resources, store};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::PathBuf};
fn main() -> anyhow::Result<()> {
    let root = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or_else(|| anyhow::anyhow!("catalog directory required"))?,
    );
    let mut c: Value = serde_json::from_slice(&std::fs::read(root.join("catalog.json"))?)?;
    let mut cache = BTreeMap::new();
    let mut changed = 0;
    for m in c["materials"].as_array_mut().unwrap() {
        if !m["uid"].as_str().unwrap_or("").starts_with("minecraft:") {
            continue;
        }
        let mut partial = false;
        let mut holes = false;
        for t in m["textures"].as_array().into_iter().flatten() {
            let name = t
                .as_str()
                .unwrap()
                .strip_prefix("/artifacts/")
                .ok_or_else(|| anyhow::anyhow!("invalid texture path"))?;
            store::validate_id(name)?;
            let flags = if let Some(flags) = cache.get(name) {
                *flags
            } else {
                let pixels = image::open(root.join("artifacts").join(name))?.to_rgba8();
                let flags = (
                    pixels.pixels().any(|p| p[3] > 0 && p[3] < 255),
                    pixels.pixels().any(|p| p[3] < 255),
                );
                cache.insert(name.to_string(), flags);
                flags
            };
            partial |= flags.0;
            holes |= flags.1;
        }
        let name = m["uid"].as_str().unwrap();
        let mode = resources::alpha_render_mode(name, partial, holes);
        if m["renderMode"] != mode {
            m["renderMode"] = json!(mode);
            changed += 1;
        }
    }
    store::write_json(&root.join("catalog.json"), &c)?;
    println!(
        "{}",
        json!({"changed":changed,"cachedTextures":cache.len()})
    );
    Ok(())
}
