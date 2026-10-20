//! Lossless run/rectangular fill candidate. Surface equality uses the final
//! wire bytes, not merely logical material IDs. The art project is untouched.
use morphforge_core::{
    material_wire::Surface,
    protocol::{key, GeometryKey},
    Box3, Scene,
};
use std::collections::BTreeMap;
pub fn candidate(
    scene: &Scene,
    surfaces: &BTreeMap<GeometryKey, Surface>,
    can_merge: impl Fn(&Box3, Surface, usize) -> bool,
) -> (Scene, BTreeMap<GeometryKey, Surface>) {
    let mut items = scene
        .boxes
        .iter()
        .map(|b| {
            (
                b.clone(),
                surfaces
                    .get(&key(b))
                    .copied()
                    .unwrap_or_else(|| Surface::catalog(b.style_id, b.tint_id, b.motion_phase)),
            )
        })
        .collect::<Vec<_>>();
    for _ in 0..3 {
        let before = items.len();
        for axis in 0..3 {
            let mut groups = BTreeMap::<(Surface, [i32; 3], [u32; 3]), Vec<Box3>>::new();
            let mut result = Vec::new();
            for (b, s) in items {
                if b.shape != 0 || !can_merge(&b, s, axis) {
                    result.push((b, s));
                    continue;
                }
                let mut min = b.min;
                let mut size = b.size;
                min[axis] = 0;
                size[axis] = 0;
                groups.entry((s, min, size)).or_default().push(b);
            }
            for ((surface, _, _), mut boxes) in groups {
                boxes.sort_by_key(|b| b.min[axis]);
                let mut current: Option<Box3> = None;
                for b in boxes {
                    if let Some(c) = &mut current {
                        if c.min[axis] + c.size[axis] as i32 == b.min[axis]
                            && c.size[axis] + b.size[axis] <= 256
                        {
                            c.size[axis] += b.size[axis];
                            continue;
                        }
                        result.push((current.take().unwrap(), surface));
                    }
                    current = Some(b);
                }
                if let Some(b) = current {
                    result.push((b, surface));
                }
            }
            items = result;
        }
        if items.len() == before {
            break;
        }
    }
    let mut out = scene.clone();
    out.boxes = items.iter().map(|(b, _)| b.clone()).collect();
    let surfaces = items.iter().map(|(b, s)| (key(b), *s)).collect();
    (out, surfaces)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn runs_merge_only_identical_rendered_surfaces_and_never_modify_source() {
        let source = Scene {
            boxes: (0..16)
                .map(|i| Box3::new([i, 0, 0], [1; 3], i as u32 + 1))
                .collect(),
            ..Default::default()
        };
        let surfaces = source
            .boxes
            .iter()
            .enumerate()
            .map(|(i, b)| (key(b), Surface::catalog(if i < 8 { 1 } else { 2 }, 0, 0)))
            .collect();
        let (out, wire) = candidate(&source, &surfaces, |_, _, _| true);
        assert_eq!(out.boxes.len(), 2);
        assert!(out.boxes.iter().all(|b| b.size == [8, 1, 1]));
        assert_eq!(source.boxes.len(), 16);
        assert_eq!(wire.len(), 2);
        for x in 0..16 {
            let b = out
                .boxes
                .iter()
                .find(|b| b.min[0] <= x && x < b.min[0] + b.size[0] as i32)
                .unwrap();
            assert_eq!(
                wire[&key(b)],
                Surface::catalog(if x < 8 { 1 } else { 2 }, 0, 0)
            );
        }
    }
}
