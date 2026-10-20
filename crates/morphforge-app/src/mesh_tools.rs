use crate::store::Workspace;
use anyhow::{bail, Context, Result};
use morphforge_core::{Operation, Triangle3};
use serde_json::{json, Value};
pub fn execute(w: &mut Workspace, args: &Value, run: Option<&str>) -> Result<Value> {
    if args["projectId"]
        .as_str()
        .is_some_and(|id| id != w.project.id)
    {
        bail!("WORKSPACE_CONFLICT：工程已切换");
    }
    let action = args["action"].as_str().unwrap_or("inspect");
    if action == "inspect" {
        let offset = args["offset"].as_u64().unwrap_or(0).min(usize::MAX as u64) as usize;
        let limit = args["limit"].as_u64().unwrap_or(32).clamp(1, 128) as usize;
        let faces: Vec<_> = w
            .project
            .scene
            .triangles
            .iter()
            .filter(|t| args["group"].as_str().is_none_or(|g| g == t.group))
            .collect();
        return Ok(
            json!({"ok":true,"revision":w.project.scene.revision,"triangleCount":faces.len(),"faces":faces.iter().skip(offset).take(limit).collect::<Vec<_>>(),"nextOffset":if offset.saturating_add(limit)<faces.len(){Some(offset+limit)}else{None},"coordinates":"signed integer grid","color":"per-face RGB8","regionErase":"whole faces selected by AABB; exact deletion uses IDs"}),
        );
    }
    let base = args["baseRevision"]
        .as_u64()
        .context("网格修改需要baseRevision")?;
    if base != w.project.scene.revision {
        bail!("REVISION_CONFLICT：工程已改变");
    }
    let mut operations = Vec::new();
    match action {
        "apply" => {
            let mut faces: Vec<Triangle3> = serde_json::from_value(args["triangles"].clone())
                .context("triangles需要三角面数组")?;
            if faces.len() > w.project.limits.max_ops {
                bail!("三角面操作数超限");
            }
            match args["mode"].as_str().unwrap_or("append") {
                "append" => {}
                "replace_group" => operations.push(Operation::RemoveGroup {
                    name: args["group"]
                        .as_str()
                        .context("replace_group需要group")?
                        .into(),
                }),
                "replace_scene" => operations.push(Operation::Clear),
                _ => bail!("网格mode需要append/replace_group/replace_scene"),
            }
            for t in &mut faces {
                if let Some(group) = args["group"].as_str() {
                    t.group = group.into();
                }
                t.validate(&w.project.limits)?;
            }
            operations.extend(faces.into_iter().map(|value| Operation::Triangle { value }));
        }
        "wedge" => {
            let min = serde_json::from_value(args["min"].clone()).context("斜面min需要XYZ整数")?;
            let size =
                serde_json::from_value(args["size"].clone()).context("斜面size需要XYZ正整数")?;
            let color =
                serde_json::from_value(args["color"].clone()).context("斜面color需要RGB8数组")?;
            let mut faces = morphforge_core::triangles::wedge(
                min,
                size,
                args["direction"].as_str().unwrap_or("+x"),
                color,
                args["group"].as_str().unwrap_or("slope"),
            )?;
            for face in &mut faces {
                face.double_sided = args["doubleSided"] == true;
            }
            operations.extend(faces.into_iter().map(|value| Operation::Triangle { value }));
        }
        "remove" => operations.push(Operation::RemoveTriangles {
            ids: serde_json::from_value(args["ids"].clone())?,
        }),
        "paint" => operations.push(Operation::PaintTriangles {
            ids: serde_json::from_value(args["ids"].clone())?,
            color: serde_json::from_value(args["color"].clone())?,
        }),
        _ => bail!("未知mesh_tools操作"),
    }
    let preview = w.project.scene.apply(&operations, &w.project.limits)?;
    w.ensure_protected_groups_unchanged(&w.project.scene, &preview)?;
    if args["dryRun"] == true {
        return Ok(
            json!({"ok":true,"dryRun":true,"revision":base,"stats":preview.stats(&w.project.limits),"target":crate::scene_tools::target_report(&preview,w.project.limits.target_boxes)}),
        );
    }
    let result = w.apply(&operations, Some(base), args["requestId"].as_str(), run)?;
    w.event("mesh.edited",run,json!({"action":action,"operationCount":operations.len(),"triangleCount":w.project.scene.triangles.len()}))?;
    Ok(result)
}
