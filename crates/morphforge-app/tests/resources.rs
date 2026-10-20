use std::io::{Cursor, Write};

use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use morphforge_app::resources::import_java_pack;
use morphforge_app::store::Workspace;
use serde_json::json;
use tempfile::TempDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

fn png(width: u32, height: u32, colors: &[[u8; 4]]) -> Vec<u8> {
    assert_eq!(height as usize % colors.len(), 0);
    let frame_height = height / colors.len() as u32;
    let mut image = RgbaImage::new(width, height);
    for (index, color) in colors.iter().enumerate() {
        for y in 0..frame_height {
            for x in 0..width {
                image.put_pixel(x, index as u32 * frame_height + y, Rgba(*color));
            }
        }
    }
    let mut output = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut output, ImageFormat::Png)
        .unwrap();
    output.into_inner()
}

fn zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o100644);
    for (path, data) in files {
        writer.start_file(*path, options).unwrap();
        writer.write_all(data).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn workspace() -> (TempDir, Workspace) {
    let directory = tempfile::tempdir().unwrap();
    let workspace = Workspace::open(directory.path().join("workspace")).unwrap();
    (directory, workspace)
}

fn pack_meta() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "pack": {
            "pack_format": 46,
            "description": "MorphForge resource test"
        }
    }))
    .unwrap()
}

fn cube_all(texture: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "parent": "minecraft:block/cube_all",
        "textures": {"all": texture}
    }))
    .unwrap()
}

fn artifact(workspace: &Workspace, url: &str) -> RgbaImage {
    let id = url.rsplit('/').next().unwrap();
    image::load_from_memory(&std::fs::read(workspace.root.join("artifacts").join(id)).unwrap())
        .unwrap()
        .to_rgba8()
}

#[test]
fn imports_static_cube_and_is_idempotent() {
    let (_directory, mut workspace) = workspace();
    let texture = png(2, 2, &[[178, 181, 184, 255]]);
    let model = cube_all("block/stone");
    let bytes = zip(&[
        ("pack.mcmeta", &pack_meta()),
        (
            "assets/example/textures/block/stone.png",
            texture.as_slice(),
        ),
        ("assets/example/models/block/stone.json", model.as_slice()),
    ]);

    let report = import_java_pack(&mut workspace, &bytes, "example.zip").unwrap();
    assert_eq!(report["materialsAdded"], 1);
    let material = workspace
        .project
        .materials
        .iter()
        .find(|material| material.uid.starts_with("example:block/stone@"))
        .unwrap();
    assert_eq!(material.frames, 1);
    assert_eq!(material.textures.len(), 6);
    assert!(material.pack.is_some());
    assert_eq!(
        artifact(&workspace, &material.textures[0]).dimensions(),
        (2, 2)
    );

    let before = workspace.project.materials.len();
    let repeated = import_java_pack(&mut workspace, &bytes, "example.zip").unwrap();
    assert_eq!(repeated["materialsAdded"], 0);
    assert_eq!(workspace.project.materials.len(), before);
}

#[test]
fn follows_model_parent_chain_and_keeps_six_face_order() {
    let (_directory, mut workspace) = workspace();
    let side = png(1, 1, &[[255, 0, 0, 255]]);
    let top = png(1, 1, &[[0, 255, 0, 255]]);
    let bottom = png(1, 1, &[[0, 0, 255, 255]]);
    let base = serde_json::to_vec(&json!({
        "parent": "minecraft:block/cube_bottom_top",
        "textures": {
            "side": "example:block/pillar_side",
            "top": "example:block/pillar_top",
            "bottom": "example:block/pillar_bottom"
        }
    }))
    .unwrap();
    let derived = serde_json::to_vec(&json!({
        "parent": "example:block/base"
    }))
    .unwrap();
    let bytes = zip(&[
        ("pack.mcmeta", &pack_meta()),
        (
            "assets/example/textures/block/pillar_side.png",
            side.as_slice(),
        ),
        (
            "assets/example/textures/block/pillar_top.png",
            top.as_slice(),
        ),
        (
            "assets/example/textures/block/pillar_bottom.png",
            bottom.as_slice(),
        ),
        ("assets/example/models/block/base.json", base.as_slice()),
        (
            "assets/example/models/block/derived.json",
            derived.as_slice(),
        ),
    ]);

    import_java_pack(&mut workspace, &bytes, "pillar.zip").unwrap();
    let material = workspace
        .project
        .materials
        .iter()
        .find(|material| material.uid.starts_with("example:block/derived@"))
        .unwrap();
    let colors: Vec<[u8; 4]> = material
        .textures
        .iter()
        .map(|url| artifact(&workspace, url).get_pixel(0, 0).0)
        .collect();
    assert_eq!(
        colors,
        vec![
            [255, 0, 0, 255],
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255, 0, 0, 255],
            [255, 0, 0, 255],
        ]
    );
}

