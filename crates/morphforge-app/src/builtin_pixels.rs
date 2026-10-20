//! Original deterministic pixel textures. No Minecraft/Mojang assets are redistributed.
use crate::store::Workspace;
use anyhow::{bail, Context, Result};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use morphforge_core::materials::Material;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Cursor,
};
pub const LICENSE: &str = include_str!("../../../materials/PIXEL-WORKSHOP-LICENSE.txt");
struct Spec {
    key: &'static str,
    name: &'static str,
    family: &'static str,
    color: [u8; 3],
    base: u32,
    metal: f32,
    rough: f32,
    opacity: f32,
    emission: f32,
    mode: &'static str,
}
fn specs() -> Vec<Spec> {
    [
        (
            "stone",
            "像素石材",
            "natural",
            [137, 145, 151],
            1,
            0.,
            0.9,
            1.,
            0.,
            "opaque",
        ),
        (
            "wood",
            "像素原木",
            "wood",
            [157, 106, 57],
            2,
            0.,
            0.8,
            1.,
            0.,
            "opaque",
        ),
        (
            "grass",
            "像素草地",
            "natural",
            [87, 148, 62],
            4,
            0.,
            0.9,
            1.,
            0.,
            "opaque",
        ),
        (
            "soil",
            "像素泥土",
            "natural",
            [120, 83, 57],
            5,
            0.,
            1.,
            1.,
            0.,
            "opaque",
        ),
        (
            "sand",
            "像素细沙",
            "natural",
            [218, 199, 144],
            6,
            0.,
            0.9,
            1.,
            0.,
            "opaque",
        ),
        (
            "snow",
            "像素积雪",
            "natural",
            [230, 241, 246],
            7,
            0.,
            0.8,
            1.,
            0.,
            "opaque",
        ),
        (
            "brick",
            "像素红砖",
            "building",
            [167, 81, 61],
            8,
            0.,
            0.9,
            1.,
            0.,
            "opaque",
        ),
        (
            "concrete",
            "像素白石",
            "building",
            [203, 205, 197],
            9,
            0.,
            0.85,
            1.,
            0.,
            "opaque",
        ),
        (
            "planks",
            "像素木板",
            "wood",
            [181, 130, 73],
            0,
            0.,
            0.8,
            1.,
            0.,
            "opaque",
        ),
        (
            "cobble",
            "像素卵石",
            "natural",
            [112, 123, 126],
            0,
            0.,
            1.,
            1.,
            0.,
            "opaque",
        ),
        (
            "iron",
            "像素铁块",
            "metal",
            [186, 199, 207],
            0,
            0.85,
            0.28,
            1.,
            0.,
            "opaque",
        ),
        (
            "copper",
            "像素铜块",
            "metal",
            [190, 112, 69],
            0,
            0.8,
            0.35,
            1.,
            0.,
            "opaque",
        ),
        (
            "gold",
            "像素金块",
            "metal",
            [237, 190, 64],
            0,
            0.9,
            0.2,
            1.,
            0.,
            "opaque",
        ),
        (
            "water",
            "像素流水",
            "water",
            [64, 143, 204],
            0,
            0.05,
            0.18,
            0.55,
            0.,
            "transparent",
        ),
        (
            "glass",
            "像素玻璃",
            "glass",
            [177, 220, 233],
            0,
            0.05,
            0.15,
            0.28,
            0.,
            "transparent",
        ),
        (
            "leaves",
            "像素树叶",
            "foliage",
            [63, 125, 63],
            0,
            0.,
            0.9,
            1.,
            0.,
            "cutout",
        ),
        (
            "glow",
            "像素灯石",
            "emissive",
            [255, 216, 135],
            0,
            0.,
            0.55,
            1.,
            1.8,
            "opaque",
        ),
        (
            "cloud",
            "像素云团",
            "cloud",
            [236, 244, 250],
            0,
            0.,
            1.,
            0.72,
            0.,
            "transparent",
        ),
        (
            "obsidian",
            "像素黑曜石",
            "natural",
            [48, 38, 67],
            0,
            0.15,
            0.35,
            1.,
            0.,
            "opaque",
        ),
    ]
    .into_iter()
    .map(
        |(key, name, family, color, base, metal, rough, opacity, emission, mode)| Spec {
            key,
            name,
            family,
            color,
            base,
            metal,
            rough,
            opacity,
            emission,
            mode,
        },
    )
    .collect()
}
fn block_specs() -> Vec<Spec> {
    let mut all = specs();
    all.extend([
        Spec {
            key: "spruce_log",
            name: "云杉原木",
            family: "wood",
            color: [104, 77, 48],
            base: 0,
            metal: 0.0f32,
            rough: 0.9f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "opaque",
        },
        Spec {
            key: "birch_log",
            name: "白桦原木",
            family: "wood",
            color: [207, 201, 179],
            base: 0,
            metal: 0.0f32,
            rough: 0.9f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "opaque",
        },
        Spec {
            key: "dark_log",
            name: "深橡原木",
            family: "wood",
            color: [72, 49, 33],
            base: 0,
            metal: 0.0f32,
            rough: 0.9f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "opaque",
        },
        Spec {
            key: "spruce_planks",
            name: "云杉木板",
            family: "wood",
            color: [126, 91, 52],
            base: 0,
            metal: 0.0f32,
            rough: 0.9f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "opaque",
        },
        Spec {
            key: "birch_planks",
            name: "白桦木板",
            family: "wood",
            color: [220, 203, 144],
            base: 0,
            metal: 0.0f32,
            rough: 0.9f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "opaque",
        },
        Spec {
            key: "dark_planks",
            name: "深色木板",
            family: "wood",
            color: [84, 58, 38],
            base: 0,
            metal: 0.0f32,
            rough: 0.9f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "opaque",
        },
        Spec {
            key: "mossy_cobble",
            name: "苔石",
            family: "natural",
            color: [98, 117, 83],
            base: 0,
            metal: 0.0f32,
            rough: 0.9f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "opaque",
        },
        Spec {
            key: "deepslate",
            name: "深板岩",
            family: "natural",
            color: [66, 72, 76],
            base: 0,
            metal: 0.0f32,
            rough: 0.9f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "opaque",
        },
        Spec {
            key: "granite",
            name: "花岗岩",
            family: "natural",
            color: [158, 115, 99],
            base: 0,
            metal: 0.0f32,
            rough: 0.9f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "opaque",
        },
        Spec {
            key: "marble",
            name: "纹理大理石",
            family: "natural",
            color: [222, 219, 209],
            base: 0,
            metal: 0.0f32,
            rough: 0.9f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "opaque",
        },
        Spec {
            key: "basalt",
            name: "玄武岩",
            family: "natural",
            color: [66, 65, 72],
            base: 0,
            metal: 0.0f32,
            rough: 0.9f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "opaque",
        },
        Spec {
            key: "red_sand",
            name: "红沙",
            family: "natural",
            color: [182, 103, 58],
            base: 0,
            metal: 0.0f32,
            rough: 0.9f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "opaque",
        },
        Spec {
            key: "sandstone",
            name: "砂岩砖",
            family: "building",
            color: [205, 184, 133],
            base: 0,
            metal: 0.0f32,
            rough: 0.9f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "opaque",
        },
        Spec {
            key: "quartz",
            name: "石英块",
            family: "building",
            color: [236, 228, 216],
            base: 0,
            metal: 0.0f32,
            rough: 0.9f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "opaque",
        },
        Spec {
            key: "blue_brick",
            name: "蓝灰砖",
            family: "building",
            color: [81, 107, 130],
            base: 0,
            metal: 0.0f32,
            rough: 0.9f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "opaque",
        },
        Spec {
            key: "grey_brick",
            name: "灰石砖",
            family: "building",
            color: [113, 120, 123],
            base: 0,
            metal: 0.0f32,
            rough: 0.9f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "opaque",
        },
        Spec {
            key: "tiles",
            name: "深色瓦",
            family: "building",
            color: [78, 74, 76],
            base: 0,
            metal: 0.0f32,
            rough: 0.9f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "opaque",
        },
        Spec {
            key: "blue_glass",
            name: "蓝玻璃",
            family: "glass",
            color: [92, 147, 214],
            base: 0,
            metal: 0.2f32,
            rough: 0.1f32,
            opacity: 0.4f32,
            emission: 0.,
            mode: "transparent",
        },
        Spec {
            key: "green_glass",
            name: "绿玻璃",
            family: "glass",
            color: [109, 189, 139],
            base: 0,
            metal: 0.2f32,
            rough: 0.1f32,
            opacity: 0.4f32,
            emission: 0.,
            mode: "transparent",
        },
        Spec {
            key: "red_glass",
            name: "红玻璃",
            family: "glass",
            color: [214, 108, 105],
            base: 0,
            metal: 0.2f32,
            rough: 0.1f32,
            opacity: 0.4f32,
            emission: 0.,
            mode: "transparent",
        },
        Spec {
            key: "moss",
            name: "绒苔",
            family: "foliage",
            color: [77, 118, 51],
            base: 0,
            metal: 0.0f32,
            rough: 0.9f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "opaque",
        },
        Spec {
            key: "dark_leaves",
            name: "深绿树叶",
            family: "foliage",
            color: [38, 83, 42],
            base: 0,
            metal: 0.0f32,
            rough: 0.9f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "cutout",
        },
        Spec {
            key: "amethyst",
            name: "紫晶",
            family: "natural",
            color: [150, 100, 190],
            base: 0,
            metal: 0.15f32,
            rough: 0.3f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "opaque",
        },
        Spec {
            key: "red_wool",
            name: "朱红羊毛",
            family: "matte",
            color: [180, 61, 51],
            base: 0,
            metal: 0.0f32,
            rough: 0.9f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "opaque",
        },
        Spec {
            key: "blue_wool",
            name: "靛蓝羊毛",
            family: "matte",
            color: [62, 88, 161],
            base: 0,
            metal: 0.0f32,
            rough: 0.9f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "opaque",
        },
        Spec {
            key: "white_wool",
            name: "白羊毛",
            family: "matte",
            color: [223, 221, 211],
            base: 0,
            metal: 0.0f32,
            rough: 0.9f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "opaque",
        },
        Spec {
            key: "black_wool",
            name: "黑羊毛",
            family: "matte",
            color: [45, 43, 48],
            base: 0,
            metal: 0.0f32,
            rough: 0.9f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "opaque",
        },
        Spec {
            key: "red_terracotta",
            name: "赭红陶土",
            family: "building",
            color: [149, 78, 57],
            base: 0,
            metal: 0.0f32,
            rough: 0.9f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "opaque",
        },
        Spec {
            key: "blue_terracotta",
            name: "青蓝陶土",
            family: "building",
            color: [78, 103, 123],
            base: 0,
            metal: 0.0f32,
            rough: 0.9f32,
            opacity: 1.0f32,
            emission: 0.,
            mode: "opaque",
        },
    ]);
    all
}
fn texture(s: &Spec, face: usize) -> Result<Vec<u8>> {
    let frames = if s.key == "water" { 4 } else { 1 };
    let mut image = RgbaImage::new(16, 16 * frames);
    for frame in 0..frames {
        for y in 0..16 {
            for x in 0..16 {
                let noise = ((x * 73 + y * 151 + (x * y) * 17 + frame * 37) % 37) as i32 - 18;
                let mut delta = noise / 2;
                let mut color = s.color;
                let mut alpha = 255;
                let pattern = if s.key.ends_with("_log") {
                    "wood"
                } else if s.key.ends_with("_planks") {
                    "planks"
                } else if s.key.ends_with("_glass") {
                    "glass"
                } else if s.key.ends_with("_brick") || s.key == "sandstone" {
                    "brick"
                } else if s.key == "dark_leaves" {
                    "leaves"
                } else if s.key == "mossy_cobble" {
                    "cobble"
                } else {
                    s.key
                };
                match pattern {
                    "wood" => {
                        delta += if face == 2 || face == 3 {
                            let d = (x as i32 - 7).abs().max((y as i32 - 7).abs());
                            if d % 3 == 0 {
                                -28
                            } else {
                                8
                            }
                        } else {
                            if (x + y / 5) % 5 == 0 {
                                -25
                            } else {
                                7
                            }
                        };
                    }
                    "planks" => {
                        if y % 4 == 0 || ((x + y / 4 * 7) % 16 == 0) {
                            delta -= 32;
                        }
                    }
                    "brick" => {
                        if y % 5 == 0 || (x + if (y / 5) % 2 == 0 { 0 } else { 4 }) % 8 == 0 {
                            color = [193, 180, 157];
                            delta = noise / 3;
                        }
                    }
                    "cobble" => {
                        if ((x / 4 * 3 + y / 4) % 4 == 0 && x % 4 == 0) || y % 4 == 0 {
                            delta -= 36;
                        }
                    }
                    "grass" => {
                        if face == 3 || (face != 2 && y > 3 + (x * 3 % 3)) {
                            color = [119, 82, 54];
                        } else if (x + y) % 7 == 0 {
                            delta += 20;
                        }
                    }
                    "glass" => {
                        alpha = if x == 0 || y == 0 || x == 15 || y == 15 {
                            190
                        } else {
                            90
                        };
                        if x + y == 13 || x + y == 14 {
                            delta += 35;
                        }
                    }
                    "water" => {
                        delta += (((x + frame * 2 + y / 3) % 8) as i32 - 4) * 3;
                    }
                    "leaves" => {
                        if (x * 7 + y * 11 + x * y) % 13 < 3 {
                            alpha = 0;
                        }
                        delta += if (x + y) % 4 == 0 { 15 } else { -4 };
                    }
                    "iron" | "copper" | "gold" => {
                        delta += if x == 0 || y == 0 {
                            25
                        } else if x == 15 || y == 15 {
                            -30
                        } else {
                            5
                        };
                    }
                    "glow" if (x * 11 + y * 7) % 7 < 3 => {
                        delta -= 55;
                    }
                    _ => {}
                }
                image.put_pixel(
                    x,
                    y + frame * 16,
                    Rgba([
                        color[0].saturating_add_signed(delta as i8),
                        color[1].saturating_add_signed(delta as i8),
                        color[2].saturating_add_signed(delta as i8),
                        alpha,
                    ]),
                );
            }
        }
    }
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image).write_to(&mut out, ImageFormat::Png)?;
    Ok(out.into_inner())
}
pub fn list() -> Value {
    json!([{"id":"pixel-workshop","name":"像素工坊","version":"1.0","license":"MIT","author":"MorphForge contributors","materialCount":specs().len(),"description":"原有19种原创像素主题。"},{"id":"block-workshop","name":"方块工坊","version":"1.0","license":"MIT","author":"MorphForge contributors","materialCount":block_specs().len(),"description":"48种原创Minecraft风格16×16像素方块：木石砖瓦、矿物、彩玻璃、羊毛与陶土。MF6头像已内置；不含Mojang素材。"}])
}
/// Prepare immutable artwork materials and texture artifacts without changing
/// the project or its revision. Import previews commit them only on apply.
pub fn prepare_theme(
    w: &mut Workspace,
    theme: &str,
    base_materials: &[Material],
) -> Result<(Vec<Material>, BTreeMap<u32, u32>, usize)> {
    if !["pixel-workshop", "block-workshop"].contains(&theme) {
        bail!("未知内置主题");
    }
    let mut materials = base_materials.to_vec();
    let mut used: BTreeSet<_> = materials.iter().map(|m| m.id).collect();
    let mut mapping = BTreeMap::new();
    let mut added = 0;
    for spec in if theme == "block-workshop" {
        block_specs()
    } else {
        specs()
    } {
        let uid = format!("mf-pixel:{}", spec.key);
        if let Some(existing) = materials.iter().find(|m| m.uid == uid) {
            mapping.insert(spec.base, existing.id);
            continue;
        }
        let id = (64..=65535)
            .find(|id| !used.contains(id))
            .context("材质编号空间已满")?;
        used.insert(id);
        let mut textures = vec![];
        for face in 0..6 {
            let png = texture(&spec, face)?;
            textures.push(format!("/artifacts/{}", w.artifact(&png, "png")?));
        }
        materials.push(Material {
            texture_scale: 1.0,
            texture_rotation: 0,
            id,
            uid,
            name: spec.name.into(),
            family: spec.family.into(),
            color: [1.; 3],
            metallic: spec.metal,
            roughness: spec.rough,
            opacity: spec.opacity,
            emission: spec.emission,
            render_mode: spec.mode.into(),
            textures,
            frames: if spec.key == "water" { 4 } else { 1 },
            frame_time: 0.1,
            pack: Some("pixel-workshop/1.0".into()),
            source: Some("MorphForge original procedural pixel textures; MIT".into()),
            procedural: None,
        });
        mapping.insert(spec.base, id);
        added += 1;
    }
    Ok((materials, mapping, added))
}
pub fn apply(w: &mut Workspace, args: &Value, run: Option<&str>) -> Result<Value> {
    let base = args["baseRevision"]
        .as_u64()
        .context("加入主题需要baseRevision")?;
    if base != w.project.scene.revision {
        bail!("REVISION_CONFLICT");
    }
    if ![Some("pixel-workshop"), Some("block-workshop")].contains(&args["id"].as_str()) {
        bail!("未知内置主题");
    }
    let source_materials = w.project.materials.clone();
    let (materials, mapping, added) =
        prepare_theme(w, args["id"].as_str().unwrap(), &source_materials)?;
    let mut scene = w.project.scene.clone();
    let mut changed = 0;
    if args["replaceBaseMaterials"] == true {
        for b in &mut scene.boxes {
            if b.style_id != 0
                && w.project
                    .materials
                    .iter()
                    .any(|m| m.id == b.style_id && m.uid.starts_with("mf:"))
            {
                if let Some(id) = mapping.get(&b.style_id) {
                    b.style_id = *id;
                    changed += 1;
                }
            }
        }
    }
    if added == 0 && changed == 0 {
        return Ok(json!({"ok":true,"unchanged":true,"revision":base,"materialsAdded":0}));
    }
    let result = w.replace_with_materials(scene, materials, Some(base), run)?;
    w.event(
        "pack.builtin.applied",
        run,
        json!({"id":args["id"],"license":"MIT","materialsAdded":added,"repaintedBoxes":changed}),
    )?;
    Ok(
        json!({"ok":true,"revision":result["revision"],"materialsAdded":added,"repaintedBoxes":changed,"license":"MIT","note":"主题已加入；配套MF6头像内置方块工坊，可直接按身份映射。额外新图片需要更新目录。"}),
    )
}
