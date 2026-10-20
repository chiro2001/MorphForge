use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use morphforge_app::{
    image_tools, references, server::App, store::Workspace, target::AvatarCatalog,
};
use morphforge_core::{Box3, Operation};
use serde_json::json;
use std::io::Cursor;
fn add(w: &mut Workspace) -> String {
    let im = RgbaImage::from_fn(3, 2, |x, y| {
        Rgba(match (x, y) {
            (0, 0) => [255, 0, 0, 255],
            (1, 0) => [0, 255, 0, 255],
            (2, 0) => [0, 0, 255, 0],
            (0, 1) => [0, 0, 255, 255],
            (1, 1) => [255, 0, 0, 255],
            _ => [1, 2, 3, 255],
        })
    });
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(im)
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    references::import(w, "test", references::prepare(bytes.get_ref()).unwrap()).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}
#[test]
fn exact_pixels_crop_resize_and_derived_attribution_are_independent_of_scene() {
    let d = tempfile::tempdir().unwrap();
    let mut w = Workspace::open(d.path().into()).unwrap();
    let id = add(&mut w);
    let before = serde_json::to_value(&w.project).unwrap();
    let v = image_tools::action(
        &mut w,
        &json!({"action":"inspect","imageId":id,"points":[[0,0],[2,0],[2,1]],"grid":[3,2]}),
        None,
    )
    .unwrap();
    assert_eq!(v["points"][0]["rgba"], json!([255, 0, 0, 255]));
    assert_eq!(v["points"][1]["rgba"], json!([0, 0, 255, 0]));
    assert_eq!(v["points"][2]["rgba"], json!([1, 2, 3, 255]));
    assert_eq!(v["grid"]["indices"][0][0], 0);
    let v=image_tools::action(&mut w,&json!({"action":"resize","imageId":id,"crop":{"x":0,"y":0,"width":2,"height":2},"width":4,"filter":"nearest"}),None).unwrap();
    assert_eq!(v["width"], 4);
    assert_eq!(v["height"], 4);
    assert_eq!(v["parentImageId"], id);
    assert_eq!(v["imageRole"], "reinspection");
    assert_ne!(v["id"], id);
    assert_eq!(
        references::record(&w, v["id"].as_str().unwrap())
            .unwrap()
            .license,
        references::record(&w, &id).unwrap().license
    );
    assert_eq!(serde_json::to_value(&w.project).unwrap(), before);
    assert!(references::image(&w, &id, false).is_ok());
    assert!(image_tools::action(
        &mut w,
        &json!({"action":"inspect","imageId":id,"points":[[3,0]]}),
        None
    )
    .is_err());
    assert!(image_tools::action(
        &mut w,
        &json!({"action":"inspect","imageId":id,"crop":{"x":2,"y":0,"width":2,"height":2}}),
        None
    )
    .is_err());
}
#[test]
fn plane_preview_apply_orientation_holes_reuse_and_undo() {
    let d = tempfile::tempdir().unwrap();
    let mut w = Workspace::open(d.path().into()).unwrap();
    let id = add(&mut w);
    w.apply(
        &[Operation::Box {
            value: Box3::new([0, 0, 0], [3, 2, 1], 1),
        }],
        Some(0),
        None,
        None,
    )
    .unwrap();
    let before = serde_json::to_value(&w.project).unwrap();
    let args = json!({"action":"voxel_plane","imageId":id,"baseRevision":1,"width":3,"height":2,"filter":"nearest","origin":[0,0,0],"canvasMode":"replace"});
    let p = image_tools::action(&mut w, &args, None).unwrap();
    assert!(p["dryRun"].as_bool().unwrap());
    assert_eq!(p["paletteColors"], 4);
    assert_eq!(serde_json::to_value(&w.project).unwrap(), before);
    let mut apply = args.clone();
    apply["dryRun"] = json!(false);
    let result = image_tools::action(&mut w, &apply, None).unwrap();
    assert_eq!(result["revision"], 2);
    assert_eq!(
        w.project.scene.boxes.iter().map(Box3::volume).sum::<u64>(),
        5
    );
    let at = |pos| {
        let b = w
            .project
            .scene
            .boxes
            .iter()
            .find(|b| b.contains_cell(pos))
            .unwrap();
        w.project
            .materials
            .iter()
            .find(|m| m.id == b.style_id)
            .unwrap()
            .color
            .map(|v| (v * 255.).round() as u8)
    };
    assert_eq!(at([0, 1, 0]), [255, 0, 0]);
    assert_eq!(at([1, 1, 0]), [0, 255, 0]);
    assert_eq!(at([0, 0, 0]), [0, 0, 255]);
    assert!(!w
        .project
        .scene
        .boxes
        .iter()
        .any(|b| b.contains_cell([2, 1, 0])));
    assert!(!w.project.source_credits.is_empty());
    let count = w.project.materials.len();
    apply["baseRevision"] = json!(2);
    let again = image_tools::action(&mut w, &apply, None).unwrap();
    assert_eq!(again["materialsAdded"], 0);
    assert_eq!(w.project.materials.len(), count);
    w.undo(false, Some(1)).unwrap();
    assert_eq!(
        serde_json::to_value(&w.project.scene.boxes).unwrap(),
        before["scene"]["boxes"]
    );
    assert!(w.project.source_credits.is_empty());
}
#[test]
fn invalid_or_protected_plane_does_not_change_geometry_or_materials() {
    let d = tempfile::tempdir().unwrap();
    let mut w = Workspace::open(d.path().into()).unwrap();
    let id = add(&mut w);
    let mut b = Box3::new([0, 0, 0], [2; 3], 1);
    b.group = "keep".into();
    w.apply(&[Operation::Box { value: b }], Some(0), None, None)
        .unwrap();
    w.project.protected_groups = ["keep".into()].into_iter().collect();
    let before = serde_json::to_value(&w.project).unwrap();
    for extra in [
        json!({"origin":[0,0,0]}),
        json!({"baseRevision":0}),
        json!({"maxColors":0}),
        json!({"width":257}),
        json!({"materialIds":[65535]}),
        json!({"origin":[1000000,0,0]}),
    ] {
        let mut args = json!({"action":"voxel_plane","imageId":id,"baseRevision":1,"width":3,"height":2,"dryRun":false});
        for (k, v) in extra.as_object().unwrap() {
            args[k] = v.clone()
        }
        assert!(image_tools::action(&mut w, &args, None).is_err());
        assert_eq!(serde_json::to_value(&w.project).unwrap(), before);
    }
}
#[tokio::test]
async fn harness_exposes_image_tools_and_nearest_existing_materials_no_new_palette() {
    let d = tempfile::tempdir().unwrap();
    let app = App::open(d.path().into(), Some(d.path().join("none.env"))).unwrap();
    let id = add(&mut app.workspace.lock().unwrap());
    let r=app.harness.tool("image_tools",&json!({"action":"voxel_plane","imageId":id,"baseRevision":0,"width":3,"height":2,"materialIds":[1,4],"maxColors":2,"dryRun":false}),None).await.unwrap();
    assert_eq!(r["materialsAdded"], 0);
    assert!(app
        .workspace
        .lock()
        .unwrap()
        .project
        .scene
        .boxes
        .iter()
        .all(|b| [1, 4].contains(&b.style_id)));
    let schema = morphforge_app::harness::tools_for_language("zh");
    assert!(schema
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["name"] == "image_tools"));
    let c =
        AvatarCatalog::from_bytes(include_bytes!("../../../materials/mf5/catalog.json")).unwrap();
    let w = app.workspace.lock().unwrap();
    assert!(
        morphforge_app::target::project_wire(&w.project.scene, &w.project.materials, Some(&c))
            .is_ok()
    );
}

