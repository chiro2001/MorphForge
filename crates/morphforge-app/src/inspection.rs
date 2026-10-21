//! Temporary capture geometry. Never writes the authored scene or OSC target.
use anyhow::{bail,Context,Result};
use morphforge_core::Scene;
use serde::Deserialize;
use serde_json::{json,Value};
#[derive(Default,Deserialize)]
#[serde(default,rename_all="camelCase",deny_unknown_fields)]
struct Inspection {
    include_groups:Vec<String>, hide_groups:Vec<String>, hide_bounds:Option<Bounds>, offsets:Vec<Offset>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Bounds {min:[i32;3],max:[i32;3]}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Offset {group:String,offset:[i32;3]}
pub fn apply(scene:&Scene,value:&Value)->Result<(Scene,Value)> {
    if value.is_null(){return Ok((scene.clone(),Value::Null))}
    let spec:Inspection=serde_json::from_value(value.clone()).context("inspection格式无效")?;
    if spec.include_groups.len()+spec.hide_groups.len()+spec.offsets.len()>256{bail!("inspection最多256个分组条目")}
    if let Some(b)=&spec.hide_bounds{if(0..3).any(|a|b.min[a]>=b.max[a]){bail!("hideBounds需要min<max")}}
    if spec.offsets.iter().any(|x|x.offset.iter().any(|v|v.unsigned_abs()>1_000_000)){bail!("inspection偏移过大")}
    let visible=|group:&str,min:[i32;3],max:[i32;3]| {
        (spec.include_groups.is_empty()||spec.include_groups.iter().any(|s|s==group))
        && !spec.hide_groups.iter().any(|s|s==group)
        && !spec.hide_bounds.as_ref().is_some_and(|b|(0..3).all(|a|min[a]<b.max[a]&&max[a]>b.min[a]))
    };
    let mut output=scene.clone();
    output.boxes.retain(|b|visible(&b.group,b.min,b.end()));
    output.triangles.retain(|t|{let b=t.bounds();visible(&t.group,b.min,b.max.map(|v|v.saturating_add(1)))});
    let offset=|group:&str|spec.offsets.iter().find(|x|x.group==group).map(|x|x.offset).unwrap_or([0;3]);
    for b in &mut output.boxes{let off=offset(&b.group);for a in 0..3{b.min[a]=b.min[a].checked_add(off[a]).context("inspection坐标溢出")?;}}
    for t in &mut output.triangles{let off=offset(&t.group);for p in &mut t.vertices{for a in 0..3{p[a]=p[a].checked_add(off[a]).context("inspection坐标溢出")?;}}}
    let report=json!({"inspectionOnly":true,"filter":value,"sourceBoxCount":scene.boxes.len(),"visibleBoxCount":output.boxes.len(),"sourceTriangleCount":scene.triangles.len(),"visibleTriangleCount":output.triangles.len(),"note":"临时观察副本；hideBounds隐藏与范围相交的整个对象，不裁切三角面；不修改作品，不作为完整交付截图。"});
    Ok((output,report))
}
#[cfg(test)]
mod tests {
 use super::*;
 use morphforge_core::Box3;
 #[test]fn temporary_group_view_preserves_authored_coordinates(){let mut s=Scene::default();let mut a=Box3::new([0,0,0],[2,2,2],1);a.group="inside".into();let mut b=Box3::new([4,0,0],[2,2,2],1);b.group="wall".into();s.boxes=vec![a,b];let before=serde_json::to_value(&s).unwrap();let(out,r)=apply(&s,&json!({"hideGroups":["wall"],"offsets":[{"group":"inside","offset":[0,3,0]}]})).unwrap();assert_eq!(out.boxes.len(),1);assert_eq!(out.boxes[0].min,[0,3,0]);assert_eq!(serde_json::to_value(&s).unwrap(),before);assert_eq!(r["inspectionOnly"],true);}
 #[test]fn invalid_ranges_and_fields_are_rejected(){let s=Scene::default();assert!(apply(&s,&json!({"hideBounds":{"min":[1,0,0],"max":[0,1,1]}})).is_err());assert!(apply(&s,&json!({"typo":true})).is_err());}
}
