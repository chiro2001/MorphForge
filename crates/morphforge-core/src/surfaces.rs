//! Conservative interface removal for adjacent boxes of the same transparent medium.
//! Source volumes are never removed. A bit is set only when a whole face is covered.
use crate::{materials::Material, Box3, Scene};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
#[derive(Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceInfo {
    pub masks: BTreeMap<u64, u8>,
    pub hidden_faces: usize,
    pub fully_hidden_boxes: usize,
    pub checks: usize,
    pub truncated: bool,
}
#[derive(Clone, Copy)]
struct Face {
    id: usize,
    rect: [i32; 4],
}
type Key = (usize, i32, bool, u32, u8);
fn tiles(r: [i32; 4]) -> Option<Vec<(i32, i32)>> {
    let a = r[0].div_euclid(4);
    let b = (r[2] - 1).div_euclid(4);
    let c = r[1].div_euclid(4);
    let d = (r[3] - 1).div_euclid(4);
    if (b - a + 1) as i64 * (d - c + 1) as i64 > 256 {
        return None;
    }
    Some((a..=b).flat_map(|u| (c..=d).map(move |v| (u, v))).collect())
}
fn rect(b: &Box3, axis: usize) -> [i32; 4] {
    let u = (axis + 1) % 3;
    let v = (axis + 2) % 3;
    let end = b.end();
    [b.min[u], b.min[v], end[u], end[v]]
}
fn subtract(a: [i32; 4], b: [i32; 4]) -> Vec<[i32; 4]> {
    let l = a[0].max(b[0]);
    let t = a[1].max(b[1]);
    let r = a[2].min(b[2]);
    let d = a[3].min(b[3]);
    if l >= r || t >= d {
        return vec![a];
    }
    [
        [a[0], a[1], l, a[3]],
        [r, a[1], a[2], a[3]],
        [l, a[1], r, t],
        [l, d, r, a[3]],
    ]
    .into_iter()
    .filter(|x| x[0] < x[2] && x[1] < x[3])
    .collect()
}
pub fn transparent_interfaces(scene: &Scene, materials: &[Material]) -> SurfaceInfo {
    let medium: HashSet<_> = materials
        .iter()
        .filter(|m| m.render_mode == "transparent")
        .map(|m| m.id)
        .collect();
    let mut small: HashMap<(Key, i32, i32), Vec<Face>> = HashMap::new();
    let mut large: HashMap<Key, Vec<Face>> = HashMap::new();
    let mut all: HashMap<Key, Vec<Face>> = HashMap::new();
    for (id, b) in scene
        .boxes
        .iter()
        .enumerate()
        .filter(|(_, b)| medium.contains(&b.style_id))
    {
        for axis in 0..3 {
            for positive in [false, true] {
                let plane = if positive { b.end()[axis] } else { b.min[axis] };
                let key = (axis, plane, positive, b.style_id, b.tint_id);
                let face = Face {
                    id,
                    rect: rect(b, axis),
                };
                all.entry(key).or_default().push(face);
                if let Some(chunks) = tiles(face.rect) {
                    for (u, v) in chunks {
                        small.entry((key, u, v)).or_default().push(face);
                    }
                } else {
                    large.entry(key).or_default().push(face);
                }
            }
        }
    }
    let mut result = SurfaceInfo::default();
    for b in scene.boxes.iter().filter(|b| medium.contains(&b.style_id)) {
        let mut mask = 0;
        for axis in 0..3 {
            for positive in [true, false] {
                let plane = if positive { b.end()[axis] } else { b.min[axis] };
                let key = (axis, plane, !positive, b.style_id, b.tint_id);
                let face_rect = rect(b, axis);
                let mut remaining = vec![face_rect];
                let mut seen = HashSet::new();
                let candidates: Vec<&Face> = if let Some(chunks) = tiles(face_rect) {
                    chunks
                        .into_iter()
                        .flat_map(|(u, v)| small.get(&(key, u, v)).into_iter().flatten())
                        .chain(large.get(&key).into_iter().flatten())
                        .collect()
                } else {
                    all.get(&key).into_iter().flatten().collect()
                };
                for f in candidates {
                    if !seen.insert(f.id) {
                        continue;
                    }
                    result.checks += 1;
                    if result.checks > 4_000_000 || remaining.len() > 2048 {
                        result.truncated = true;
                        break;
                    }
                    remaining = remaining
                        .into_iter()
                        .flat_map(|r| subtract(r, f.rect))
                        .collect();
                    if remaining.is_empty() {
                        break;
                    }
                }
                if remaining.is_empty() {
                    mask |= 1 << (axis * 2 + usize::from(!positive));
                }
            }
        }
        if mask != 0 {
            result.masks.insert(b.id, mask);
            result.hidden_faces += mask.count_ones() as usize;
            if mask == 63 {
                result.fully_hidden_boxes += 1;
            }
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::materials::builtins;
    fn b(id: u64, p: [i32; 3], size: [u32; 3], style: u32) -> Box3 {
        let mut b = Box3::new(p, size, style);
        b.id = id;
        b
    }
    #[test]
    fn removes_only_touching_same_medium_and_handles_partial_union() {
        let styles = builtins();
        let water = styles.iter().find(|m| m.family == "water").unwrap().id;
        let scene = Scene {
            boxes: vec![
                b(1, [0, 0, 0], [2, 2, 2], water),
                b(2, [2, 0, 0], [1, 1, 2], water),
                b(3, [2, 1, 0], [1, 1, 2], water),
                b(4, [5, 0, 0], [1, 1, 1], water),
            ],
            ..Default::default()
        };
        let result = transparent_interfaces(&scene, &styles);
        assert_eq!(result.masks[&1] & 1, 1);
        assert_eq!(result.masks[&2] & 2, 2);
        assert!(!result.masks.contains_key(&4));
        let mut partial = scene.clone();
        partial.boxes.remove(2);
        assert_eq!(
            transparent_interfaces(&partial, &styles)
                .masks
                .get(&1)
                .copied()
                .unwrap_or(0)
                & 1,
            0
        );
        let mut tinted = scene;
        tinted.boxes[1].tint_id = 2;
        assert_eq!(
            transparent_interfaces(&tinted, &styles)
                .masks
                .get(&1)
                .copied()
                .unwrap_or(0)
                & 1,
            0
        );
    }
    #[test]
    fn opaque_and_separate_volumes_are_not_culled() {
        let styles = builtins();
        let scene = Scene {
            boxes: vec![b(1, [0, 0, 0], [1, 1, 1], 1), b(2, [1, 0, 0], [1, 1, 1], 1)],
            ..Default::default()
        };
        assert!(transparent_interfaces(&scene, &styles).masks.is_empty());
    }
}
