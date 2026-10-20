use crate::store::Workspace;
use anyhow::{bail, Context, Result};
use morphforge_core::{Operation, Scene};
use serde_json::{json, Value};
pub fn target_report(scene: &Scene, capacity: usize) -> Value {
    let bounds = scene.stats(&Default::default()).bounds;
    let offset = bounds
        .as_ref()
        .map(|b| {
            [
                -(b.min[0] + b.max[0]).div_euclid(2),
                -b.min[1],
                -(b.min[2] + b.max[2]).div_euclid(2),
            ]
        })
        .unwrap_or([0; 3]);
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
    json!({"bounds":bounds,"extent":extent,"targetMin":[-128,0,-128],"targetMax":[128,256,128],"centerOffset":offset,"canCenterToFit":extent.iter().all(|n|*n<=256),"outOfBoundsCount":invalid.len(),"outOfBoundsGroups":invalid.iter().map(|b|b.group.as_str()).collect::<std::collections::BTreeSet<_>>(),"boxCount":scene.boxes.len(),"capacity":capacity,"invalidStyleIds":invalid_styles,"fits":invalid.is_empty()&&invalid_styles.is_empty()&&scene.boxes.len()<=capacity,"coordinateSpace":"signed integer voxel grid; max bounds exclusive","note":"X/Z允许负数；显示缩放不会扩大256³编码范围。"})
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
    let ids = w.project.scene.boxes.iter().map(|b| b.id).collect();
    let before = target_report(&w.project.scene, w.project.limits.target_boxes);
    if offset == [0; 3] || w.project.scene.boxes.is_empty() {
        return Ok(
            json!({"ok":true,"unchanged":true,"dryRun":args["dryRun"]==true,"offset":offset,"revision":base,"before":before,"after":before}),
        );
    }
    let ops = [Operation::Transform {
        ids,
        offset,
        turns: 0,
        mirror: None,
        copy: false,
    }];
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
