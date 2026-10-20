//! Resolve editor selections against the canonical scene after a transform.
//! Boxes can split/merge and change IDs, so retaining source IDs is insufficient.
use anyhow::Result;
use morphforge_core::{Box3, Limits, Operation, Scene};
use std::collections::{BTreeSet, HashMap, HashSet};

pub fn after_edit(
    before: &Scene,
    after: &Scene,
    operations: &[Operation],
    limits: &Limits,
) -> Result<Option<Vec<u64>>> {
    let (targets, copy, paint) = if let [Operation::Transform {
        ids,
        offset,
        turns,
        mirror,
        copy,
    }] = operations
    {
        let ids: BTreeSet<_> = ids.iter().copied().collect();
        let mut targets = before.clone();
        targets.boxes.retain(|b| ids.contains(&b.id));
        targets.triangles.retain(|t| ids.contains(&t.id));
        let targets = targets.apply(
            &[Operation::Transform {
                ids: ids.into_iter().collect(),
                offset: *offset,
                turns: *turns,
                mirror: mirror.clone(),
                copy: false,
            }],
            limits,
        )?;
        (targets, *copy, false)
    } else if !operations.is_empty()
        && operations.iter().all(|op| {
            matches!(
                op,
                Operation::Paint { .. } | Operation::PaintTriangles { .. }
            )
        })
    {
        let mut targets = Scene::default();
        let mut faces = HashSet::new();
        for op in operations {
            match op {
                Operation::Paint {
                    min,
                    size,
                    style_id,
                    tint_id,
                } => {
                    let mut b = Box3::new(*min, *size, *style_id);
                    b.tint_id = *tint_id;
                    targets.boxes.push(b);
                }
                Operation::PaintTriangles { ids, .. } => faces.extend(ids.iter().copied()),
                _ => unreachable!(),
            }
        }
        targets.triangles = after
            .triangles
            .iter()
            .filter(|t| faces.contains(&t.id))
            .cloned()
            .collect();
        (targets, false, true)
    } else {
        return Ok(None);
    };
    let mut selected = BTreeSet::new();
    // A bounded spatial broad phase keeps full 16K selections responsive.
    let mut buckets: HashMap<[i32; 3], Vec<usize>> = HashMap::new();
    let mut large = Vec::new();
    for (index, target) in targets.boxes.iter().enumerate() {
        if let Some(cells) = chunks(target) {
            for cell in cells {
                buckets.entry(cell).or_default().push(index);
            }
        } else {
            large.push(index);
        }
    }
    for b in &after.boxes {
        let matches = |index: usize| {
            let t = &targets.boxes[index];
            (if paint {
                b.style_id == t.style_id && b.tint_id == t.tint_id
            } else {
                b.appearance() == t.appearance() && b.shape == t.shape
            }) && b.intersects(t)
        };
        let found = if let Some(cells) = chunks(b) {
            large.iter().copied().any(matches)
                || cells.iter().any(|cell| {
                    buckets
                        .get(cell)
                        .is_some_and(|v| v.iter().copied().any(matches))
                })
        } else {
            (0..targets.boxes.len()).any(matches)
        };
        if found {
            selected.insert(b.id);
        }
    }
    let faces: HashSet<_> = targets
        .triangles
        .iter()
        .map(|t| (t.vertices, t.color, t.group.as_str(), t.double_sided))
        .collect();
    let original_faces: HashSet<_> = before.triangles.iter().map(|t| t.id).collect();
    for t in &after.triangles {
        if copy && original_faces.contains(&t.id) {
            continue;
        }
        if faces.contains(&(t.vertices, t.color, t.group.as_str(), t.double_sided)) {
            selected.insert(t.id);
        }
    }
    Ok(Some(selected.into_iter().collect()))
}
fn chunks(b: &Box3) -> Option<Vec<[i32; 3]>> {
    let lo = b.min.map(|n| n.div_euclid(32));
    let hi = b.end().map(|n| (n - 1).div_euclid(32));
    let count = (0..3).map(|a| (hi[a] - lo[a] + 1) as u64).product::<u64>();
    if count > 512 {
        return None;
    }
    let mut out = Vec::with_capacity(count as usize);
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                out.push([x, y, z]);
            }
        }
    }
    Some(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    use morphforge_core::{Box3, Triangle3};
    fn run(copy: bool, offset: [i32; 3]) {
        let before = Scene {
            boxes: vec![Box3 {
                id: 1,
                ..Box3::new([0, 0, 0], [4, 4, 4], 1)
            }],
            ..Default::default()
        };
        let op = Operation::Transform {
            ids: vec![1],
            offset,
            turns: 0,
            mirror: None,
            copy,
        };
        let after = before
            .apply(std::slice::from_ref(&op), &Limits::default())
            .unwrap();
        let ids = after_edit(&before, &after, &[op], &Limits::default())
            .unwrap()
            .unwrap();
        assert!(!ids.is_empty());
        assert!(ids.iter().all(|id| after.boxes.iter().any(|b| b.id == *id)));
        assert!(after
            .boxes
            .iter()
            .filter(|b| ids.contains(&b.id))
            .any(|b| b.contains_cell(offset)));
        if copy && offset[0] >= 8 {
            assert!(!ids.contains(&after.boxes.iter().find(|b| b.min == [0, 0, 0]).unwrap().id));
        }
    }
    #[test]
    fn moving_retains_a_selection_when_box_ids_change() {
        run(false, [8, 0, 0]);
    }
    #[test]
    fn copying_selects_destination_and_handles_overwrite_and_merge() {
        run(true, [2, 0, 0]);
        run(true, [4, 0, 0]);
        run(true, [8, 0, 0]);
    }
    #[test]
    fn colored_triangle_copy_selects_only_the_new_face() {
        let before = Scene {
            triangles: vec![Triangle3 {
                id: 1,
                vertices: [[0, 0, 0], [4, 0, 0], [0, 4, 0]],
                color: [40, 70, 90],
                double_sided: false,
                group: "face".into(),
            }],
            ..Default::default()
        };
        let op = Operation::Transform {
            ids: vec![1],
            offset: [8, 0, 0],
            turns: 1,
            mirror: Some("x".into()),
            copy: true,
        };
        let after = before
            .apply(std::slice::from_ref(&op), &Limits::default())
            .unwrap();
        let ids = after_edit(&before, &after, &[op], &Limits::default())
            .unwrap()
            .unwrap();
        assert_eq!(ids.len(), 1);
        assert_ne!(ids[0], 1);
        assert!(after.triangles.iter().any(|t| t.id == ids[0]));
    }
    #[test]
    fn repainting_keeps_the_merged_destination_selected() {
        let before = Scene {
            boxes: vec![Box3 {
                id: 1,
                ..Box3::new([0, 0, 0], [4, 4, 4], 1)
            }],
            ..Default::default()
        };
        let op = Operation::Paint {
            min: [0, 0, 0],
            size: [4, 4, 4],
            style_id: 3,
            tint_id: 0,
        };
        let after = before
            .apply(std::slice::from_ref(&op), &Limits::default())
            .unwrap();
        let ids = after_edit(&before, &after, &[op], &Limits::default())
            .unwrap()
            .unwrap();
        assert_eq!(ids.len(), 1);
        assert_eq!(
            after
                .boxes
                .iter()
                .find(|b| b.id == ids[0])
                .unwrap()
                .style_id,
            3
        );
    }
    #[test]
    fn full_capacity_box_selection_is_resolved_without_losing_ids() {
        let before = Scene {
            boxes: (0..16384)
                .map(|i| Box3 {
                    id: i + 1,
                    ..Box3::new(
                        [
                            ((i % 128) * 2) as i32 - 128,
                            0,
                            ((i / 128) * 2) as i32 - 128,
                        ],
                        [1, 1, 1],
                        1,
                    )
                })
                .collect(),
            ..Default::default()
        };
        let op = Operation::Transform {
            ids: before.boxes.iter().map(|b| b.id).collect(),
            offset: [0, 2, 0],
            turns: 0,
            mirror: None,
            copy: false,
        };
        let after = before
            .apply(std::slice::from_ref(&op), &Limits::default())
            .unwrap();
        let ids = after_edit(&before, &after, &[op], &Limits::default())
            .unwrap()
            .unwrap();
        assert_eq!(ids.len(), 16384);
    }
}