#[test]
fn normalizes_different_face_cycles_to_shared_tick_timeline() {
    let (_directory, mut workspace) = workspace();
    let side = png(1, 2, &[[255, 0, 0, 255], [255, 128, 0, 255]]);
    let caps = png(
        1,
        3,
        &[[0, 255, 0, 255], [0, 128, 255, 255], [128, 0, 255, 255]],
    );
    let metadata = serde_json::to_vec(&json!({"animation": {"frametime": 1}})).unwrap();
    let model = serde_json::to_vec(&json!({
        "parent": "minecraft:block/cube_bottom_top",
        "textures": {
            "side": "example:block/side",
            "top": "example:block/top",
            "bottom": "example:block/bottom"
        }
    }))
    .unwrap();
    let bytes = zip(&[
        ("pack.mcmeta", &pack_meta()),
        ("assets/example/textures/block/side.png", side.as_slice()),
        (
            "assets/example/textures/block/side.png.mcmeta",
            metadata.as_slice(),
        ),
        ("assets/example/textures/block/top.png", caps.as_slice()),
        (
            "assets/example/textures/block/top.png.mcmeta",
            metadata.as_slice(),
        ),
        ("assets/example/textures/block/bottom.png", caps.as_slice()),
        (
            "assets/example/textures/block/bottom.png.mcmeta",
            metadata.as_slice(),
        ),
        (
            "assets/example/models/block/multi_cycle.json",
            model.as_slice(),
        ),
    ]);

    import_java_pack(&mut workspace, &bytes, "multi.zip").unwrap();
    let material = workspace
        .project
        .materials
        .iter()
        .find(|material| material.uid.starts_with("example:block/multi_cycle@"))
        .unwrap();
    assert_eq!(material.frames, 6);
    assert_eq!(
        artifact(&workspace, &material.textures[0]).dimensions(),
        (1, 6)
    );
    assert_eq!(
        artifact(&workspace, &material.textures[2]).dimensions(),
        (1, 6)
    );
}

#[test]
fn bakes_explicit_frame_order_and_per_frame_timing() {
    let (_directory, mut workspace) = workspace();
    let texture = png(
        2,
        6,
        &[[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]],
    );
    let metadata = serde_json::to_vec(&json!({
        "animation": {
            "frametime": 2,
            "frames": [
                {"index": 2, "time": 1},
                {"index": 0, "time": 2},
                {"index": 1}
            ]
        }
    }))
    .unwrap();
    let model = cube_all("block/cycle");
    let bytes = zip(&[
        ("pack.mcmeta", &pack_meta()),
        (
            "assets/example/textures/block/cycle.png",
            texture.as_slice(),
        ),
        (
            "assets/example/textures/block/cycle.png.mcmeta",
            metadata.as_slice(),
        ),
        ("assets/example/models/block/cycle.json", model.as_slice()),
    ]);

    let report = import_java_pack(&mut workspace, &bytes, "cycle.zip").unwrap();
    assert_eq!(report["animatedMaterials"], 1);
    let material = workspace
        .project
        .materials
        .iter()
        .find(|material| material.uid.starts_with("example:block/cycle@"))
        .unwrap();
    assert_eq!(material.frames, 5);
    assert!((material.frame_time - 0.05).abs() < f32::EPSILON);
    let image = artifact(&workspace, &material.textures[0]);
    assert_eq!(image.dimensions(), (2, 10));
    let expected = [
        [0, 0, 255, 255],
        [255, 0, 0, 255],
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 255, 0, 255],
    ];
    for (frame, color) in expected.iter().enumerate() {
        assert_eq!(image.get_pixel(0, frame as u32 * 2).0, *color);
    }
}

