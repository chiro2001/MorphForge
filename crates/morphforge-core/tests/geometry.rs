use morphforge_core::protocol::*;
use morphforge_core::*;
use rand::{Rng, SeedableRng};
use std::collections::BTreeMap;
fn cells(s: &Scene) -> BTreeMap<[i32; 3], u32> {
    let mut out = BTreeMap::new();
    for b in &s.boxes {
        let e = b.end();
        for x in b.min[0]..e[0] {
            for y in b.min[1]..e[1] {
                for z in b.min[2]..e[2] {
                    assert!(out.insert([x, y, z], b.style_id).is_none(), "overlap");
                }
            }
        }
    }
    out
}
#[test]
fn random_operations_match_independent_dense_oracle() {
    let mut scene = Scene::default();
    let mut oracle = BTreeMap::new();
    let mut rng = rand::rngs::StdRng::seed_from_u64(3612);
    let limits = Limits::default();
    for _ in 0..500 {
        let p = [
            rng.gen_range(-4..4),
            rng.gen_range(0..8),
            rng.gen_range(-4..4),
        ];
        let size = [
            rng.gen_range(1..3),
            rng.gen_range(1..3),
            rng.gen_range(1..3),
        ];
        let style = rng.gen_range(1..7);
        let kind = rng.gen_range(0..3);
        let op = match kind {
            0 => Operation::Box {
                value: Box3::new(p, size, style),
            },
            1 => Operation::Erase { min: p, size },
            _ => Operation::Paint {
                min: p,
                size,
                style_id: style,
                tint_id: 0,
            },
        };
        for x in p[0]..p[0] + size[0] as i32 {
            for y in p[1]..p[1] + size[1] as i32 {
                for z in p[2]..p[2] + size[2] as i32 {
                    match kind {
                        0 => {
                            oracle.insert([x, y, z], style);
                        }
                        1 => {
                            oracle.remove(&[x, y, z]);
                        }
                        _ => {
                            if let Some(s) = oracle.get_mut(&[x, y, z]) {
                                *s = style
                            }
                        }
                    }
                }
            }
        }
        scene = scene.apply(&[op], &limits).unwrap();
        assert_eq!(cells(&scene), oracle);
        scene.validate(&limits).unwrap();
    }
}
#[test]
fn huge_cube_is_sparse_and_hole_has_exact_volume() {
    let l = Limits::default();
    let s = Scene::default()
        .apply(
            &[Operation::Box {
                value: Box3::new([-128, 0, -128], [256; 3], 1),
            }],
            &l,
        )
        .unwrap();
    assert_eq!(s.boxes.len(), 1);
    let t = s
        .apply(
            &[Operation::Erase {
                min: [0, 3, 0],
                size: [2; 3],
            }],
            &l,
        )
        .unwrap();
    assert_eq!(t.boxes.len(), 6);
    assert_eq!(t.stats(&l).voxel_count, 256u64.pow(3) - 8);
}
#[test]
fn failed_transaction_preserves_original() {
    let l = Limits {
        max_boxes: 1,
        ..Limits::default()
    };
    let s = Scene::default()
        .apply(
            &[Operation::Box {
                value: Box3::new([0; 3], [3; 3], 1),
            }],
            &l,
        )
        .unwrap();
    let before = serde_json::to_string(&s).unwrap();
    assert!(s
        .apply(
            &[Operation::Erase {
                min: [1; 3],
                size: [1; 3]
            }],
            &l
        )
        .is_err());
    assert_eq!(serde_json::to_string(&s).unwrap(), before);
}
#[test]
fn geometry_beyond_vrc_is_valid_art() {
    let l = Limits::default();
    let s = Scene::default()
        .apply(
            &[Operation::Box {
                value: Box3::new([500, 20, -600], [800, 2, 2], 9),
            }],
            &l,
        )
        .unwrap();
    assert!(validate_target(&s, 16384).is_err());
    assert!(s.validate(&l).is_ok());
}
#[test]
fn independent_16k_boxes_fit_without_quadratic_validation() {
    let mut ops = vec![];
    for x in 0..32 {
        for y in 0..16 {
            for z in 0..32 {
                ops.push(Operation::Box {
                    value: Box3::new([x * 3 - 48, y * 3, z * 3 - 48], [1; 3], 1),
                });
            }
        }
    }
    let start = std::time::Instant::now();
    let s = Scene::default().apply(&ops, &Limits::default()).unwrap();
    assert_eq!(s.boxes.len(), 16384);
    s.validate(&Limits::default()).unwrap();
    assert!(validate_target(&s, 16384).is_ok());
    eprintln!("16384 sparse boxes + validation: {:?}", start.elapsed());
}
#[test]
fn group_prevents_semantic_merge_and_transform_is_exact() {
    let l = Limits::default();
    let mut a = Box3::new([0; 3], [2, 1, 1], 1);
    a.group = "a".into();
    let mut b = Box3::new([2, 0, 0], [2, 1, 1], 1);
    b.group = "b".into();
    let s = Scene::default()
        .apply(
            &[Operation::Box { value: a }, Operation::Box { value: b }],
            &l,
        )
        .unwrap();
    assert_eq!(s.boxes.len(), 2);
    let t = s
        .apply(
            &[Operation::Transform {
                ids: vec![s.boxes[0].id],
                offset: [0, 2, 0],
                turns: 1,
                mirror: None,
                copy: true,
            }],
            &l,
        )
        .unwrap();
    assert_eq!(t.stats(&l).voxel_count, 6);
}
#[test]
fn legacy_import_shifts_only_xz() {
    let v = serde_json::json!({"format":"MorphForgeEditor/1","size":16,"boxes":[{"min":[128,0,128],"size":[1,2,1],"styleId":2}]});
    let s = Scene::import_json(&v, &Limits::default()).unwrap();
    assert_eq!(s.boxes[0].min, [0; 3]);
}
#[test]
fn mf3_byte_layout_and_opcode_range() {
    for seq in 0..16 {
        for command in [
            Command::Box {
                value: Box3::new([-128, 0, 127], [1; 3], 4095),
            },
            Command::Voxel {
                value: Box3::new([0; 3], [1; 3], 1),
            },
            Command::Clear,
            Command::Brush {
                style: 4095,
                tint: 63,
                motion: 63,
            },
        ] {
            let p = packet(&command, seq, false).unwrap();
            assert_eq!(&p[..8], b"#bundle\0");
            assert_eq!(p.windows(5).filter(|w| *w == b"MF3_D").count(), 6);
            assert_eq!(
                i32::from_be_bytes(p[p.len() - 4..].try_into().unwrap()),
                ((seq << 4) | payload(&command).unwrap().0) as i32
            );
        }
    }
    let b = Box3::new([0, 0, 0], [256, 1, 1], 1);
    assert!(validate_box(&b).is_err());
}
#[test]
fn brush_is_sent_only_at_changes_and_remove_uses_old_geometry() {
    let a = Box3::new([0; 3], [1; 3], 2);
    let b = Box3::new([3, 0, 0], [1; 3], 2);
    let c = plan(&[], &[a.clone(), b.clone()], None, false);
    assert_eq!(c.len(), 3);
    assert!(matches!(c[0], Command::Brush { .. }));
    let c = plan(&[a, b.clone()], &[b], Some((2, 0, 0)), false);
    assert_eq!(c.len(), 1);
    assert!(matches!(c[0], Command::Remove { .. }));
}

#[test]
fn import_rejects_non_scene_json_and_unwraps_exported_projects() {
    let limits = Limits::default();
    for value in [
        serde_json::json!({}),
        serde_json::json!({"arbitrary":"json"}),
        serde_json::json!({"format":"Unknown/9","boxes":[]}),
        serde_json::json!({"boxes":null}),
        serde_json::json!({"boxes":[],"size":0}),
    ] {
        assert!(Scene::import_json(&value, &limits).is_err());
    }
    let scene = Scene::default()
        .apply(
            &[Operation::Box {
                value: Box3::new([2, 1, 3], [4, 2, 1], 3),
            }],
            &limits,
        )
        .unwrap();
    let exported = serde_json::json!({"format":"MorphForgeProject/1","project":{"scene":scene,"materials":[]}});
    let imported = Scene::import_json(&exported, &limits).unwrap();
    assert_eq!(imported.stats(&limits).voxel_count, 8);
    assert_eq!(imported.boxes[0].min, [2, 1, 3]);
}
