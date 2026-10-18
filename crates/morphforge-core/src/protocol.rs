//! MF3 is a command stream; all brush state is serialized on the same paced writer.
use crate::{Box3, GeometryError, Result, Scene};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Command {
    Box { value: Box3 },
    Voxel { value: Box3 },
    Erase { value: Box3 },
    Remove { value: Box3 },
    Clear,
    Brush { style: u32, tint: u8, motion: u8 },
}
pub fn key(b: &Box3) -> ([i32; 3], [u32; 3]) {
    (b.min, b.size)
}
pub fn appearance(b: &Box3) -> (u32, u8, u8) {
    (b.style_id, b.tint_id, b.motion_phase)
}
pub fn validate_box(b: &Box3) -> Result<()> {
    for a in 0..3 {
        let p = b.min[a] as i64 + if a == 1 { 0 } else { 128 };
        if !(0..=255).contains(&p) || b.size[a] < 1 || b.size[a] > 256 || p + b.size[a] as i64 > 256
        {
            return Err(GeometryError(
                "目标VRC只支持X/Z -128..127、Y 0..255的256³空间".into(),
            ));
        }
    }
    if b.style_id > 4095 || b.tint_id > 63 || b.motion_phase > 63 {
        return Err(GeometryError("目标VRC样式/染色/运动编码越界".into()));
    }
    Ok(())
}
pub fn validate_target(scene: &Scene, capacity: usize) -> Result<()> {
    if scene.boxes.len() > capacity {
        return Err(GeometryError(format!(
            "发布目标容量为{capacity} Box，草稿仍已保留"
        )));
    }
    for b in &scene.boxes {
        validate_box(b)?;
    }
    Ok(())
}
pub fn payload(c: &Command) -> Result<(u8, [u8; 6])> {
    let mut data = [0; 6];
    let (op, b) = match c {
        Command::Clear => return Ok((5, data)),
        Command::Brush {
            style,
            tint,
            motion,
        } => {
            if *style > 4095 || *tint > 63 || *motion > 63 {
                return Err(GeometryError("刷子编码越界".into()));
            }
            data[0] = (*style & 63) as u8;
            data[1] = (*style >> 6) as u8;
            data[2] = *tint;
            data[3] = *motion;
            return Ok((6, data));
        }
        Command::Box { value } => (1, value),
        Command::Voxel { value } => (2, value),
        Command::Erase { value } => (3, value),
        Command::Remove { value } => (4, value),
    };
    validate_box(b)?;
    for a in 0..3 {
        data[a] = (b.min[a] + if a == 1 { 0 } else { 128 }) as u8;
        data[a + 3] = if op == 2 { 0 } else { (b.size[a] - 1) as u8 };
    }
    Ok((op, data))
}
fn osc_str(s: &str) -> Vec<u8> {
    let mut b = s.as_bytes().to_vec();
    b.push(0);
    while !b.len().is_multiple_of(4) {
        b.push(0)
    }
    b
}
pub fn bundle(values: &[(String, OscValue)]) -> Vec<u8> {
    let mut out = b"#bundle\0".to_vec();
    out.extend(1u64.to_be_bytes());
    for (name, value) in values {
        let mut msg = osc_str(&format!("/avatar/parameters/{name}"));
        match value {
            OscValue::Int(v) => {
                msg.extend(osc_str(",i"));
                msg.extend(v.to_be_bytes())
            }
            OscValue::Float(v) => {
                msg.extend(osc_str(",f"));
                msg.extend(v.to_be_bytes())
            }
            OscValue::Bool(v) => msg.extend(osc_str(if *v { ",T" } else { ",F" })),
        };
        out.extend((msg.len() as u32).to_be_bytes());
        out.extend(msg);
    }
    out
}
#[derive(Clone, Debug)]
pub enum OscValue {
    Int(i32),
    Float(f32),
    Bool(bool),
}
pub fn packet(c: &Command, seq: u8, legacy: bool) -> Result<Vec<u8>> {
    if seq >= if legacy { 32 } else { 16 } {
        return Err(GeometryError("序号越界".into()));
    }
    let (op, data) = payload(c)?;
    if legacy {
        let old_op = match op {
            1 | 2 => 1,
            3 => 2,
            4 => 3,
            5 => 4,
            6 => return Err(GeometryError("MF2没有Brush命令".into())),
            _ => 0,
        };
        let mut values: Vec<_> = data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (format!("MF2_D{i:02}"), OscValue::Int(v as i32)))
            .collect();
        let meta = match c {
            Command::Box { value } | Command::Voxel { value } => [
                value.style_id & 63,
                value.style_id >> 6,
                value.tint_id as u32,
                value.motion_phase as u32,
            ],
            _ => [0; 4],
        };
        for (i, v) in meta.into_iter().enumerate() {
            values.push((format!("MF2_D{:02}", i + 6), OscValue::Int(v as i32)))
        }
        values.push((
            "MF2_Ctl".into(),
            OscValue::Int(((seq << 3) | old_op) as i32),
        ));
        return Ok(bundle(&values));
    }
    let mut values: Vec<_> = data
        .into_iter()
        .enumerate()
        .map(|(i, v)| (format!("MF3_D{i:02}"), OscValue::Int(v as i32)))
        .collect();
    values.push(("MF3_Ctl".into(), OscValue::Int(((seq << 4) | op) as i32)));
    Ok(bundle(&values))
}
pub fn plan(
    sent: &[Box3],
    target: &[Box3],
    brush: Option<(u32, u8, u8)>,
    legacy: bool,
) -> Vec<Command> {
    let old: BTreeMap<_, _> = sent.iter().map(|b| (key(b), b)).collect();
    let new: BTreeMap<_, _> = target.iter().map(|b| (key(b), b)).collect();
    let mut commands = vec![];
    for (k, b) in &old {
        if !new.contains_key(k) {
            commands.push(Command::Remove {
                value: (*b).clone(),
            })
        }
    }
    let mut changed: Vec<_> = new
        .iter()
        .filter(|(k, b)| old.get(k).is_none_or(|a| appearance(a) != appearance(b)))
        .map(|(_, b)| (*b).clone())
        .collect();
    // Spatial order inside each brush group; valid because the canonical target is disjoint.
    changed.sort_by_key(|b| (appearance(b), key(b)));
    let mut active = brush;
    for b in changed {
        let a = appearance(&b);
        if !legacy && active != Some(a) {
            commands.push(Command::Brush {
                style: a.0,
                tint: a.1,
                motion: a.2,
            });
            active = Some(a)
        }
        commands.push(if b.size == [1; 3] {
            Command::Voxel { value: b }
        } else {
            Command::Box { value: b }
        })
    }
    commands
}
pub fn used_styles(scene: &Scene) -> BTreeSet<u32> {
    scene.boxes.iter().map(|b| b.style_id).collect()
}