#[test]
fn bakes_interpolated_ticks() {
    let (_directory, mut workspace) = workspace();
    let texture = png(1, 2, &[[0, 0, 0, 255], [255, 255, 255, 255]]);
    let metadata = serde_json::to_vec(&json!({
        "animation": {
            "frames": [
                {"index": 0, "time": 3},
                {"index": 1, "time": 1}
            ],
            "interpolate": true
        }
    }))
    .unwrap();
    let model = cube_all("block/fade");
    let bytes = zip(&[
        ("pack.mcmeta", &pack_meta()),
        ("assets/example/textures/block/fade.png", texture.as_slice()),
        (
            "assets/example/textures/block/fade.png.mcmeta",
            metadata.as_slice(),
        ),
        ("assets/example/models/block/fade.json", model.as_slice()),
    ]);

    import_java_pack(&mut workspace, &bytes, "fade.zip").unwrap();
    let material = workspace
        .project
        .materials
        .iter()
        .find(|material| material.uid.starts_with("example:block/fade@"))
        .unwrap();
    assert_eq!(material.frames, 4);
    let image = artifact(&workspace, &material.textures[0]);
    assert_eq!(image.get_pixel(0, 0).0, [0, 0, 0, 255]);
    assert_eq!(image.get_pixel(0, 1).0, [85, 85, 85, 255]);
    assert_eq!(image.get_pixel(0, 2).0, [170, 170, 170, 255]);
    assert_eq!(image.get_pixel(0, 3).0, [255, 255, 255, 255]);
}

#[test]
fn rejects_zip_path_traversal_symlinks_and_backslashes() {
    let (_directory, mut workspace) = workspace();
    let texture = png(1, 1, &[[255, 255, 255, 255]]);
    let meta = pack_meta();
    let traversal = zip(&[("pack.mcmeta", &meta), ("../evil.png", texture.as_slice())]);
    assert!(import_java_pack(&mut workspace, &traversal, "bad.zip")
        .unwrap_err()
        .to_string()
        .contains("路径"));

    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .add_symlink(
            "assets/example/textures/block/link.png",
            "stone.png",
            SimpleFileOptions::default(),
        )
        .unwrap();
    let symlink = writer.finish().unwrap().into_inner();
    assert!(import_java_pack(&mut workspace, &symlink, "link.zip")
        .unwrap_err()
        .to_string()
        .contains("符号链接"));

    let backslash = zip(&[
        ("pack.mcmeta", &meta),
        (
            "assets\\example\\textures\\block\\stone.png",
            texture.as_slice(),
        ),
    ]);
    assert!(
        import_java_pack(&mut workspace, &backslash, "backslash.zip")
            .unwrap_err()
            .to_string()
            .contains("无效路径")
    );
}

#[test]
fn reports_invalid_animation_frame_but_keeps_fallback_frames() {
    let (_directory, mut workspace) = workspace();
    let texture = png(1, 2, &[[10, 20, 30, 255], [40, 50, 60, 255]]);
    let metadata = serde_json::to_vec(&json!({
        "animation": {
            "frames": [{"index": 9, "time": 1}]
        }
    }))
    .unwrap();
    let model = cube_all("block/warn");
    let bytes = zip(&[
        ("pack.mcmeta", &pack_meta()),
        ("assets/example/textures/block/warn.png", texture.as_slice()),
        (
            "assets/example/textures/block/warn.png.mcmeta",
            metadata.as_slice(),
        ),
        ("assets/example/models/block/warn.json", model.as_slice()),
    ]);

    let report = import_java_pack(&mut workspace, &bytes, "warn.zip").unwrap();
    let warnings = report["warnings"].as_array().unwrap();
    assert!(warnings
        .iter()
        .any(|warning| warning.as_str().unwrap().contains("越界动画帧")));
    let material = workspace
        .project
        .materials
        .iter()
        .find(|material| material.uid.starts_with("example:block/warn@"))
        .unwrap();
    assert_eq!(material.frames, 2);
}
#[test]
fn preserved_pack_blockstates_select_alias_model_and_orientation() {
    use morphforge_app::minecraft::{
        compile::{self, Options},
        formats::{Block, BlockState, Building},
    };
    let (_dir, mut workspace) = workspace();
    let texture = png(2, 2, &[[200, 100, 20, 255]]);
    let model = cube_all("example:block/amber");
    let states=serde_json::to_vec(&json!({"variants":{"lit=true":{"model":"example:block/active","x":90,"y":90},"lit=false":{"model":"example:block/active"}}})).unwrap();
    let bytes = zip(&[
        ("pack.mcmeta", &pack_meta()),
        ("assets/example/textures/block/amber.png", &texture),
        ("assets/example/models/block/active.json", &model),
        ("assets/example/blockstates/lamp.json", &states),
    ]);
    let report = import_java_pack(&mut workspace, &bytes, "lamp.zip").unwrap();
    let pack = report["packId"].as_str().unwrap();
    assert_eq!(report["blockstateCount"], 1);
    assert_eq!(
        std::fs::read(workspace.root.join("packs").join(format!("{pack}.zip"))).unwrap(),
        bytes
    );
    let building = Building {
        format: "fixture".into(),
        data_version: None,
        palette: vec![BlockState::parse("example:lamp[lit=true]").unwrap()],
        blocks: vec![Block {
            position: [0; 3],
            state: 0,
        }],
        regions: vec![],
        bounds: Some(([0; 3], [1; 3])),
        entities: 0,
        block_entities: 0,
        overwritten_cells: 0,
        source_cells: 1,
        warnings: vec![],
    };
    let compiled = compile::compile(
        &mut workspace,
        &building,
        Options {
            resource_pack: pack.into(),
            ..Default::default()
        },
    )
    .unwrap();
    let m = compiled
        .materials
        .iter()
        .find(|m| m.id == compiled.scene.boxes[0].style_id)
        .unwrap();
    assert!(m.uid.starts_with("example:block/active@"));
    assert_eq!(m.texture_rotation, 9);
    assert_eq!(m.texture_scale, 1.);
    assert_eq!(compiled.report["approximateMaterialStates"], 0);
    let path = workspace
        .root
        .join("packs")
        .join(format!("{pack}-blockstates.json"));
    std::fs::write(path, b"{}").unwrap();
    assert!(compile::compile(
        &mut workspace,
        &building,
        Options {
            resource_pack: pack.into(),
            ..Default::default()
        }
    )
    .err()
    .unwrap()
    .to_string()
    .contains("MC_PACK_CORRUPT"));
}

