use crate::store::Workspace;
use anyhow::{bail, Context, Result};
use morphforge_core::{Operation, Scene};
use serde_json::{json, Value};
pub fn target_report(scene: &Scene, capacity: usize) -> Value {
    let bounds = scene.stats(&Default::default()).bounds;
    let mut offset = bounds
        .as_ref()
        .map(|b| {
            [
                -(b.min[0] + b.max[0]).div_euclid(2),
                -b.min[1],
                -(b.min[2] + b.max[2]).div_euclid(2),
            ]
        })
        .unwrap_or([0; 3]);
    let mut lower = [i64::MIN / 4; 3];
    let mut upper = [i64::MAX / 4; 3];
    let target_min = [-128, 0, -128];
    for b in &scene.boxes {
        let end = b.end();
        for a in 0..3 {
            lower[a] = lower[a].max(target_min[a] - b.min[a] as i64);
            upper[a] = upper[a].min(if a == 1 { 256 } else { 128 } - end[a] as i64);
        }
    }
    for t in &scene.triangles {
        let b = t.bounds();
        for a in 0..3 {
            lower[a] = lower[a].max(target_min[a] - b.min[a] as i64);
            upper[a] = upper[a].min(if a == 1 { 255 } else { 127 } - b.max[a] as i64);
        }
    }
    let can_center = (0..3).all(|a| lower[a] <= upper[a]);
    if can_center {
        for a in 0..3 {
            offset[a] = (offset[a] as i64).clamp(lower[a], upper[a]) as i32;
        }
    }
    for p in &scene.text_panels {let b=p.bounds();for a in 0..3 {lower[a]=lower[a].max(target_min[a]-b.min[a] as i64);upper[a]=upper[a].min(if a==1{256}else{128}-b.max[a] as i64);}}
    let can_center=(0..3).all(|a|lower[a]<=upper[a]);
    if can_center {for a in 0..3{offset[a]=(offset[a] as i64).clamp(lower[a],upper[a])as i32;}}
    let extent = bounds
        .as_ref()
        .map(|b| std::array::from_fn::<_, 3, _>(|a| b.max[a] - b.min[a]))
        .unwrap_or([0; 3]);
    let invalid: Vec<_> = scene
        .boxes
        .iter()
        .filter(|b| {
            let e = b.end();
            b.min[0] < -128
                || e[0] > 128
                || b.min[1] < 0
                || e[1] > 256
                || b.min[2] < -128
                || e[2] > 128
        })
        .collect();
    let invalid_styles: std::collections::BTreeSet<_> = scene
        .boxes
        .iter()
        .filter(|b| b.style_id > 65535)
        .map(|b| b.style_id)
        .collect();
    let invalid_triangles: Vec<_> = scene
        .triangles
        .iter()
        .filter(|t| t.wire_vertices().is_err())
        .collect();
    let invalid_text=scene.text_panels.iter().filter(|p|{let b=p.bounds();(0..3).any(|a|b.min[a]<(if a==1{0}else{-128})||b.max[a]>(if a==1{256}else{128}))}).collect::<Vec<_>>();
    let text_target=crate::text_panels::target_report(scene);
    let object_count = scene.boxes.len() + scene.triangles.len();
    let groups: std::collections::BTreeSet<_> = invalid
        .iter()
        .map(|b| b.group.as_str())
        .chain(invalid_triangles.iter().map(|t| t.group.as_str()))
        .collect();
    json!({"bounds":bounds,"extent":extent,"targetMin":[-128,0,-128],"targetMax":[128,256,128],"triangleVertexMax":[127,255,127],"centerOffset":offset,"canCenterToFit":can_center,"outOfBoundsCount":invalid.len()+invalid_triangles.len()+invalid_text.len(),"outOfBoundsTriangleCount":invalid_triangles.len(),"outOfBoundsGroups":groups,"boxCount":scene.boxes.len(),"triangleCount":scene.triangles.len(),"objectCount":object_count,"capacity":capacity,"invalidStyleIds":invalid_styles,"textPanels":text_target,"fits":invalid.is_empty()&&invalid_triangles.is_empty()&&invalid_text.is_empty()&&text_target["fits"]==true&&invalid_styles.is_empty()&&object_count<=capacity,"coordinateSpace":"signed integer grid; Box max bounds exclusive, triangle vertex max inclusive","note":"X/Z允许负数；显示缩放不会扩大256³编码范围。"})
}
pub fn transform(w: &mut Workspace, args: &Value, run: Option<&str>) -> Result<Value> {
    let base = args["baseRevision"]
        .as_u64()
        .context("整体定位需要baseRevision")?;
    if base != w.project.scene.revision {
        bail!("REVISION_CONFLICT：工程已改变，请重新预览");
    }
    let action = args["action"].as_str().unwrap_or("center_on_ground");
    let offset: [i32; 3] = match action {
        "center_on_ground" => serde_json::from_value(
            target_report(&w.project.scene, w.project.limits.target_boxes)["centerOffset"].clone(),
        )?,
        "translate" => {
            serde_json::from_value(args["offset"].clone()).context("offset需要三个整数")?
        }
        _ => bail!("整体定位仅支持translate/center_on_ground"),
    };
    let ids = w
        .project
        .scene
        .boxes
        .iter()
        .map(|b| b.id)
        .chain(w.project.scene.triangles.iter().map(|t| t.id))
        .collect();
    let before = target_report(&w.project.scene, w.project.limits.target_boxes);
    if offset == [0; 3]
        || (w.project.scene.boxes.is_empty() && w.project.scene.triangles.is_empty() && w.project.scene.text_panels.is_empty())
    {
        return Ok(
            json!({"ok":true,"unchanged":true,"dryRun":args["dryRun"]==true,"offset":offset,"revision":base,"before":before,"after":before}),
        );
    }
    let mut ops = vec![Operation::Transform {
        ids,
        offset,
        turns: 0,
        mirror: None,
        copy: false,
    }];
    for p in &w.project.scene.text_panels{let mut p=p.clone();for a in 0..3{p.position[a]+=offset[a]as f64;}ops.push(Operation::TextPanel{value:p});}
    let preview = w.project.scene.apply(&ops, &w.project.limits)?;
    w.ensure_protected_groups_unchanged(&w.project.scene, &preview)?;
    let after = target_report(&preview, w.project.limits.target_boxes);
    if args["dryRun"] == true {
        return Ok(
            json!({"ok":true,"dryRun":true,"offset":offset,"revision":base,"before":before,"after":after}),
        );
    }
    let applied = w.apply(&ops, Some(base), args["requestId"].as_str(), run)?;
    w.event("scene.transformed",run,json!({"action":action,"offset":offset,"before":before,"after":after,"sourceFilesChanged":false}))?;
    Ok(
        json!({"ok":true,"revision":applied["revision"],"offset":offset,"before":before,"after":after,"sourceFilesChanged":false,"note":"几何已移动，源码未改写；重新从源码replace会使用源码原坐标。可撤销。"}),
    )
}
