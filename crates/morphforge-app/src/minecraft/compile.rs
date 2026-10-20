use super::formats::{BlockState, Building};
use crate::store::Workspace;
use anyhow::{bail, Context, Result};
use morphforge_core::{materials::Material, Box3, Limits, Operation, Scene};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
fn scale() -> u32 {
    4
}
fn group() -> String {
    "minecraft".into()
}
fn yes() -> bool {
    true
}
fn pack() -> String {
    "builtin".into()
}
fn mode() -> String {
    "replace".into()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Options {
    #[serde(default = "scale")]
    pub block_scale: u32,
    #[serde(default = "group")]
    pub group: String,
    #[serde(default = "yes")]
    pub center: bool,
    #[serde(default)]
    pub offset: [i32; 3],
    #[serde(default)]
    pub surface_only: bool,
    #[serde(default = "pack")]
    pub resource_pack: String,
    #[serde(default = "mode")]
    pub mode: String,
    #[serde(skip)]
    pub state_materials: super::blockstates::Mapping,
    #[serde(skip)]
    pub state_warnings: Vec<String>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            block_scale: scale(),
            group: group(),
            center: true,
            offset: [0; 3],
            surface_only: false,
            resource_pack: pack(),
            mode: mode(),
            state_materials: Default::default(),
            state_warnings: Vec::new(),
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Compiled {
    pub scene: Scene,
    pub materials: Vec<Material>,
    pub report: Value,
    pub options: Options,
}
#[derive(Clone)]
struct Prepared {
    parts: Vec<Box3>,
    opaque_full: bool,
    unknown: bool,
    quantized: bool,
}
type Part = ([i32; 3], [i32; 3]);
fn part(min: [i32; 3], max: [i32; 3]) -> Part {
    (min, max)
}
fn facing(state: &BlockState) -> [i32; 2] {
    match state
        .properties
        .get("facing")
        .map(String::as_str)
        .unwrap_or("north")
    {
        "east" => [1, 0],
        "west" => [-1, 0],
        "south" => [0, 1],
        _ => [0, -1],
    }
}
fn connection(state: &BlockState, key: &str) -> bool {
    state
        .properties
        .get(key)
        .is_some_and(|v| v != "false" && v != "none")
}
fn shapes(state: &BlockState) -> (Vec<Part>, bool) {
    let name = state.name.split(':').nth(1).unwrap_or("");
    let p = &state.properties;
    if name.ends_with("_slab") {
        return (
            match p.get("type").map(String::as_str).unwrap_or("bottom") {
                "double" => vec![part([0; 3], [16; 3])],
                "top" => vec![part([0, 8, 0], [16; 3])],
                _ => vec![part([0; 3], [16, 8, 16])],
            },
            false,
        );
    }
    if name.ends_with("_stairs") {
        let top = p.get("half").is_some_and(|s| s == "top");
        let (base0, base1, step0, step1) = if top { (8, 16, 0, 8) } else { (0, 8, 8, 16) };
        let mut parts = vec![part([0, base0, 0], [16, base1, 16])];
        let direction = facing(state);
        let shape = p.get("shape").map(String::as_str).unwrap_or("straight");
        let left = [direction[1], -direction[0]];
        let side = if shape.ends_with("right") {
            [-left[0], -left[1]]
        } else {
            left
        };
        for x in [0, 8] {
            for z in [0, 8] {
                let front = (x + 4 - 8) * direction[0] + (z + 4 - 8) * direction[1] > 0;
                let sideways = (x + 4 - 8) * side[0] + (z + 4 - 8) * side[1] > 0;
                let keep = if shape.starts_with("inner_") {
                    front || sideways
                } else if shape.starts_with("outer_") {
                    front && sideways
                } else {
                    front
                };
                if keep {
                    parts.push(part([x, step0, z], [x + 8, step1, z + 8]));
                }
            }
        }
        return (parts, false);
    }
    if name.ends_with("_trapdoor") {
        if p.get("open").is_some_and(|v| v == "true") {
            let d = facing(state);
            let (mut lo, mut hi) = ([0; 3], [16; 3]);
            let a = if d[0] != 0 { 0 } else { 2 };
            if d[0] + d[1] > 0 {
                lo[a] = 13;
            } else {
                hi[a] = 3;
            }
            return (vec![part(lo, hi)], false);
        }
        return (
            if p.get("half").is_some_and(|s| s == "top") {
                vec![part([0, 13, 0], [16; 3])]
            } else {
                vec![part([0; 3], [16, 3, 16])]
            },
            false,
        );
    }
    if name.ends_with("_door") {
        let mut d = facing(state);
        if p.get("open").is_some_and(|v| v == "true") {
            d = if p.get("hinge").is_some_and(|v| v == "right") {
                [-d[1], d[0]]
            } else {
                [d[1], -d[0]]
            };
        }
        let (mut lo, mut hi) = ([0; 3], [16; 3]);
        let a = if d[0] != 0 { 0 } else { 2 };
        if d[0] + d[1] > 0 {
            lo[a] = 13;
        } else {
            hi[a] = 3;
        }
        return (vec![part(lo, hi)], false);
    }
    if name.ends_with("_fence") {
        let mut parts = vec![part([6, 0, 6], [10, 16, 10])];
        for (key, a, positive) in [
            ("east", 0, true),
            ("west", 0, false),
            ("south", 2, true),
            ("north", 2, false),
        ] {
            if connection(state, key) {
                for (y0, y1) in [(6, 9), (12, 15)] {
                    let (mut lo, mut hi) = ([7, y0, 7], [9, y1, 9]);
                    if positive {
                        hi[a] = 16;
                    } else {
                        lo[a] = 0;
                    }
                    parts.push(part(lo, hi));
                }
            }
        }
        return (parts, false);
    }
    if name.ends_with("_wall") {
        let mut parts = Vec::new();
        if p.get("up").is_none_or(|v| v != "false") {
            parts.push(part([4, 0, 4], [12, 16, 12]));
        }
        for (key, a, positive) in [
            ("east", 0, true),
            ("west", 0, false),
            ("south", 2, true),
            ("north", 2, false),
        ] {
            if connection(state, key) {
                let height = if p.get(key).is_some_and(|s| s == "tall") {
                    16
                } else {
                    14
                };
                let (mut lo, mut hi) = ([5, 0, 5], [11, height, 11]);
                if positive {
                    hi[a] = 16;
                } else {
                    lo[a] = 0;
                }
                parts.push(part(lo, hi));
            }
        }
        if parts.is_empty() {
            parts.push(part([4, 0, 4], [12, 16, 12]));
        }
        return (parts, false);
    }
    if name.ends_with("_pane") || name == "iron_bars" {
        let mut parts = vec![part([7, 0, 7], [9, 16, 9])];
        let any = ["north", "south", "east", "west"]
            .iter()
            .any(|k| connection(state, k));
        for (key, a, positive) in [
            ("east", 0, true),
            ("west", 0, false),
            ("south", 2, true),
            ("north", 2, false),
        ] {
            if !any || connection(state, key) {
                let (mut lo, mut hi) = ([7, 0, 7], [9, 16, 9]);
                if positive {
                    hi[a] = 16;
                } else {
                    lo[a] = 0;
                }
                parts.push(part(lo, hi));
            }
        }
        return (parts, false);
    }
    if name == "snow" {
        let layers = p
            .get("layers")
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(1)
            .clamp(1, 8);
        return (vec![part([0; 3], [16, layers * 2, 16])], false);
    }
    if name.ends_with("_carpet") {
        return (vec![part([0; 3], [16, 1, 16])], false);
    }
    if name == "farmland" || name == "dirt_path" {
        return (vec![part([0; 3], [16, 15, 16])], false);
    }
    if name == "chest" || name == "trapped_chest" || name == "ender_chest" {
        return (vec![part([1, 0, 1], [15, 14, 15])], false);
    }
    if name.ends_with("_bed") {
        return (
            vec![
                part([0, 3, 0], [16, 9, 16]),
                part([1, 0, 1], [4, 3, 4]),
                part([12, 0, 12], [15, 3, 15]),
            ],
            false,
        );
    }
    if name.ends_with("torch") {
        return (vec![part([6, 0, 6], [10, 10, 10])], true);
    }
    if name == "water" || name == "lava" {
        let level = p
            .get("level")
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(0);
        let height = if level == 0 || level >= 8 {
            16
        } else {
            (8 - level) * 2
        };
        return (vec![part([0; 3], [16, height, 16])], false);
    }
    let full = state.name.starts_with("minecraft:")
        && ([
            "_block",
            "_planks",
            "_log",
            "_wood",
            "_bricks",
            "_terracotta",
            "_concrete",
            "_concrete_powder",
            "_wool",
            "_ore",
            "_leaves",
            "_glass",
            "_sandstone",
            "_pillar",
            "_hyphae",
            "_stem",
        ]
        .iter()
        .any(|suffix| name.ends_with(suffix))
            || [
                "stone",
                "granite",
                "diorite",
                "andesite",
                "polished_granite",
                "polished_diorite",
                "polished_andesite",
                "deepslate",
                "cobbled_deepslate",
                "dirt",
                "coarse_dirt",
                "rooted_dirt",
                "podzol",
                "mycelium",
                "sand",
                "red_sand",
                "gravel",
                "clay",
                "terracotta",
                "cobblestone",
                "mossy_cobblestone",
                "bedrock",
                "obsidian",
                "crying_obsidian",
                "glass",
                "bricks",
                "sandstone",
                "quartz_block",
                "smooth_stone",
                "netherrack",
                "soul_sand",
                "soul_soil",
                "basalt",
                "smooth_basalt",
                "blackstone",
                "end_stone",
                "purpur_block",
                "bookshelf",
                "crafting_table",
                "furnace",
                "blast_furnace",
                "smoker",
                "tnt",
                "sponge",
                "wet_sponge",
                "ice",
                "packed_ice",
                "blue_ice",
                "melon",
                "pumpkin",
                "carved_pumpkin",
                "jack_o_lantern",
                "glowstone",
                "sea_lantern",
                "barrier",
                "moss_block",
                "hay_block",
                "coal_block",
            ]
            .contains(&name));
    (vec![part([0; 3], [16; 3])], !full)
}
fn material_name(state: &BlockState) -> String {
    let name = state.name.split(':').nth(1).unwrap_or("");
    if let Some(color) = name.strip_suffix("_carpet") {
        return format!("{color}_wool");
    }
    let stripped = name
        .strip_suffix("_stairs")
        .or_else(|| name.strip_suffix("_slab"))
        .or_else(|| name.strip_suffix("_wall"))
        .or_else(|| name.strip_suffix("_fence"))
        .unwrap_or(name);
    match stripped {
        "oak" | "spruce" | "birch" | "jungle" | "acacia" | "dark_oak" | "mangrove" | "cherry"
        | "bamboo" | "crimson" | "warped" => format!("{stripped}_planks"),
        "stone_brick" => "stone_bricks".into(),
        "mossy_stone_brick" => "mossy_stone_bricks".into(),
        "brick" => "bricks".into(),
        "nether_brick" => "nether_bricks".into(),
        "quartz" => "quartz_block".into(),
        _ => stripped.strip_suffix("_pane").unwrap_or(stripped).into(),
    }
}
fn builtin_key(name: &str) -> &'static str {
    if name.contains("spruce") {
        return if name.contains("log") || name.contains("wood") {
            "spruce_log"
        } else {
            "spruce_planks"
        };
    }
    if name.contains("birch") {
        return if name.contains("log") || name.contains("wood") {
            "birch_log"
        } else {
            "birch_planks"
        };
    }
    if name.contains("dark_oak") {
        return if name.contains("log") || name.contains("wood") {
            "dark_log"
        } else {
            "dark_planks"
        };
    }
    if name.contains("planks")
        || name.contains("door")
        || name.contains("fence")
        || name.contains("bookshelf")
        || name.contains("chest")
    {
        return "planks";
    }
    if name.contains("log") || name.contains("wood") || name.contains("stem") {
        return "wood";
    }
    if name.contains("deepslate") || name.contains("blackstone") {
        return "deepslate";
    }
    if name.contains("mossy_cobblestone") {
        return "mossy_cobble";
    }
    if name.contains("cobblestone") {
        return "cobble";
    }
    if name.contains("stone_brick") {
        return "grey_brick";
    }
    if name.contains("bricks") {
        return "brick";
    }
    if name.contains("glass") || name == "ice" {
        return "glass";
    }
    if name.contains("wool") {
        return "white_wool";
    }
    if name.contains("terracotta") {
        return "concrete";
    }
    if name.contains("grass") {
        return "grass";
    }
    if name.contains("dirt") {
        return "soil";
    }
    if name.contains("sandstone") {
        return "sandstone";
    }
    if name.contains("red_sand") {
        return "red_sand";
    }
    if name.contains("sand") {
        return "sand";
    }
    if name.contains("snow") {
        return "snow";
    }
    if name.contains("leaves") {
        return "leaves";
    }
    if name.contains("quartz") {
        return "quartz";
    }
    if name.contains("basalt") {
        return "basalt";
    }
    if name.contains("granite") {
        return "granite";
    }
    if name.contains("amethyst") {
        return "amethyst";
    }
    if name.contains("iron") {
        return "iron";
    }
    if name.contains("copper") {
        return "copper";
    }
    if name.contains("gold") {
        return "gold";
    }
    if name.contains("glow") || name.contains("lantern") {
        return "glow";
    }
    if name == "water" {
        return "water";
    }
    if name == "lava" {
        return "lava";
    }
    if name.contains("concrete") {
        return "concrete";
    }
    "stone"
}
fn allocate(materials: &[Material]) -> Result<u32> {
    let used: BTreeSet<_> = materials.iter().map(|m| m.id).collect();
    (64..=65535)
        .find(|id| !used.contains(id))
        .context("工程材质编号空间已满")
}
pub fn tile_variant(base: &Material, cells: u32) -> Material {
    let mut m = base.clone();
    if cells != 1 && !m.textures.is_empty() {
        m.id = 0;
        m.uid = format!("{}:mc-tiles-{cells}", base.uid);
        m.name = format!("{} · MC {cells}格/块", base.name);
        m.texture_scale = cells as f32;
    }
    m
}
fn orient_variant(mut material: Material, rotation: u8) -> Material {
    if rotation != 0 && !material.textures.is_empty() {
        material.id = 0;
        material.uid = format!("{}:mc-rotation-{rotation}", material.uid);
        material.texture_rotation = rotation;
    }
    material
}
pub fn prepare_builtin_variants(
    w: &mut Workspace,
    materials: &[Material],
) -> Result<Vec<Material>> {
    let (mut all, _, _) = crate::builtin_pixels::prepare_theme(w, "block-workshop", materials)?;
    let bases: Vec<_> = all
        .iter()
        .filter(|m| m.uid.starts_with("mf-pixel:"))
        .cloned()
        .collect();
    for cells in [2, 4, 8, 16] {
        for base in &bases {
            let mut variant = tile_variant(base, cells);
            if !all.iter().any(|m| m.uid == variant.uid) {
                variant.id = allocate(&all)?;
                all.push(variant);
            }
        }
    }
    for cells in [1, 2, 4, 8, 16] {
        for base in bases
            .iter()
            .filter(|m| m.uid == "mf-pixel:wood" || m.uid.ends_with("_log"))
        {
            for rotation in [4, 8] {
                let mut variant = orient_variant(tile_variant(base, cells), rotation);
                if !all.iter().any(|m| m.uid == variant.uid) {
                    variant.id = allocate(&all)?;
                    all.push(variant);
                }
            }
        }
    }
    for cells in [1, 2, 4, 8, 16] {
        for color in super::formats::COLORS {
            for suffix in ["wool", "carpet", "concrete", "terracotta", "stained_glass"] {
                let state = BlockState::parse(&format!("minecraft:{color}_{suffix}"))?;
                material(
                    &state,
                    &mut all,
                    &Options {
                        block_scale: cells,
                        ..Default::default()
                    },
                )?;
            }
        }
    }
    Ok(all)
}
fn material(
    state: &BlockState,
    materials: &mut Vec<Material>,
    options: &Options,
) -> Result<(u32, bool)> {
    let name = state.name.split(':').nth(1).unwrap_or("");
    let base_name = material_name(state);
    let ns = state.name.split(':').next().unwrap_or("minecraft");
    let mut approximate = options.resource_pack == "builtin";
    let base = if options.resource_pack == "builtin" {
        let key = builtin_key(&base_name);
        materials
            .iter()
            .find(|m| m.uid == format!("mf-pixel:{key}"))
            .or_else(|| materials.iter().find(|m| m.uid == "mf-pixel:stone"))
            .cloned()
    } else {
        let candidates = [
            options
                .state_materials
                .get(&state.canonical())
                .map(|(name, _)| name.clone())
                .unwrap_or_else(|| format!("{ns}:block/{name}")),
            format!("{ns}:block/{base_name}"),
        ];
        let exact = materials
            .iter()
            .find(|m| {
                m.pack.as_deref() == Some(options.resource_pack.as_str())
                    && candidates
                        .iter()
                        .any(|c| m.uid.split('@').next() == Some(c.as_str()))
            })
            .cloned();
        if exact.is_none() {
            approximate = true;
        }
        exact.or_else(|| {
            materials
                .iter()
                .find(|m| m.uid == format!("mf-pixel:{}", builtin_key(&base_name)))
                .cloned()
        })
    }
    .context("无法找到建筑材质，请先加入内置方块工坊或所选资源包")?;
    let rotation = options
        .state_materials
        .get(&state.canonical())
        .map(|(_, rotation)| *rotation)
        .unwrap_or_else(|| match state.properties.get("axis").map(String::as_str) {
            Some("x") => 4,
            Some("z") => 8,
            _ => {
                if name.ends_with("_stairs") || name.ends_with("_wall") || name.ends_with("_fence")
                {
                    0
                } else {
                    match state.properties.get("facing").map(String::as_str) {
                        Some("east") => 1,
                        Some("south") => 2,
                        Some("west") => 3,
                        Some("up") => 8,
                        Some("down") => 12,
                        _ => 0,
                    }
                }
            }
        });
    let mut variant = orient_variant(tile_variant(&base, options.block_scale), rotation);
    // Coloring a shared original wool/glass tile does not allocate a texture.
    let dyes = [
        ("white", [240, 240, 240]),
        ("orange", [249, 128, 29]),
        ("magenta", [199, 78, 189]),
        ("light_blue", [58, 179, 218]),
        ("yellow", [254, 216, 61]),
        ("lime", [128, 199, 31]),
        ("pink", [243, 139, 170]),
        ("gray", [71, 79, 82]),
        ("light_gray", [157, 157, 151]),
        ("cyan", [22, 156, 156]),
        ("purple", [137, 50, 184]),
        ("blue", [60, 68, 170]),
        ("brown", [131, 84, 50]),
        ("green", [94, 124, 22]),
        ("red", [176, 46, 38]),
        ("black", [29, 29, 33]),
    ];
    if options.resource_pack == "builtin"
        && (name.ends_with("_wool")
            || name.ends_with("_carpet")
            || name.ends_with("_concrete")
            || name.contains("stained_glass")
            || name.ends_with("_terracotta"))
    {
        if let Some((dye, rgb)) = dyes
            .iter()
            .find(|(dye, _)| name.starts_with(&format!("{dye}_")))
        {
            variant.color = rgb.map(|v| v as f32 / 255.);
            variant.uid = format!("{}:dye-{dye}", variant.uid);
            variant.id = 0;
        }
    }
    if let Some(existing) = materials.iter().find(|m| m.uid == variant.uid) {
        return Ok((existing.id, approximate));
    }
    variant.id = allocate(materials)?;
    let id = variant.id;
    materials.push(variant);
    Ok((id, approximate))
}
pub fn compile(w: &mut Workspace, building: &Building, mut options: Options) -> Result<Compiled> {
    (options.state_materials, options.state_warnings) =
        super::blockstates::resolve(&w.root, &options.resource_pack, &building.palette)?;
    let originals = w.project.materials.clone();
    let (materials, _, _) = crate::builtin_pixels::prepare_theme(w, "block-workshop", &originals)?;
    compile_prepared(
        &w.project.scene,
        &w.project.limits,
        materials,
        originals.len(),
        building,
        options,
    )
}
/// CPU-heavy geometry compilation is independent of the live workspace lock.
pub fn compile_prepared(
    source_scene: &Scene,
    limits: &Limits,
    mut materials: Vec<Material>,
    original_material_count: usize,
    building: &Building,
    options: Options,
) -> Result<Compiled> {
    if ![1, 2, 4, 8, 16].contains(&options.block_scale)
        || options.group.is_empty()
        || options.group.len() > 128
        || !["replace", "overlay"].contains(&options.mode.as_str())
    {
        bail!("建筑选项需要blockScale=1/2/4/8/16、有效分组及replace/overlay");
    }
    if options.resource_pack != "builtin"
        && !materials
            .iter()
            .any(|m| m.pack.as_deref() == Some(options.resource_pack.as_str()))
    {
        bail!("选定资源包尚未导入当前工程，请先导入资源包");
    }
    let used: BTreeSet<_> = building.blocks.iter().map(|b| b.state).collect();
    let mut prepared = BTreeMap::new();
    let mut quantized = Vec::new();
    let mut unknown = Vec::new();
    let mut approximate_materials = Vec::new();
    for index in used {
        let state = &building.palette[index];
        let (style, approximate) = material(state, &mut materials, &options)?;
        let (parts, unsupported) = shapes(state);
        let mut ops = Vec::new();
        let mut rounded = false;
        for (lo, hi) in parts {
            let mut min = [0; 3];
            let mut size = [0; 3];
            for a in 0..3 {
                let left = lo[a] as f64 * options.block_scale as f64 / 16.;
                let right = hi[a] as f64 * options.block_scale as f64 / 16.;
                rounded |= left.fract() != 0. || right.fract() != 0.;
                min[a] = left.round() as i32;
                let end = (right.round() as i32).max(min[a] + 1);
                size[a] = (end - min[a]) as u32;
            }
            let mut b = Box3::new(min, size, style);
            b.group = options.group.clone();
            b.mergeable = materials
                .iter()
                .find(|m| m.id == style)
                .is_none_or(|m| m.texture_scale != 0.);
            ops.push(Operation::Box { value: b });
        }
        let local = Scene::default().apply(&ops, &Limits::default())?;
        let r = options.block_scale;
        let full = local.boxes.len() == 1
            && local.boxes[0].min == [0; 3]
            && local.boxes[0].size == [r; 3]
            && materials
                .iter()
                .find(|m| m.id == style)
                .is_some_and(|m| m.render_mode == "opaque" && m.opacity >= 1.);
        if rounded {
            quantized.push(state.canonical());
        }
        if unsupported {
            unknown.push(state.canonical());
        }
        if approximate {
            approximate_materials.push(state.canonical());
        }
        prepared.insert(
            index,
            Prepared {
                parts: local.boxes,
                opaque_full: full,
                unknown: unsupported,
                quantized: rounded,
            },
        );
    }
    let r = options.block_scale as i64;
    let mut offset = options.offset.map(i64::from);
    if options.center {
        if let Some((min, max)) = building.bounds {
            for a in 0..3 {
                offset[a] -= min[a] as i64 * r;
                if a != 1 {
                    offset[a] -= (max[a] as i64 - min[a] as i64) * r / 2;
                }
            }
        }
    }
    let lookup: HashMap<_, _> = building
        .blocks
        .iter()
        .map(|b| (b.position, b.state))
        .collect();
    let mut boxes = Vec::new();
    let mut hidden = 0usize;
    let mut unknown_blocks = 0usize;
    let mut quantized_blocks = 0usize;
    for block in &building.blocks {
        let state = &prepared[&block.state];
        if options.surface_only
            && state.opaque_full
            && [
                [1, 0, 0],
                [-1, 0, 0],
                [0, 1, 0],
                [0, -1, 0],
                [0, 0, 1],
                [0, 0, -1],
            ]
            .iter()
            .all(|d| {
                lookup
                    .get(&std::array::from_fn(|a| block.position[a] + d[a]))
                    .is_some_and(|index| prepared[index].opaque_full)
            })
        {
            hidden += 1;
            continue;
        }
        unknown_blocks += usize::from(state.unknown);
        quantized_blocks += usize::from(state.quantized);
        for part in &state.parts {
            let mut b = part.clone();
            b.id = 0;
            for a in 0..3 {
                b.min[a] =
                    i32::try_from(block.position[a] as i64 * r + part.min[a] as i64 + offset[a])
                        .context("建筑位置溢出")?;
            }
            if boxes.len() >= 2_097_152 {
                bail!("建筑展开超过2097152个临时对象，请使用外壳或裁剪源建筑后重试；原文件仍保留");
            }
            boxes.push(b);
        }
    }
    let unmerged = boxes.len();
    let mut boxes = morphforge_core::geometry::merge(boxes);
    for (i, b) in boxes.iter_mut().enumerate() {
        b.id = i as u64 + 1;
    }
    let mut scene = Scene {
        format: "MorphForgeScene/2".into(),
        boxes,
        cell_size: source_scene.cell_size,
        revision: source_scene.revision,
        ..Default::default()
    };
    scene.validate(limits)?;
    if options.mode == "overlay" {
        let operations: Vec<_> = scene
            .boxes
            .into_iter()
            .map(|mut value| {
                value.id = 0;
                Operation::Box { value }
            })
            .collect();
        scene = source_scene.clone();
        for chunk in operations.chunks(limits.max_ops.max(1)) {
            scene = scene.apply(chunk, limits)?;
        }
    }
    let stats = scene.stats(limits);
    let mut warnings = building.warnings.clone();
    warnings.extend(options.state_warnings.clone());
    if !unknown.is_empty() {
        warnings.push(
            "部分形状使用整块或近似几何替代；完整方块状态保存在源建筑中，可重新导入。".into(),
        );
    }
    if !quantized.is_empty() {
        warnings.push(
            "部分细节小于当前格精度，已量化为至少一格；提高blockScale可保留更多细节。".into(),
        );
    }
    if options.resource_pack == "builtin" {
        warnings.push(
            "使用原创Minecraft风格材质近似，不包含Mojang原版贴图；可选择已导入的资源包。".into(),
        );
    }
    if options.surface_only {
        warnings.push("本次只导入外壳；内部格未进入可编辑场景，源建筑仍完整保存。".into());
    }
    let report = json!({"format":building.format,"dataVersion":building.data_version,"sourceBlocks":building.blocks.len(),"sourceCells":building.source_cells,"sourceBounds":building.bounds,"regions":building.regions,"blockScale":options.block_scale,"center":options.center,"translation":offset,"beforeMerge":unmerged,"generatedBoxes":stats.box_count,"hiddenSourceBlocks":hidden,"approximateShapeBlocks":unknown_blocks,"quantizedBlocks":quantized_blocks,"unknownShapes":unknown.iter().take(128).collect::<Vec<_>>(),"quantizedStates":quantized.iter().take(128).collect::<Vec<_>>(),"approximateMaterialStates":approximate_materials.len(),"approximateMaterials":approximate_materials.iter().take(128).collect::<Vec<_>>(),"paletteCount":building.palette.len(),"materialsAdded":materials.len().saturating_sub(original_material_count),"stats":stats,"target":crate::scene_tools::target_report(&scene,limits.target_boxes),"estimatedFrames":scene.boxes.len().div_ceil(3),"estimatedSecondsAt250ms":scene.boxes.len().div_ceil(3) as f64*0.25,"estimateNote":"粗略按3Box/帧计算；实际由共外观、单位体素批量与池分布决定，无远端ACK。","warnings":warnings});
    Ok(Compiled {
        scene,
        materials,
        report,
        options,
    })
}

#[cfg(test)]
mod tests {
    use super::super::formats::{Block, Region};
    use super::*;
    fn building(name: &str, positions: Vec<[i32; 3]>) -> Building {
        let min = std::array::from_fn(|a| positions.iter().map(|p| p[a]).min().unwrap());
        let max = std::array::from_fn(|a| positions.iter().map(|p| p[a] + 1).max().unwrap());
        Building {
            format: "fixture".into(),
            data_version: Some(3955),
            palette: vec![BlockState::parse(name).unwrap()],
            blocks: positions
                .into_iter()
                .map(|position| Block { position, state: 0 })
                .collect(),
            regions: vec![Region {
                name: "fixture".into(),
                origin: [0; 3],
                size: [1; 3],
            }],
            bounds: Some((min, max)),
            entities: 0,
            block_entities: 0,
            overwritten_cells: 0,
            source_cells: 1,
            warnings: Vec::new(),
        }
    }
    #[test]
    fn full_blocks_merge_and_preview_does_not_mutate_project() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = Workspace::open(dir.path().into()).unwrap();
        let before = serde_json::to_value(&w.project).unwrap();
        let b = building(
            "minecraft:stone",
            vec![[1000, 20, -50], [1001, 20, -50], [1002, 20, -50]],
        );
        let c = compile(&mut w, &b, Options::default()).unwrap();
        assert_eq!(c.scene.boxes.len(), 1);
        assert_eq!(c.scene.boxes[0].size, [12, 4, 4]);
        assert_eq!(c.scene.boxes[0].min, [-6, 0, -2]);
        let m = c
            .materials
            .iter()
            .find(|m| m.id == c.scene.boxes[0].style_id)
            .unwrap();
        assert_eq!(m.texture_scale, 4.);
        assert_eq!(serde_json::to_value(&w.project).unwrap(), before);
    }
    #[test]
    fn stair_shapes_have_independent_dense_cell_volume() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = Workspace::open(dir.path().into()).unwrap();
        for (shape, volume) in [
            ("straight", 48),
            ("inner_left", 56),
            ("inner_right", 56),
            ("outer_left", 40),
            ("outer_right", 40),
        ] {
            for facing in ["north", "south", "east", "west"] {
                for half in ["bottom", "top"] {
                    let b = building(
                        &format!("minecraft:oak_stairs[shape={shape},facing={facing},half={half}]"),
                        vec![[0; 3]],
                    );
                    let c = compile(
                        &mut w,
                        &b,
                        Options {
                            center: false,
                            ..Default::default()
                        },
                    )
                    .unwrap();
                    assert_eq!(
                        c.scene.stats(&w.project.limits).voxel_count,
                        volume,
                        "{shape}/{facing}/{half}"
                    );
                    assert_eq!(c.report["quantizedBlocks"], 0);
                }
            }
        }
    }
    #[test]
    fn thin_geometry_is_visible_and_quantization_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = Workspace::open(dir.path().into()).unwrap();
        let b = building("minecraft:red_carpet", vec![[0; 3]]);
        let c = compile(
            &mut w,
            &b,
            Options {
                center: false,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(c.scene.stats(&w.project.limits).voxel_count, 16);
        assert_eq!(c.report["quantizedBlocks"], 1);
        let m = c
            .materials
            .iter()
            .find(|m| m.id == c.scene.boxes[0].style_id)
            .unwrap();
        assert!(m.color[0] > m.color[1]);
    }
    #[test]
    fn explicit_shell_keeps_original_and_unknown_blocks_are_diagnosed() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = Workspace::open(dir.path().into()).unwrap();
        let mut positions = Vec::new();
        for x in 0..3 {
            for y in 0..3 {
                for z in 0..3 {
                    positions.push([x, y, z]);
                }
            }
        }
        let b = building("minecraft:stone", positions);
        let c = compile(
            &mut w,
            &b,
            Options {
                surface_only: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(c.report["hiddenSourceBlocks"], 1);
        assert_eq!(b.blocks.len(), 27);
        let b = building("example:unknown[detail=kept]", vec![[0; 3]]);
        let c = compile(&mut w, &b, Options::default()).unwrap();
        assert_eq!(c.report["approximateShapeBlocks"], 1);
        assert_eq!(b.palette[0].properties["detail"], "kept");
    }
}
