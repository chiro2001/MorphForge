use crate::store::Workspace;
use anyhow::{bail, Context, Result};
use morphforge_core::materials::Material;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PaletteRequest {
    base_revision: u64,
    colors: Vec<ColorRequest>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ColorRequest {
    name: String,
    color: String,
}

/// Append deterministic opaque colors; never recolor an existing material.
pub(super) fn define(workspace: &mut Workspace, args: &Value, run: Option<&str>) -> Result<Value> {
    let request: PaletteRequest = serde_json::from_value(args.clone())?;
    if request.base_revision != workspace.project.scene.revision {
        bail!("REVISION_CONFLICT")
    }
    if request.colors.is_empty() || request.colors.len() > 32 {
        bail!("PALETTE_SIZE：一次定义1至32个颜色")
    }
    let mut materials = workspace.project.materials.clone();
    let mut used: BTreeSet<_> = materials.iter().map(|m| m.id).collect();
    let mut palette = Vec::new();
    let mut added = 0;
    for entry in request.colors {
        let name = entry.name.trim();
        if name.is_empty() || name.len() > 128 {
            bail!("PALETTE_NAME：颜色名称须为1至128字节")
        }
        let hex = entry
            .color
            .strip_prefix('#')
            .filter(|s| s.len() == 6 && s.bytes().all(|c| c.is_ascii_hexdigit()))
            .context("PALETTE_COLOR：颜色须为#RRGGBB（sRGB）")?
            .to_ascii_lowercase();
        let rgb = [0, 2, 4]
            .map(|offset| u8::from_str_radix(&hex[offset..offset + 2], 16).unwrap() as f32 / 255.0);
        let uid = format!("mf:palette-{hex}");
        let id = if let Some(existing) = materials.iter().find(|m| m.uid == uid) {
            if existing.id > 65535
                || existing.color != rgb
                || existing.source.as_deref() != Some("MorphForge palette/1")
                || existing.family != "matte"
                || existing.render_mode != "opaque"
                || !existing.textures.is_empty()
                || existing.opacity != 1.0
                || existing.metallic != 0.0
                || existing.emission != 0.0
            {
                bail!("PALETTE_UID_CONFLICT：同名UID已有不同材质，不会覆盖")
            }
            existing.id
        } else {
            let id = (25..=65535)
                .find(|id| !used.contains(id))
                .context("PALETTE_CAPACITY：25..65535内无可用材质ID，请复用已有色板")?;
            used.insert(id);
            materials.push(Material {
                id,
                uid: uid.clone(),
                name: name.to_owned(),
                family: "matte".into(),
                color: rgb,
                metallic: 0.0,
                roughness: 0.9,
                opacity: 1.0,
                emission: 0.0,
                render_mode: "opaque".into(),
                textures: Vec::new(),
                frames: 0,
                frame_time: 0.0,
                pack: None,
                source: Some("MorphForge palette/1".into()),
                procedural: None,
            });
            added += 1;
            id
        };
        palette.push(json!({"name":name,"id":id,"uid":uid,"color":format!("#{hex}"),"rgb":rgb}));
    }
    // Validate the entire palette before the first mutation, and preserve geometry
    // and existing materials through the standard versioned-material transaction.
    if added > 0 {
        workspace.replace_with_materials(
            workspace.project.scene.clone(),
            materials,
            Some(request.base_revision),
            run,
        )?;
    }
    let value = json!({
        "ok":true,"revision":workspace.project.scene.revision,"palette":palette,
        "added":added,"geometryChanged":false,
        "vrcCatalogExportRequired":added > 0,"mf5RebuildRequired":false,
        "hint":"Use returned IDs/UIDs in run_recipe at the returned revision. MF5 sends these colors as RGB24 without an Avatar rebuild. MF4/older still need matching catalogs. MF5可直接发送新颜色；旧版仍需材质目录打包。"
    });
    workspace.event("palette.defined", run, value.clone())?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use morphforge_core::{Box3, Operation};

    #[test]
    fn palette_reuses_exact_colors_preserves_geometry_and_roundtrips_history() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = Workspace::open(dir.path().to_path_buf()).unwrap();
        w.apply(
            &[Operation::Box {
                value: Box3::new([0, 0, 0], [2, 2, 2], 1),
            }],
            Some(0),
            None,
            None,
        )
        .unwrap();
        let geometry = serde_json::to_value(&w.project.scene.boxes).unwrap();
        let originals = serde_json::to_value(&w.project.materials).unwrap();
        let result = define(
            &mut w,
            &json!({"baseRevision":1,"colors":[
                {"name":"肤色","color":"#FFD6BF"},{"name":"同色","color":"#ffd6bf"},
                {"name":"藏蓝","color":"#172D59"}
            ]}),
            None,
        )
        .unwrap();
        assert_eq!(result["added"], 2);
        assert_eq!(result["palette"][0]["id"], result["palette"][1]["id"]);
        assert_eq!(
            serde_json::to_value(&w.project.scene.boxes).unwrap(),
            geometry
        );
        assert_eq!(
            serde_json::to_value(w.materials_at(1).unwrap()).unwrap(),
            originals
        );
        let reuse = define(
            &mut w,
            &json!({"baseRevision":2,"colors":[{"name":"skin","color":"#ffd6bf"}]}),
            None,
        )
        .unwrap();
        assert_eq!(reuse["revision"], 2);
        assert_eq!(reuse["added"], 0);
        drop(w);
        let w = Workspace::open(dir.path().to_path_buf()).unwrap();
        assert_eq!(w.project.materials.len(), 26);
        assert_eq!(
            w.materials_at(2).unwrap().last().unwrap().color,
            [23.0 / 255.0, 45.0 / 255.0, 89.0 / 255.0]
        );
    }

    #[test]
    fn invalid_palette_and_revision_conflicts_do_not_partially_mutate() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = Workspace::open(dir.path().to_path_buf()).unwrap();
        let original = serde_json::to_value(&w.project).unwrap();
        for args in [
            json!({"baseRevision":0,"colors":[{"name":"valid","color":"#123456"},{"name":"bad","color":"蓝色"}]}),
            json!({"baseRevision":9,"colors":[{"name":"valid","color":"#123456"}]}),
            json!({"baseRevision":0,"colors":[]}),
            json!({"baseRevision":0,"colors":[{"name":" ","color":"#123456"}]}),
        ] {
            assert!(define(&mut w, &args, None).is_err());
            assert_eq!(serde_json::to_value(&w.project).unwrap(), original);
        }
    }
}
