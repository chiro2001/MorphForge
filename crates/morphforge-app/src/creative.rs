//! Project-scoped reusable assets, reference cameras and non-destructive motion.
use crate::store::Workspace;
use anyhow::{bail, Context, Result};
use morphforge_core::{Operation, Scene};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Creative {
    pub revision: u64,
    pub bookmarks: BTreeMap<String, Bookmark>,
    pub components: BTreeMap<String, Component>,
    pub clips: BTreeMap<String, Clip>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bookmark {
    pub name: String,
    pub shot: Value,
    #[serde(default)]
    pub comparison: Option<Comparison>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Comparison {
    pub image_id: String,
    pub mode: String,
    pub opacity: f64,
    pub zoom: f64,
    pub offset: [f64; 2],
    /// Normalized image coordinates x,y,width,height.
    pub crop: [f64; 4],
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Component {
    pub name: String,
    pub description: String,
    pub source_revision: u64,
    pub anchor: [i32; 3],
    pub scene: Scene,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Clip {
    pub name: String,
    pub duration: f64,
    #[serde(default)]
    pub looping: bool,
    pub tracks: Vec<Track>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Track {
    pub group: String,
    pub pivot: [f64; 3],
    pub interpolation: String,
    pub keys: Vec<Keyframe>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Keyframe {
    pub time: f64,
    pub translation: [f64; 3],
    /// Euler XYZ degrees; interpolation uses shortest-path quaternion slerp.
    pub rotation: [f64; 3],
    pub scale: [f64; 3],
}
fn name(value: &str) -> Result<()> {
    if value.trim().is_empty() || value.len() > 128 {
        bail!("名称须为1..128字节")
    }
    Ok(())
}
fn point(value: &[f64; 3], bound: f64) -> bool {
    value.iter().all(|v| v.is_finite() && v.abs() <= bound)
}
pub fn validate_clip(clip: &Clip) -> Result<()> {
    name(&clip.name)?;
    if !clip.duration.is_finite()
        || !(0.05..=3600.).contains(&clip.duration)
        || clip.tracks.is_empty()
        || clip.tracks.len() > 64
    {
        bail!("动画时长0.05..3600秒，需要1..64条分组轨道")
    }
    let total_keys: usize = clip.tracks.iter().map(|t| t.keys.len()).sum();
    if total_keys > 4096 {
        bail!("单片段最多4096关键帧")
    }
    let mut groups = BTreeSet::new();
    for track in &clip.tracks {
        name(&track.group)?;
        if !groups.insert(&track.group)
            || !point(&track.pivot, 1_000_000.)
            || !["linear", "step"].contains(&track.interpolation.as_str())
            || track.keys.is_empty()
            || track.keys.len() > 256
        {
            bail!("轨道分组不可重复；枢轴/插值/关键帧无效")
        }
        let mut previous = -1.;
        for key in &track.keys {
            if !key.time.is_finite()
                || key.time < 0.
                || key.time > clip.duration
                || key.time <= previous
                || (previous >= 0. && key.time as f32 <= previous as f32)
                || !point(&key.translation, 1_000_000.)
                || !point(&key.rotation, 36000.)
                || !key
                    .scale
                    .iter()
                    .all(|v| v.is_finite() && (0.001..=1000.).contains(v))
            {
                bail!("关键帧时间须严格递增且位于时长内，缩放0.001..1000；变换须为有限数")
            }
            previous = key.time;
        }
    }
    Ok(())
}
pub fn validate_comparison(c: &Comparison) -> Result<()> {
    if c.image_id.len() > 128
        || !["split", "overlay"].contains(&c.mode.as_str())
        || !c.opacity.is_finite()
        || !(0.0..=1.).contains(&c.opacity)
        || !c.zoom.is_finite()
        || !(0.1..=10.).contains(&c.zoom)
        || !c.offset.iter().all(|v| v.is_finite() && v.abs() <= 2.)
        || !c
            .crop
            .iter()
            .all(|v| v.is_finite() && (0.0..=1.).contains(v))
        || c.crop[2] <= 0.
        || c.crop[3] <= 0.
        || c.crop[0] + c.crop[2] > 1.000001
        || c.crop[1] + c.crop[3] > 1.000001
    {
        bail!("参考图对照参数无效：裁剪为归一化x,y,w,h，且须位于图片内")
    }
    Ok(())
}
impl Creative {
    pub fn validate(&self, limits: &morphforge_core::Limits) -> Result<()> {
        if self.bookmarks.len() > 64 || self.components.len() > 128 || self.clips.len() > 32 {
            bail!("创作资产超过数量上限")
        }
        let mut objects = 0;
        for (id, b) in &self.bookmarks {
            name(id)?;
            name(&b.name)?;
            crate::capture::view_options(&json!({"shots":[b.shot]}))?;
            if let Some(c) = &b.comparison {
                validate_comparison(c)?;
            }
        }
        for (id, c) in &self.components {
            name(id)?;
            name(&c.name)?;
            if c.description.len() > 4096 {
                bail!("组件说明过长")
            }
            c.scene.validate(limits)?;
            objects += c.scene.boxes.len() + c.scene.triangles.len();
        }
        if objects > 65536 {
            bail!("项目组件库最多保存65536个几何对象")
        }
        for (id, clip) in &self.clips {
            name(id)?;
            validate_clip(clip)?;
        }
        Ok(())
    }
    pub fn brief(&self) -> Value {
        json!({"revision":self.revision,"bookmarks":self.bookmarks.iter().map(|(id,b)|json!({"id":id,"name":b.name,"imageId":b.comparison.as_ref().map(|c|&c.image_id)})).collect::<Vec<_>>(),"components":self.components.iter().map(|(id,c)|json!({"id":id,"name":c.name,"boxCount":c.scene.boxes.len(),"triangleCount":c.scene.triangles.len()})).collect::<Vec<_>>(),"clips":self.clips.iter().map(|(id,c)|json!({"id":id,"name":c.name,"duration":c.duration,"groups":c.tracks.iter().map(|t|&t.group).collect::<Vec<_>>()})).collect::<Vec<_>>()})
    }
    pub fn summary(&self) -> Value {
        json!({"revision":self.revision,"bookmarks":self.bookmarks,"components":self.components.iter().map(|(id,c)|json!({"id":id,"name":c.name,"description":c.description,"boxCount":c.scene.boxes.len(),"triangleCount":c.scene.triangles.len(),"sourceRevision":c.source_revision,"anchor":c.anchor})).collect::<Vec<_>>(),"clips":self.clips,"animationTarget":"editor-and-glb; explicit Avatar publish requires mf6-motion4/1 (3 tracks, 4 keys)"})
    }
}

fn save(
    w: &mut Workspace,
    mut next: Creative,
    args: &Value,
    run: Option<&str>,
    kind: &str,
) -> Result<Value> {
    if args["baseCreativeRevision"].as_u64() != Some(w.project.creative.revision) {
        bail!("REVISION_CONFLICT：创作资产已改变，请重新读取")
    }
    next.validate(&w.project.limits)?;
    next.revision = next.revision.checked_add(1).context("创作资产版本溢出")?;
    let old = std::mem::replace(&mut w.project.creative, next);
    if let Err(e) = w.persist() {
        w.project.creative = old;
        return Err(e);
    }
    let result = w.project.creative.summary();
    w.event(kind,run,json!({"action":args["action"],"id":args["id"],"creativeRevision":w.project.creative.revision}))?;
    Ok(result)
}
pub fn action(w: &mut Workspace, domain: &str, args: &Value, run: Option<&str>) -> Result<Value> {
    if let Some(id) = args["projectId"].as_str() {
        if id != w.project.id {
            bail!("WORKSPACE_CONFLICT")
        }
    }
    let act = args["action"].as_str().unwrap_or("list");
    if act == "list" {
        return Ok(w.project.creative.summary());
    }
    if domain == "components" && act == "place" {
        return place(w, args, run);
    }
    let id = args["id"].as_str().context("需要资产id")?;
    name(id)?;
    if act == "inspect" {
        return match domain {
            "components" => Ok(
                json!({"component":w.project.creative.components.get(id).context("组件不存在")?,"creativeRevision":w.project.creative.revision}),
            ),
            _ => bail!("不支持的查看操作"),
        };
    }
    let mut next = w.project.creative.clone();
    match (domain, act) {
        ("references", "save") => {
            let mut bookmark: Bookmark = serde_json::from_value(args["bookmark"].clone())?;
            bookmark.shot["id"] = json!(id);
            if let Some(c) = &bookmark.comparison {
                crate::references::record(w, &c.image_id)?;
            }
            next.bookmarks.insert(id.into(), bookmark);
        }
        ("references", "remove") => {
            if next.bookmarks.remove(id).is_none() {
                bail!("相机书签不存在")
            }
        }
        ("animation", "save") => {
            let clip: Clip = serde_json::from_value(args["clip"].clone())?;
            validate_clip(&clip)?;
            let groups = w.project.scene.stats(&w.project.limits).groups;
            for t in &clip.tracks {
                if !groups.contains_key(&t.group) {
                    bail!("动画目标分组不存在：{}", t.group)
                }
            }
            next.clips.insert(id.into(), clip);
        }
        ("animation", "remove") => {
            if next.clips.remove(id).is_none() {
                bail!("动画不存在")
            }
        }
        ("components", "remove") => {
            if next.components.remove(id).is_none() {
                bail!("组件不存在")
            }
        }
        ("components", "save") => {
            if args["baseRevision"].as_u64() != Some(w.project.scene.revision) {
                bail!("REVISION_CONFLICT")
            }
            let ids: Vec<u64> =
                serde_json::from_value(args.get("ids").cloned().unwrap_or(json!([])))?;
            let groups: Vec<String> =
                serde_json::from_value(args.get("groups").cloned().unwrap_or(json!([])))?;
            if ids.is_empty() && groups.is_empty() {
                bail!("保存组件需要选区ids或groups")
            }
            let ids: BTreeSet<_> = ids.into_iter().collect();
            let groups: BTreeSet<_> = groups.into_iter().collect();
            let mut scene = w.project.scene.clone();
            scene.revision = 0;
            scene
                .boxes
                .retain(|b| ids.contains(&b.id) || groups.contains(&b.group));
            scene
                .triangles
                .retain(|t| ids.contains(&t.id) || groups.contains(&t.group));
            let bounds = scene
                .stats(&w.project.limits)
                .bounds
                .context("组件选区为空")?;
            let anchor: [i32; 3] = if let Some(v) = args.get("anchor") {
                serde_json::from_value(v.clone())?
            } else {
                [
                    (bounds.min[0] + bounds.max[0]).div_euclid(2),
                    bounds.min[1],
                    (bounds.min[2] + bounds.max[2]).div_euclid(2),
                ]
            };
            for b in &mut scene.boxes {
                for a in 0..3 {
                    b.min[a] = b.min[a].checked_sub(anchor[a]).context("组件原点溢出")?
                }
            }
            for t in &mut scene.triangles {
                for p in &mut t.vertices {
                    for a in 0..3 {
                        p[a] = p[a].checked_sub(anchor[a]).context("组件原点溢出")?
                    }
                }
            }
            next.components.insert(
                id.into(),
                Component {
                    name: args["name"].as_str().unwrap_or(id).into(),
                    description: args["description"].as_str().unwrap_or("").into(),
                    anchor,
                    source_revision: w.project.scene.revision,
                    scene,
                },
            );
        }
        _ => bail!("不支持的创作资产操作"),
    }
    save(w, next, args, run, &format!("creative.{domain}"))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Instance {
    pub offset: [i32; 3],
    #[serde(default)]
    pub turns: i32,
    #[serde(default)]
    pub mirror: Option<String>,
    #[serde(default = "unit")]
    pub scale: i32,
    pub group: String,
    #[serde(default)]
    pub style_id: Option<u32>,
}
fn unit() -> i32 {
    1
}
fn place(w: &mut Workspace, args: &Value, run: Option<&str>) -> Result<Value> {
    let request = args["requestId"]
        .as_str()
        .context("组件放置需要requestId")?;
    if request.is_empty() || request.len() > 256 {
        bail!("requestId须为1..256字节")
    }
    let fingerprint = crate::store::hash(&serde_json::to_vec(
        &json!({"tool":"components","args":args}),
    )?);
    if let Some(old) = w.project.import_receipts.get(request) {
        if old["fingerprint"] != fingerprint {
            bail!("REQUEST_ID_CONFLICT")
        }
        let mut v = old["result"].clone();
        v["replayed"] = json!(true);
        return Ok(v);
    }
    if args["baseCreativeRevision"]
        .as_u64()
        .is_some_and(|r| r != w.project.creative.revision)
    {
        bail!("REVISION_CONFLICT：组件库已改变")
    }
    if args["baseRevision"].as_u64() != Some(w.project.scene.revision) {
        bail!("REVISION_CONFLICT")
    }
    let c = w
        .project
        .creative
        .components
        .get(args["id"].as_str().context("需要组件id")?)
        .context("组件不存在")?;
    let instances: Vec<Instance> = serde_json::from_value(args["instances"].clone())?;
    if instances.is_empty() || instances.len() > 128 {
        bail!("每次放置1..128个组件实例")
    }
    let mut operations = vec![];
    for inst in instances {
        name(&inst.group)?;
        if !(1..=64).contains(&inst.scale) {
            bail!("组件整数缩放1..64")
        }
        if let Some(style) = inst.style_id {
            if !w.project.materials.iter().any(|m| m.id == style) {
                bail!("样式不存在")
            }
        }
        let mut scene = c.scene.clone();
        for b in &mut scene.boxes {
            b.group = inst.group.clone();
            if let Some(style) = inst.style_id {
                b.style_id = style
            }
            for a in 0..3 {
                b.min[a] = b.min[a].checked_mul(inst.scale).context("缩放溢出")?;
                b.size[a] = b.size[a]
                    .checked_mul(inst.scale as u32)
                    .context("缩放溢出")?
            }
        }
        for t in &mut scene.triangles {
            t.group = inst.group.clone();
            for p in &mut t.vertices {
                for v in p {
                    *v = v.checked_mul(inst.scale).context("缩放溢出")?
                }
            }
        }
        scene.validate(&w.project.limits)?;
        let ids = scene
            .boxes
            .iter()
            .map(|b| b.id)
            .chain(scene.triangles.iter().map(|t| t.id))
            .collect();
        let transformed = scene.apply(
            &[Operation::Transform {
                ids,
                offset: inst.offset,
                turns: inst.turns,
                mirror: inst.mirror,
                copy: false,
            }],
            &w.project.limits,
        )?;
        operations.extend(transformed.boxes.into_iter().map(|mut value| {
            value.id = 0;
            Operation::Box { value }
        }));
        operations.extend(transformed.triangles.into_iter().map(|mut value| {
            value.id = 0;
            Operation::Triangle { value }
        }));
        if operations.len() > w.project.limits.max_ops {
            bail!("组件展开超出操作上限")
        }
    }
    let preview = w.project.scene.apply(&operations, &w.project.limits)?;
    w.ensure_protected_groups_unchanged(&w.project.scene, &preview)?;
    if args["dryRun"] == true {
        return Ok(
            json!({"dryRun":true,"stats":preview.stats(&w.project.limits),"operations":operations.len()}),
        );
    }
    let old = w.project.import_receipts.clone();
    let expected =
        json!({"ok":true,"revision":preview.revision,"stats":preview.stats(&w.project.limits)});
    w.project.import_receipts.insert(
        request.into(),
        json!({"fingerprint":fingerprint,"result":expected}),
    );
    match w.apply(
        &operations,
        args["baseRevision"].as_u64(),
        Some(request),
        run,
    ) {
        Ok(v) => Ok(v),
        Err(e) => {
            w.project.import_receipts = old;
            Err(e)
        }
    }
}

/// Add named cameras, comparison images and sampled motion to a capture job.
pub fn resolve_capture(w: &Workspace, args: &Value) -> Result<Value> {
    let mut out = args.clone();
    if let Some(id) = args["bookmarkId"].as_str() {
        let b = w
            .project
            .creative
            .bookmarks
            .get(id)
            .context("相机书签不存在")?;
        out["views"] = json!([]);
        out["shots"] = json!([b.shot]);
        if args["compare"] == true {
            out["comparison"] = serde_json::to_value(&b.comparison)?;
        }
    }
    if let Some(c) = out.get("comparison").filter(|v| !v.is_null()) {
        let c: Comparison = serde_json::from_value(c.clone())?;
        validate_comparison(&c)?;
        crate::references::record(w, &c.image_id)?;
    }
    if let Some(a) = args.get("animation") {
        let id = a["clipId"].as_str().context("动画截图需要clipId")?;
        let clip = w.project.creative.clips.get(id).context("动画不存在")?;
        validate_clip(clip)?;
        let time = a["time"].as_f64().context("动画截图需要time秒")?;
        if !time.is_finite() || time < 0. || time > clip.duration {
            bail!("动画截图时间超出时长")
        }
        out["animationClip"] = serde_json::to_value(clip)?;
        out["animationTime"] = json!(time);
    }
    Ok(out)
}

/// Canonical XYZ Euler quaternion reflected into glTF/Three coordinates.
pub fn quaternion(rotation: [f64; 3]) -> [f64; 4] {
    let [x, y, z] = rotation.map(|v| v.to_radians() / 2.);
    let (s1, c1) = x.sin_cos();
    let (s2, c2) = y.sin_cos();
    let (s3, c3) = z.sin_cos();
    [
        -(s1 * c2 * c3 + c1 * s2 * s3),
        -(c1 * s2 * c3 - s1 * c2 * s3),
        c1 * c2 * s3 + s1 * s2 * c3,
        c1 * c2 * c3 - s1 * s2 * s3,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use morphforge_core::{Box3, Triangle3};
    fn setup() -> (tempfile::TempDir, Workspace) {
        let d = tempfile::tempdir().unwrap();
        let mut w = Workspace::open(d.path().into()).unwrap();
        let mut b = Box3::new([10, 2, 20], [4, 6, 2], 2);
        b.group = "window".into();
        let t = Triangle3 {
            id: 0,
            vertices: [[10, 8, 20], [14, 8, 20], [12, 10, 20]],
            color: [50, 90, 150],
            group: "window".into(),
            double_sided: true,
        };
        w.apply(
            &[
                Operation::Box { value: b },
                Operation::Triangle { value: t },
            ],
            Some(0),
            None,
            None,
        )
        .unwrap();
        (d, w)
    }
    fn capture(w: &mut Workspace) {
        action(w,"components",&json!({"action":"save","id":"window","name":"窗","groups":["window"],"baseRevision":w.project.scene.revision,"baseCreativeRevision":w.project.creative.revision}),None).unwrap();
    }
    #[test]
    fn component_origin_mixed_geometry_and_roundtrip() {
        let (d, mut w) = setup();
        capture(&mut w);
        let c = &w.project.creative.components["window"];
        assert_eq!(c.anchor, [12, 2, 21]);
        assert_eq!(c.scene.boxes[0].min, [-2, 0, -1]);
        assert_eq!(c.scene.triangles[0].vertices[2], [0, 8, -1]);
        let w = Workspace::open(d.path().into()).unwrap();
        assert_eq!(w.project.creative.components.len(), 1);
    }
    #[test]
    fn placement_preview_apply_retry_undo_and_protection() {
        let (_d, mut w) = setup();
        capture(&mut w);
        let base = w.project.scene.revision;
        let mut args = json!({"action":"place","id":"window","baseRevision":base,"requestId":"place-1","instances":[{"offset":[-20,0,0],"turns":1,"mirror":"x","scale":2,"group":"copy"}],"dryRun":true});
        action(&mut w, "components", &args, None).unwrap();
        assert_eq!(w.project.scene.revision, base);
        args["dryRun"] = json!(false);
        let result = action(&mut w, "components", &args, None).unwrap();
        assert_eq!(w.project.scene.triangles.len(), 2);
        assert_eq!(w.project.scene.boxes.len(), 2);
        assert_eq!(
            action(&mut w, "components", &args, None).unwrap()["revision"],
            result["revision"]
        );
        let mut conflict = args.clone();
        conflict["instances"][0]["offset"] = json!([-19, 0, 0]);
        assert!(action(&mut w, "components", &conflict, None)
            .unwrap_err()
            .to_string()
            .contains("REQUEST_ID_CONFLICT"));
        w.undo(false, None).unwrap();
        assert_eq!(w.project.scene.boxes.len(), 1);
        w.set_protected_groups(vec!["window".into()]).unwrap();
        args["requestId"] = json!("protected");
        args["baseRevision"] = json!(w.project.scene.revision);
        args["instances"][0]["offset"] = json!([12, 2, 21]);
        args["instances"][0]["scale"] = json!(1);
        args["instances"][0]["turns"] = json!(0);
        args["instances"][0]
            .as_object_mut()
            .unwrap()
            .remove("mirror");
        assert!(action(&mut w, "components", &args, None)
            .unwrap_err()
            .to_string()
            .contains("PROTECTED_GROUP"));
    }
    #[test]
    fn rejects_bad_metadata_without_mutating_scene_or_library() {
        let (_d, mut w) = setup();
        capture(&mut w);
        let before = serde_json::to_value(&w.project.creative).unwrap();
        assert!(action(
            &mut w,
            "components",
            &json!({"action":"remove","id":"window","baseCreativeRevision":0}),
            None
        )
        .is_err());
        assert_eq!(before, serde_json::to_value(&w.project.creative).unwrap());
        let revision = w.project.scene.revision;
        assert!(action(&mut w,"components",&json!({"action":"place","id":"window","requestId":"x","baseRevision":revision,"instances":[{"offset":[0,0,0],"scale":0,"group":"bad"}]}),None).is_err());
    }
    pub fn clip() -> Clip {
        serde_json::from_value(json!({"name":"开窗","duration":2,"looping":true,"tracks":[{"group":"window","pivot":[10,2,20],"interpolation":"linear","keys":[{"time":0,"translation":[0,0,0],"rotation":[0,0,0],"scale":[1,1,1]},{"time":2,"translation":[0,4,0],"rotation":[0,90,0],"scale":[1,2,1]}]}]})).unwrap()
    }
    #[test]
    fn animation_validation_and_persistence_leave_geometry_intact() {
        let (_d, mut w) = setup();
        let original = serde_json::to_value(&w.project.scene).unwrap();
        action(
            &mut w,
            "animation",
            &json!({"action":"save","id":"open","clip":clip(),"baseCreativeRevision":0}),
            None,
        )
        .unwrap();
        assert_eq!(original, serde_json::to_value(&w.project.scene).unwrap());
        let mut c = clip();
        c.tracks.push(c.tracks[0].clone());
        assert!(validate_clip(&c).is_err());
        let mut c = clip();
        c.tracks[0].keys[1].scale[0] = 0.;
        assert!(validate_clip(&c).is_err());
        let c = resolve_capture(&w, &json!({"animation":{"clipId":"open","time":1.}})).unwrap();
        assert_eq!(c["animationTime"], 1.);
        assert!(resolve_capture(&w, &json!({"animation":{"clipId":"open","time":3.}})).is_err());
    }
    #[test]
    fn animated_glb_contains_group_pivot_channels_and_valid_binary_ranges() {
        let (d, w) = setup();
        let bytes = crate::animation_export::export(
            &w.project.scene,
            &w.project.materials,
            d.path(),
            &clip(),
        )
        .unwrap();
        assert_eq!(
            u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize,
            bytes.len()
        );
        let len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let doc: Value = serde_json::from_slice(&bytes[20..20 + len]).unwrap();
        assert_eq!(
            doc["animations"][0]["channels"].as_array().unwrap().len(),
            3
        );
        let nodes = doc["nodes"].as_array().unwrap();
        let pivot = nodes.last().unwrap();
        assert_eq!(pivot["translation"], json!([2.5, 0.5, -5.]));
        assert_eq!(pivot["children"].as_array().unwrap().len(), 2);
        for view in doc["bufferViews"].as_array().unwrap() {
            assert!(
                view["byteOffset"].as_u64().unwrap() + view["byteLength"].as_u64().unwrap()
                    <= doc["buffers"][0]["byteLength"].as_u64().unwrap()
            );
        }
        let q = quaternion([0., 90., 0.]);
        assert!((q[1] + 2f64.sqrt() / 2.).abs() < 1e-12);
    }
    #[test]
    fn comparison_crop_and_bookmark_validation() {
        let (_d, mut w) = setup();
        let image = crate::references::list(&w).unwrap()["images"][0]["id"].clone();
        let c = json!({"imageId":image,"mode":"overlay","opacity":0.5,"zoom":1,"offset":[0,0],"crop":[0,0,1,1]});
        action(&mut w,"references",&json!({"action":"save","id":"front","baseCreativeRevision":0,"bookmark":{"name":"正面","shot":{"projection":"orthographic","yaw":0,"pitch":0},"comparison":c}}),None).unwrap();
        let r = resolve_capture(&w, &json!({"bookmarkId":"front","compare":true})).unwrap();
        assert_eq!(r["shots"][0]["id"], "front");
        assert_eq!(r["comparison"]["mode"], "overlay");
        let mut c: Comparison = serde_json::from_value(c).unwrap();
        c.crop = [0.5, 0., 1., 1.];
        assert!(validate_comparison(&c).is_err());
    }
}