#[test]
fn vanilla_full_cube_templates_preserve_grass_layers_and_face_tint() {
    let (_dir, mut w) = workspace();
    let dirt = png(2, 2, &[[100, 50, 25, 255]]);
    let top = png(2, 2, &[[255, 255, 255, 255]]);
    let overlay = png(2, 2, &[[255, 255, 255, 128]]);
    let faces = json!({"east":{"texture":"#side"},"west":{"texture":"#side"},"up":{"texture":"#top","tintindex":0},"down":{"texture":"#bottom"},"south":{"texture":"#side"},"north":{"texture":"#side"}});
    let mut grass = json!({"textures":{"side":"minecraft:block/dirt","bottom":"minecraft:block/dirt","top":"minecraft:block/grass_top","overlay":"minecraft:block/overlay"},"elements":[{"from":[0,0,0],"to":[16,16,16],"faces":faces},{"from":[0,0,0],"to":[16,16,16],"faces":{"east":{"texture":"#overlay","tintindex":0}}}]});
    // A template is present in the ZIP, rather than assumed absent as in
    // ordinary resource packs. Layered full cubes remain valid cube materials.
    grass["parent"] = json!("minecraft:block/block");
    let model = serde_json::to_vec(&grass).unwrap();
    let bytes = zip(&[
        ("pack.mcmeta", &pack_meta()),
        ("assets/minecraft/models/block/grass_block.json", &model),
        ("assets/minecraft/textures/block/dirt.png", &dirt),
        ("assets/minecraft/textures/block/grass_top.png", &top),
        ("assets/minecraft/textures/block/overlay.png", &overlay),
    ]);
    let r = import_java_pack(&mut w, &bytes, "grass.zip").unwrap();
    assert_eq!(r["materialsAdded"], 1);
    let m = w
        .project
        .materials
        .iter()
        .find(|m| m.uid.starts_with("minecraft:block/grass_block@"))
        .unwrap();
    assert_eq!(
        artifact(&w, &m.textures[0]).get_pixel(0, 0).0,
        [123, 120, 57, 255]
    );
    assert_eq!(
        artifact(&w, &m.textures[2]).get_pixel(0, 0).0,
        [145, 189, 89, 255]
    );
    assert_eq!(
        artifact(&w, &m.textures[3]).get_pixel(0, 0).0,
        [100, 50, 25, 255]
    );
}

#[test]
fn partial_alpha_glass_and_slime_are_blended_instead_of_disappearing_in_cutout() {
    for name in [
        "white_stained_glass",
        "tinted_glass",
        "slime_block",
        "nether_portal",
    ] {
        assert_eq!(
            morphforge_app::resources::alpha_render_mode(name, true, true),
            "transparent"
        );
    }
    assert_eq!(
        morphforge_app::resources::alpha_render_mode("oak_leaves", true, true),
        "cutout"
    );
    assert_eq!(
        morphforge_app::resources::alpha_render_mode("iron_bars", false, true),
        "cutout"
    );
    assert_eq!(
        morphforge_app::resources::alpha_render_mode("stone", false, false),
        "opaque"
    );
}
