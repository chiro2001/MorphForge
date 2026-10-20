use super::nbt::{self, Tag};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
const MAX_CELLS: usize = 2_097_152;
const MAX_PALETTE: usize = 65536;
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockState {
    pub name: String,
    pub properties: BTreeMap<String, String>,
}
impl BlockState {
    pub fn air(&self) -> bool {
        [
            "minecraft:air",
            "minecraft:cave_air",
            "minecraft:void_air",
            "minecraft:structure_void",
        ]
        .contains(&self.name.as_str())
    }
    pub fn canonical(&self) -> String {
        if self.properties.is_empty() {
            self.name.clone()
        } else {
            format!(
                "{}[{}]",
                self.name,
                self.properties
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
    }
    pub fn parse(value: &str) -> Result<Self> {
        if value.len() > 4096 {
            bail!("方块状态名称过长");
        }
        let (name, properties) = if let Some((name, properties)) = value.split_once('[') {
            (
                name,
                Some(properties.strip_suffix(']').context("方块状态缺少]")?),
            )
        } else {
            (value, None)
        };
        let name = if name.contains(':') {
            name.to_owned()
        } else {
            format!("minecraft:{name}")
        };
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_:-/.$".contains(&b))
        {
            bail!("无效的方块资源名称");
        }
        let mut out = Self {
            name,
            properties: BTreeMap::new(),
        };
        if let Some(properties) = properties {
            for pair in properties.split(',').filter(|v| !v.is_empty()) {
                let (k, v) = pair.split_once('=').context("方块属性需要key=value")?;
                if k.is_empty()
                    || v.is_empty()
                    || k.len() > 128
                    || v.len() > 256
                    || out.properties.insert(k.into(), v.into()).is_some()
                {
                    bail!("无效或重复方块属性");
                }
            }
        }
        Ok(out)
    }
    fn from_nbt(value: &Tag) -> Result<Self> {
        let mut state = Self::parse(value.required("Name")?.string()?)?;
        if let Some(properties) = value.get("Properties") {
            for (k, v) in properties.compound()? {
                if k.len() > 128 {
                    bail!("方块属性名过长");
                }
                let v = v.string()?;
                if v.len() > 256 {
                    bail!("方块属性值过长");
                }
                state.properties.insert(k.clone(), v.into());
            }
        }
        Ok(state)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub position: [i32; 3],
    pub state: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    pub name: String,
    pub origin: [i32; 3],
    pub size: [i32; 3],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Building {
    pub format: String,
    pub data_version: Option<i64>,
    pub palette: Vec<BlockState>,
    pub blocks: Vec<Block>,
    pub regions: Vec<Region>,
    pub bounds: Option<([i32; 3], [i32; 3])>,
    pub entities: usize,
    pub block_entities: usize,
    pub overwritten_cells: usize,
    pub source_cells: usize,
    pub warnings: Vec<String>,
}
struct Builder {
    building: Building,
    lookup: BTreeMap<BlockState, usize>,
    cells: HashMap<[i32; 3], usize>,
}
impl Builder {
    fn new(format: String, root: &Tag) -> Self {
        Self {
            building: Building {
                format,
                data_version: root
                    .get("DataVersion")
                    .and_then(|v| v.int().ok())
                    .or_else(|| root.get("MinecraftDataVersion").and_then(|v| v.int().ok())),
                palette: Vec::new(),
                blocks: Vec::new(),
                regions: Vec::new(),
                bounds: None,
                entities: 0,
                block_entities: 0,
                overwritten_cells: 0,
                source_cells: 0,
                warnings: Vec::new(),
            },
            lookup: BTreeMap::new(),
            cells: HashMap::new(),
        }
    }
    fn state(&mut self, state: BlockState) -> Result<usize> {
        if let Some(id) = self.lookup.get(&state) {
            return Ok(*id);
        }
        if self.building.palette.len() >= MAX_PALETTE {
            bail!("方块色板超过65536状态");
        }
        let id = self.building.palette.len();
        self.lookup.insert(state.clone(), id);
        self.building.palette.push(state);
        Ok(id)
    }
    fn cell(&mut self, position: [i32; 3], state: usize) -> Result<()> {
        self.building.source_cells += 1;
        if self.building.source_cells > MAX_CELLS {
            bail!("建筑超过2097152个源格保护值");
        }
        if position.iter().any(|v| (*v as i64).abs() > 1_000_000) {
            bail!("建筑源坐标超过±1000000");
        }
        if self.building.palette[state].air() {
            self.cells.remove(&position);
        } else if self.cells.insert(position, state).is_some() {
            self.building.overwritten_cells += 1;
        }
        Ok(())
    }
    fn extras(&mut self, root: &Tag) {
        for key in ["Entities", "entities"] {
            self.building.entities += root
                .get(key)
                .and_then(|v| v.list().ok())
                .map_or(0, |v| v.len());
        }
        for key in ["BlockEntities", "TileEntities", "TileEntity"] {
            self.building.block_entities += root
                .get(key)
                .and_then(|v| v.list().ok())
                .map_or(0, |v| v.len());
        }
    }
    fn finish(mut self) -> Building {
        self.building.blocks = self
            .cells
            .into_iter()
            .map(|(position, state)| Block { position, state })
            .collect();
        self.building
            .blocks
            .sort_by_key(|b| [b.position[1], b.position[2], b.position[0]]);
        if !self.building.blocks.is_empty() {
            self.building.bounds = Some((
                std::array::from_fn(|a| {
                    self.building
                        .blocks
                        .iter()
                        .map(|b| b.position[a])
                        .min()
                        .unwrap()
                }),
                std::array::from_fn(|a| {
                    self.building
                        .blocks
                        .iter()
                        .map(|b| b.position[a] + 1)
                        .max()
                        .unwrap()
                }),
            ));
        }
        if self.building.entities > 0 || self.building.block_entities > 0 {
            self.building.warnings.push(
                "实体与方块实体已保留在源文件中，仅导入建筑外观，不模拟红石/容器/游戏逻辑。".into(),
            );
        }
        if self.building.overwritten_cells > 0 {
            self.building.warnings.push(format!(
                "重叠区域覆盖了{}个非空气位置，按区域名称排序，后者覆盖前者。",
                self.building.overwritten_cells
            ));
        }
        self.building
    }
}
fn volume(size: [usize; 3]) -> Result<usize> {
    if size.iter().any(|n| *n == 0 || *n > 4096) {
        bail!("建筑区域边长需要1..4096");
    }
    let n = size
        .iter()
        .try_fold(1usize, |a, b| a.checked_mul(*b))
        .context("区域体积溢出")?;
    if n > MAX_CELLS {
        bail!("单区域超过2097152格保护值");
    }
    Ok(n)
}
fn position(index: usize, size: [usize; 3], origin: [i32; 3]) -> Result<[i32; 3]> {
    let p = [
        index % size[0],
        index / (size[0] * size[2]),
        (index / size[0]) % size[2],
    ];
    let mut out = [0; 3];
    for a in 0..3 {
        out[a] = origin[a]
            .checked_add(i32::try_from(p[a])?)
            .context("建筑坐标溢出")?;
    }
    Ok(out)
}
fn palette_list(builder: &mut Builder, tag: &Tag) -> Result<Vec<usize>> {
    let list = tag.list()?;
    if list.is_empty() || list.len() > MAX_PALETTE {
        bail!("建筑方块色板无效");
    }
    list.iter()
        .map(|value| builder.state(BlockState::from_nbt(value)?))
        .collect()
}
fn litematic(root: &Tag) -> Result<Building> {
    let mut out = Builder::new("litematic".into(), root);
    let regions = root.required("Regions")?.compound()?;
    if regions.len() > 1024 {
        bail!("Litematica子区域过多");
    }
    for (name, region) in regions {
        let origin = region.required("Position")?.vector()?;
        let signed = region.required("Size")?.vector()?;
        let size = signed.map(|v| v.unsigned_abs() as usize);
        let n = volume(size)?;
        let mut minimum = origin;
        for a in 0..3 {
            if signed[a] < 0 {
                minimum[a] = origin[a]
                    .checked_add(signed[a] + 1)
                    .context("负向区域坐标溢出")?;
            }
        }
        let palette = palette_list(&mut out, region.required("BlockStatePalette")?)?;
        let bits = (usize::BITS - (palette.len() - 1).leading_zeros()).max(2) as usize;
        let states = region.required("BlockStates")?.longs()?;
        let required = (n * bits).div_ceil(64);
        if states.len() != required {
            bail!("Litematica BlockStates长度与区域/色板不符");
        }
        for i in 0..n {
            let bit = i * bits;
            let word = bit / 64;
            let shift = bit % 64;
            let mut state = (states[word] as u64) >> shift;
            if shift + bits > 64 {
                state |= (states[word + 1] as u64) << (64 - shift);
            }
            let index = (state & ((1u64 << bits) - 1)) as usize;
            let state = *palette
                .get(index)
                .context("Litematica引用不存在的方块状态")?;
            out.cell(position(i, size, minimum)?, state)?;
        }
        out.building.regions.push(Region {
            name: name.clone(),
            origin,
            size: signed,
        });
        out.extras(region);
    }
    Ok(out.finish())
}
fn unsigned_short(tag: &Tag) -> Result<usize> {
    match tag {
        Tag::Short(v) => Ok(*v as u16 as usize),
        _ => Ok(usize::try_from(tag.int()?)?),
    }
}
fn varints(data: &[u8], n: usize) -> Result<Vec<usize>> {
    let mut out = Vec::with_capacity(n);
    let mut cursor = 0;
    for _ in 0..n {
        let mut value = 0u32;
        let mut complete = false;
        for group in 0..5 {
            let byte = *data.get(cursor).context("Schematic方块VarInt截断")?;
            cursor += 1;
            if group == 4 && byte & 0xf0 != 0 {
                bail!("Schematic VarInt溢出");
            }
            value |= ((byte & 127) as u32) << (group * 7);
            if byte & 128 == 0 {
                complete = true;
                break;
            }
        }
        if !complete {
            bail!("Schematic VarInt超过5字节");
        }
        if value as usize >= MAX_PALETTE {
            bail!("Schematic色板编号越界");
        }
        out.push(value as usize);
    }
    if cursor != data.len() {
        bail!("Schematic方块数据存在多余字节");
    }
    Ok(out)
}
fn sponge(root: &Tag) -> Result<Building> {
    let version = root.required("Version")?.int()?;
    if !(1..=3).contains(&version) {
        bail!("支持Sponge schematic v1/v2/v3，文件版本为{version}");
    }
    let size = [
        unsigned_short(root.required("Width")?)?,
        unsigned_short(root.required("Height")?)?,
        unsigned_short(root.required("Length")?)?,
    ];
    let n = volume(size)?;
    let origin = root
        .get("Offset")
        .map(Tag::vector)
        .transpose()?
        .unwrap_or([0; 3]);
    let mut out = Builder::new(format!("sponge_schem_v{version}"), root);
    let blocks = if version == 3 {
        root.required("Blocks")?
    } else {
        root
    };
    let mapping = blocks.required("Palette")?.compound()?;
    if mapping.is_empty() || mapping.len() > MAX_PALETTE {
        bail!("Schematic色板无效");
    }
    let mut palette = BTreeMap::new();
    for (name, index) in mapping {
        let index = usize::try_from(index.int()?)?;
        if index >= MAX_PALETTE {
            bail!("Schematic色板编号越界");
        }
        if palette
            .insert(index, out.state(BlockState::parse(name)?)?)
            .is_some()
        {
            bail!("Schematic色板编号重复");
        }
    }
    let data = blocks
        .required(if version == 3 { "Data" } else { "BlockData" })?
        .bytes()?;
    for (i, index) in varints(data, n)?.into_iter().enumerate() {
        let state = *palette.get(&index).context("Schematic引用了缺失色板项")?;
        out.cell(position(i, size, origin)?, state)?;
    }
    out.building.regions.push(Region {
        name: "Schematic".into(),
        origin,
        size: size.map(|v| v as i32),
    });
    out.extras(root);
    if version == 3 {
        out.extras(blocks);
    }
    Ok(out.finish())
}
fn java(root: &Tag) -> Result<Building> {
    let size = root.required("size")?.vector()?;
    if size.iter().any(|n| *n <= 0 || *n > 4096) {
        bail!("Java结构尺寸无效");
    }
    let mut out = Builder::new("java_structure".into(), root);
    let palette = if let Some(palette) = root.get("palette") {
        palette
    } else {
        let list = root.required("palettes")?.list()?;
        if list.len() > 1 {
            out.building
                .warnings
                .push("结构含多套候选色板，本次使用第一套，完整源文件已保留。".into());
        }
        list.first().context("Java结构palettes为空")?
    };
    let palette = palette_list(&mut out, palette)?;
    let blocks = root.required("blocks")?.list()?;
    if blocks.len() > MAX_CELLS {
        bail!("Java结构方块数超限");
    }
    for block in blocks {
        let pos = block.required("pos")?.vector()?;
        if (0..3).any(|a| pos[a] < 0 || pos[a] >= size[a]) {
            bail!("Java结构方块超出size");
        }
        let index = usize::try_from(block.required("state")?.int()?)?;
        out.cell(pos, *palette.get(index).context("Java结构色板索引越界")?)?;
        if block.get("nbt").is_some() {
            out.building.block_entities += 1;
        }
    }
    out.building.regions.push(Region {
        name: "Structure".into(),
        origin: [0; 3],
        size,
    });
    out.extras(root);
    Ok(out.finish())
}
pub(crate) const COLORS: [&str; 16] = [
    "white",
    "orange",
    "magenta",
    "light_blue",
    "yellow",
    "lime",
    "pink",
    "gray",
    "light_gray",
    "cyan",
    "purple",
    "blue",
    "brown",
    "green",
    "red",
    "black",
];
fn legacy_state(id: u16, data: u8) -> Result<BlockState> {
    let named = match id {
        0 => "air",
        1 => match data {
            1 => "granite",
            2 => "polished_granite",
            3 => "diorite",
            4 => "polished_diorite",
            5 => "andesite",
            6 => "polished_andesite",
            _ => "stone",
        },
        2 => "grass_block",
        3 => "dirt",
        4 => "cobblestone",
        7 => "bedrock",
        12 => "sand",
        13 => "gravel",
        14 => "gold_ore",
        15 => "iron_ore",
        16 => "coal_ore",
        20 => "glass",
        21 => "lapis_ore",
        22 => "lapis_block",
        24 => "sandstone",
        30 => "cobweb",
        41 => "gold_block",
        42 => "iron_block",
        45 => "bricks",
        46 => "tnt",
        47 => "bookshelf",
        48 => "mossy_cobblestone",
        49 => "obsidian",
        50 => "torch",
        52 => "spawner",
        54 => "chest",
        56 => "diamond_ore",
        57 => "diamond_block",
        58 => "crafting_table",
        60 => "farmland",
        61 | 62 => "furnace",
        65 => "ladder",
        79 => "ice",
        80 => "snow_block",
        81 => "cactus",
        82 => "clay",
        84 => "jukebox",
        85 => "oak_fence",
        87 => "netherrack",
        88 => "soul_sand",
        89 => "glowstone",
        98 => match data {
            1 => "mossy_stone_bricks",
            2 => "cracked_stone_bricks",
            3 => "chiseled_stone_bricks",
            _ => "stone_bricks",
        },
        101 => "iron_bars",
        102 => "glass_pane",
        103 => "melon",
        110 => "mycelium",
        112 => "nether_bricks",
        113 => "nether_brick_fence",
        116 => "enchanting_table",
        121 => "end_stone",
        129 => "emerald_ore",
        133 => "emerald_block",
        152 => "redstone_block",
        155 => "quartz_block",
        165 => "slime_block",
        166 => "barrier",
        169 => "sea_lantern",
        170 => "hay_block",
        172 => "terracotta",
        173 => "coal_block",
        174 => "packed_ice",
        8 | 9 => "water",
        10 | 11 => "lava",
        _ => "",
    };
    if !named.is_empty() {
        return BlockState::parse(named);
    }
    if [35, 95, 159, 160, 171, 251, 252].contains(&id) {
        let suffix = match id {
            35 => "wool",
            95 => "stained_glass",
            159 => "terracotta",
            160 => "stained_glass_pane",
            171 => "carpet",
            251 => "concrete",
            _ => "concrete_powder",
        };
        return BlockState::parse(&format!("{}_{}", COLORS[(data & 15) as usize], suffix));
    }
    if id == 5 {
        return BlockState::parse(&format!(
            "{}_planks",
            ["oak", "spruce", "birch", "jungle", "acacia", "dark_oak"][(data as usize).min(5)]
        ));
    }
    if id == 17 || id == 162 {
        let wood = if id == 17 {
            ["oak", "spruce", "birch", "jungle"][(data & 3) as usize]
        } else {
            ["acacia", "dark_oak"][(data & 1) as usize]
        };
        let axis = match data & 12 {
            4 => "x",
            8 => "z",
            _ => "y",
        };
        return BlockState::parse(&format!("{wood}_log[axis={axis}]"));
    }
    if id == 18 || id == 161 {
        let wood = if id == 18 {
            ["oak", "spruce", "birch", "jungle"][(data & 3) as usize]
        } else {
            ["acacia", "dark_oak"][(data & 1) as usize]
        };
        return BlockState::parse(&format!("{wood}_leaves"));
    }
    let stairs = match id {
        53 => Some("oak"),
        67 => Some("cobblestone"),
        108 => Some("brick"),
        109 => Some("stone_brick"),
        114 => Some("nether_brick"),
        128 => Some("sandstone"),
        134 => Some("spruce"),
        135 => Some("birch"),
        136 => Some("jungle"),
        156 => Some("quartz"),
        163 => Some("acacia"),
        164 => Some("dark_oak"),
        180 => Some("red_sandstone"),
        _ => None,
    };
    if let Some(name) = stairs {
        return BlockState::parse(&format!(
            "{name}_stairs[facing={},half={},shape=straight]",
            ["east", "west", "south", "north"][(data & 3) as usize],
            if data & 4 != 0 { "top" } else { "bottom" }
        ));
    }
    if [43, 44, 125, 126].contains(&id) {
        let material = if id >= 125 {
            ["oak", "spruce", "birch", "jungle", "acacia", "dark_oak"][(data as usize & 7).min(5)]
        } else {
            [
                "smooth_stone",
                "sandstone",
                "petrified_oak",
                "cobblestone",
                "brick",
                "stone_brick",
                "nether_brick",
                "quartz",
            ][(data & 7) as usize]
        };
        return BlockState::parse(&format!(
            "{material}_slab[type={}]",
            if id == 43 || id == 125 {
                "double"
            } else if data & 8 != 0 {
                "top"
            } else {
                "bottom"
            }
        ));
    }
    BlockState::parse(&format!("legacy:{id}_{data}"))
}
fn mcedit(root: &Tag) -> Result<Building> {
    let size = [
        unsigned_short(root.required("Width")?)?,
        unsigned_short(root.required("Height")?)?,
        unsigned_short(root.required("Length")?)?,
    ];
    let n = volume(size)?;
    let blocks = root.required("Blocks")?.bytes()?;
    let data = root.required("Data")?.bytes()?;
    if blocks.len() != n || data.len() != n {
        bail!("旧Schematic Blocks/Data长度错误");
    }
    let extra = root.get("AddBlocks").map(Tag::bytes).transpose()?;
    if extra.is_some_and(|v| v.len() != n.div_ceil(2)) {
        bail!("AddBlocks长度错误");
    }
    let mut out = Builder::new("mcedit_schematic".into(), root);
    let origin = ["WEOffsetX", "WEOffsetY", "WEOffsetZ"].map(|k| {
        root.get(k)
            .and_then(|v| v.int().ok())
            .and_then(|n| i32::try_from(n).ok())
            .unwrap_or(0)
    });
    for i in 0..n {
        let upper = extra.map_or(0, |v| {
            if i % 2 == 0 {
                v[i / 2] & 15
            } else {
                v[i / 2] >> 4
            }
        });
        let id = blocks[i] as u16 | ((upper as u16) << 8);
        let state = out.state(legacy_state(id, data[i] & 15)?)?;
        out.cell(position(i, size, origin)?, state)?;
    }
    out.building.regions.push(Region {
        name: "MCEdit".into(),
        origin,
        size: size.map(|v| v as i32),
    });
    out.extras(root);
    Ok(out.finish())
}
pub fn parse(bytes: &[u8], name: &str) -> Result<Building> {
    if name.to_ascii_lowercase().ends_with(".mcstructure") {
        bail!("暂不支持Bedrock .mcstructure；支持.litematic/.schem/.schematic/Java结构.nbt");
    }
    let root = nbt::read(bytes)?;
    let root = root.get("Schematic").unwrap_or(&root);
    if root.get("Regions").is_some() {
        litematic(root)
    } else if root.get("Version").is_some() && root.get("Palette").is_some()
        || root.get("Version").is_some()
            && root.get("Blocks").and_then(|v| v.get("Palette")).is_some()
    {
        sponge(root)
    } else if root.get("palette").is_some() || root.get("palettes").is_some() {
        java(root)
    } else if root.get("Blocks").is_some() && root.get("Data").is_some() {
        mcedit(root)
    } else {
        bail!("无法识别建筑NBT结构；.nbt扩展名本身不能说明这是结构文件")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn c(values: Vec<(&str, Tag)>) -> Tag {
        Tag::Compound(values.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }
    fn list(values: &[i32]) -> Tag {
        Tag::List(values.iter().map(|v| Tag::Int(*v)).collect())
    }
    fn state(name: &str) -> Tag {
        c(vec![("Name", Tag::String(name.into()))])
    }
    fn kind(t: &Tag) -> u8 {
        match t {
            Tag::Byte(_) => 1,
            Tag::Short(_) => 2,
            Tag::Int(_) => 3,
            Tag::Long(_) => 4,
            Tag::Float(_) => 5,
            Tag::Double(_) => 6,
            Tag::Bytes(_) => 7,
            Tag::String(_) => 8,
            Tag::List(_) => 9,
            Tag::Compound(_) => 10,
            Tag::Ints(_) => 11,
            Tag::Longs(_) => 12,
        }
    }
    fn string(out: &mut Vec<u8>, text: &str) {
        out.extend((text.len() as u16).to_be_bytes());
        out.extend(text.as_bytes());
    }
    fn payload(out: &mut Vec<u8>, tag: &Tag) {
        match tag {
            Tag::Byte(v) => out.push(*v as u8),
            Tag::Short(v) => out.extend(v.to_be_bytes()),
            Tag::Int(v) => out.extend(v.to_be_bytes()),
            Tag::Long(v) => out.extend(v.to_be_bytes()),
            Tag::Float(v) => out.extend(v.to_be_bytes()),
            Tag::Double(v) => out.extend(v.to_be_bytes()),
            Tag::Bytes(v) => {
                out.extend((v.len() as i32).to_be_bytes());
                out.extend(v);
            }
            Tag::String(v) => string(out, v),
            Tag::List(v) => {
                out.push(v.first().map_or(0, kind));
                out.extend((v.len() as i32).to_be_bytes());
                for t in v {
                    payload(out, t);
                }
            }
            Tag::Compound(v) => {
                for (name, t) in v {
                    out.push(kind(t));
                    string(out, name);
                    payload(out, t);
                }
                out.push(0);
            }
            Tag::Ints(v) => {
                out.extend((v.len() as i32).to_be_bytes());
                for n in v {
                    out.extend(n.to_be_bytes());
                }
            }
            Tag::Longs(v) => {
                out.extend((v.len() as i32).to_be_bytes());
                for n in v {
                    out.extend(n.to_be_bytes());
                }
            }
        }
    }
    fn bytes(tag: &Tag) -> Vec<u8> {
        let mut out = vec![10, 0, 0];
        payload(&mut out, tag);
        out
    }
    #[test]
    fn litematic_cross_word_palette_and_negative_region_are_exact() {
        let mut packed = vec![0u64; 225usize.div_ceil(64)];
        for i in 0..75 {
            let value = i % 5;
            for bit in 0..3 {
                if value & (1 << bit) != 0 {
                    let index = i * 3 + bit;
                    packed[index / 64] |= 1 << (index % 64);
                }
            }
        }
        let root = c(vec![
            ("MinecraftDataVersion", Tag::Int(3955)),
            (
                "Regions",
                c(vec![(
                    "negative",
                    c(vec![
                        (
                            "Position",
                            c(vec![
                                ("x", Tag::Int(10)),
                                ("y", Tag::Int(0)),
                                ("z", Tag::Int(0)),
                            ]),
                        ),
                        (
                            "Size",
                            c(vec![
                                ("x", Tag::Int(-25)),
                                ("y", Tag::Int(3)),
                                ("z", Tag::Int(1)),
                            ]),
                        ),
                        (
                            "BlockStatePalette",
                            Tag::List(
                                ["air", "stone", "dirt", "oak_planks", "glass"]
                                    .map(state)
                                    .to_vec(),
                            ),
                        ),
                        (
                            "BlockStates",
                            Tag::Longs(packed.into_iter().map(|v| v as i64).collect()),
                        ),
                    ]),
                )]),
            ),
        ]);
        let building = parse(&bytes(&root), "negative.litematic").unwrap();
        assert_eq!(building.blocks.len(), 60);
        assert_eq!(building.data_version, Some(3955));
        for block in &building.blocks {
            let index = (block.position[0] + 14) as usize + block.position[1] as usize * 25;
            assert_eq!(
                building.palette[block.state].name,
                [
                    "minecraft:air",
                    "minecraft:stone",
                    "minecraft:dirt",
                    "minecraft:oak_planks",
                    "minecraft:glass"
                ][index % 5]
            );
        }
    }
    #[test]
    fn sponge_versions_sparse_palette_varint_and_offset_are_supported() {
        for version in 1..=3 {
            let palette = c(vec![
                ("minecraft:air", Tag::Int(0)),
                ("minecraft:oak_stairs[facing=west,half=top]", Tag::Int(129)),
            ]);
            let data = Tag::Bytes(vec![0x81, 0x01, 0]);
            let mut values = vec![
                ("Version", Tag::Int(version)),
                ("DataVersion", Tag::Int(3955)),
                ("Width", Tag::Short(2)),
                ("Height", Tag::Short(1)),
                ("Length", Tag::Short(1)),
                ("Offset", Tag::Ints(vec![-4, 2, 6])),
            ];
            if version == 3 {
                values.push(("Blocks", c(vec![("Palette", palette), ("Data", data)])));
            } else {
                values.push(("Palette", palette));
                values.push(("BlockData", data));
            }
            let mut root = c(values);
            if version == 3 {
                root = c(vec![("Schematic", root)]);
            }
            let building = parse(&bytes(&root), "build.schem").unwrap();
            assert_eq!(building.blocks.len(), 1);
            assert_eq!(building.blocks[0].position, [-4, 2, 6]);
            let state = &building.palette[building.blocks[0].state];
            assert_eq!(state.properties["facing"], "west");
            assert_eq!(state.properties["half"], "top");
        }
        assert!(varints(&[255; 5], 1).is_err());
        assert!(varints(&[128], 1).is_err());
        assert!(varints(&[0, 0], 1).is_err());
    }
    #[test]
    fn java_sparse_structure_and_legacy_metadata_keep_building_identity() {
        let root = c(vec![
            ("size", list(&[4, 4, 4])),
            ("palette", Tag::List(vec![state("air"), state("stone")])),
            (
                "blocks",
                Tag::List(vec![c(vec![
                    ("state", Tag::Int(1)),
                    ("pos", list(&[1, 2, 3])),
                    ("nbt", c(vec![("id", Tag::String("example".into()))])),
                ])]),
            ),
        ]);
        let building = parse(&bytes(&root), "structure.nbt").unwrap();
        assert_eq!(building.blocks[0].position, [1, 2, 3]);
        assert_eq!(building.block_entities, 1);
        let root = c(vec![
            ("Width", Tag::Short(3)),
            ("Height", Tag::Short(1)),
            ("Length", Tag::Short(1)),
            ("Blocks", Tag::Bytes(vec![1, 53, 35])),
            ("Data", Tag::Bytes(vec![0, 0, 14])),
        ]);
        let building = parse(&bytes(&root), "old.schematic").unwrap();
        let states: Vec<_> = building
            .blocks
            .iter()
            .map(|b| &building.palette[b.state])
            .collect();
        assert_eq!(states[0].name, "minecraft:stone");
        assert_eq!(states[1].properties["facing"], "east");
        assert_eq!(states[2].name, "minecraft:red_wool");
    }
    #[test]
    fn arbitrary_nbt_is_not_mistaken_for_a_building() {
        assert!(parse(&bytes(&c(vec![("x", Tag::Int(1))])), "anything.nbt").is_err());
        assert!(parse(&[0; 10], "bedrock.mcstructure")
            .unwrap_err()
            .to_string()
            .contains("Bedrock"));
    }
}
