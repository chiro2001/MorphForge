use morphforge_core::*;
fn triangle() -> Triangle3 {
    Triangle3::new([[0, 0, 0], [4, 0, 0], [0, 4, 0]], [240, 120, 40])
}
#[test]
fn mixed_object_ids_updates_and_delete_are_unambiguous() {
    let limits = Limits::default();
    let scene = Scene::default()
        .apply(
            &[
                Operation::Box {
                    value: Box3::new([0, 0, 3], [1; 3], 1),
                },
                Operation::Triangle { value: triangle() },
            ],
            &limits,
        )
        .unwrap();
    assert_ne!(scene.boxes[0].id, scene.triangles[0].id);
    assert_eq!(scene.format, "MorphForgeScene/2");
    let mut update = scene.triangles[0].clone();
    update.color = [0, 255, 0];
    update.vertices[2] = [0, 8, 0];
    let next = scene
        .apply(
            &[Operation::Triangle {
                value: update.clone(),
            }],
            &limits,
        )
        .unwrap();
    assert_eq!(next.triangles, vec![update]);
    assert_eq!(next.boxes, scene.boxes);
    let next = next
        .apply(
            &[Operation::RemoveTriangles {
                ids: vec![scene.triangles[0].id],
            }],
            &limits,
        )
        .unwrap();
    assert!(next.triangles.is_empty());
    assert_eq!(next.boxes.len(), 1);
}
#[test]
fn transform_mirror_keeps_face_front_and_source_when_copying() {
    let limits = Limits::default();
    let scene = Scene::default()
        .apply(&[Operation::Triangle { value: triangle() }], &limits)
        .unwrap();
    let id = scene.triangles[0].id;
    let next = scene
        .apply(
            &[Operation::Transform {
                ids: vec![id],
                offset: [10, 2, 0],
                turns: 0,
                mirror: Some("x".into()),
                copy: true,
            }],
            &limits,
        )
        .unwrap();
    assert_eq!(next.triangles.len(), 2);
    assert_eq!(next.triangles[0], scene.triangles[0]);
    let v = next.triangles[1].vertices;
    let cross =
        (v[1][0] - v[0][0]) * (v[2][1] - v[0][1]) - (v[1][1] - v[0][1]) * (v[2][0] - v[0][0]);
    assert!(cross > 0);
}
#[test]
fn face_groups_paint_and_whole_face_erase_work() {
    let limits = Limits::default();
    let scene = Scene::default()
        .apply(&[Operation::Triangle { value: triangle() }], &limits)
        .unwrap();
    let id = scene.triangles[0].id;
    let next = scene
        .apply(
            &[
                Operation::Group {
                    ids: vec![id],
                    name: "roof".into(),
                },
                Operation::PaintTriangles {
                    ids: vec![id],
                    color: [1, 2, 3],
                },
            ],
            &limits,
        )
        .unwrap();
    assert_eq!(next.triangles[0].group, "roof");
    assert_eq!(next.triangles[0].color, [1, 2, 3]);
    assert_eq!(next.stats(&limits).groups["roof"], 1);
    assert!(next
        .apply(
            &[Operation::RemoveGroup {
                name: "roof".into()
            }],
            &limits
        )
        .unwrap()
        .triangles
        .is_empty());
    assert!(next
        .apply(
            &[Operation::Erase {
                min: [1, 1, 0],
                size: [1; 3]
            }],
            &limits
        )
        .unwrap()
        .triangles
        .is_empty());
}
#[test]
fn transaction_failure_preserves_geometry_and_import_keeps_mesh() {
    let limits = Limits::default();
    let scene = Scene::default()
        .apply(&[Operation::Triangle { value: triangle() }], &limits)
        .unwrap();
    let old = serde_json::to_value(&scene).unwrap();
    let mut bad = triangle();
    bad.vertices = [[0; 3]; 3];
    assert!(scene
        .apply(
            &[Operation::Clear, Operation::Triangle { value: bad }],
            &limits
        )
        .is_err());
    assert_eq!(serde_json::to_value(&scene).unwrap(), old);
    let imported = Scene::import_json(&old, &limits).unwrap();
    assert_eq!(imported.triangles.len(), 1);
    assert_eq!(imported.triangles[0].vertices, scene.triangles[0].vertices);
    let legacy = Scene::import_json(&serde_json::json!({"boxes":[]}), &limits).unwrap();
    assert!(legacy.triangles.is_empty());
    assert!(serde_json::to_value(legacy)
        .unwrap()
        .get("triangles")
        .is_none());
}
#[test]
fn triangles_count_towards_shared_object_limit() {
    let limits = Limits {
        max_boxes: 1,
        ..Default::default()
    };
    let result = Scene::default().apply(
        &[
            Operation::Triangle { value: triangle() },
            Operation::Box {
                value: Box3::new([0, 0, 8], [1; 3], 1),
            },
        ],
        &limits,
    );
    assert!(result.is_err());
}