#[test]
fn median_cut_preserves_gradient_span_and_all_planes_have_expected_orientation() {
    let d = tempfile::tempdir().unwrap();
    let mut w = Workspace::open(d.path().into()).unwrap();
    let mut b = Cursor::new(Vec::new());
    let im = RgbaImage::from_fn(64, 1, |x, _| Rgba([(x * 4) as u8, 0, 0, 255]));
    DynamicImage::ImageRgba8(im)
        .write_to(&mut b, ImageFormat::Png)
        .unwrap();
    let id = references::import(
        &mut w,
        "gradient",
        references::prepare(b.get_ref()).unwrap(),
    )
    .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let r=image_tools::action(&mut w,&json!({"action":"voxel_plane","imageId":id,"width":64,"height":1,"maxColors":4,"baseRevision":0,"dryRun":false}),None).unwrap();
    assert_eq!(r["paletteColors"], 4);
    let colors: Vec<_> = w
        .project
        .scene
        .boxes
        .iter()
        .map(|b| {
            w.project
                .materials
                .iter()
                .find(|m| m.id == b.style_id)
                .unwrap()
                .color[0]
        })
        .collect();
    assert!(colors.iter().copied().fold(0., f32::max) > 0.8);
    assert!(colors.iter().copied().fold(1., f32::min) < 0.2);
    let id = add(&mut w);
    for (plane, min, max) in [
        ("xy", [-1, 0, 0], [2, 2, 1]),
        ("xz", [-1, 0, -1], [2, 1, 1]),
        ("yz", [0, 0, -1], [1, 2, 2]),
    ] {
        let r=image_tools::action(&mut w,&json!({"action":"voxel_plane","imageId":id,"width":3,"height":2,"plane":plane,"maxColors":4,"baseRevision":1}),None).unwrap();
        assert_eq!(r["origin"], json!(min));
        let _ = max;
    }
}

#[test]
fn texture_palette_uses_image_representative_colors_and_overlay_preserves_holes() {
    let d = tempfile::tempdir().unwrap();
    let mut w = Workspace::open(d.path().into()).unwrap();
    let id = add(&mut w);
    for (id, color) in [(6000, [255, 0, 0, 255]), (6001, [0, 0, 255, 255])] {
        let im = RgbaImage::from_pixel(2, 2, Rgba(color));
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(im)
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();
        let file = w.artifact(bytes.get_ref(), "png").unwrap();
        let mut m = w.project.materials[0].clone();
        m.id = id;
        m.uid = format!("test:texture:{id}");
        m.color = [1.; 3];
        m.textures = vec![format!("/artifacts/{file}"); 6];
        w.project.materials.push(m);
    }
    w.apply(
        &[Operation::Box {
            value: Box3::new([-1, 0, 0], [3, 2, 1], 1),
        }],
        Some(0),
        None,
        None,
    )
    .unwrap();
    let r=image_tools::action(&mut w,&json!({"action":"voxel_plane","imageId":id,"width":3,"height":2,"baseRevision":1,"materialIds":[6000,6001],"dryRun":false}),None).unwrap();
    assert_eq!(r["materialsAdded"], 0);
    assert_eq!(r["canvasReplacesRegion"], false);
    assert_eq!(r["target"]["fits"], true);
    for (pos, style) in [([-1, 1, 0], 6000), ([-1, 0, 0], 6001), ([1, 1, 0], 1)] {
        assert_eq!(
            w.project
                .scene
                .boxes
                .iter()
                .find(|b| b.contains_cell(pos))
                .unwrap()
                .style_id,
            style
        );
    }
}
