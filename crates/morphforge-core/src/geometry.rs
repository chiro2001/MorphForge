use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct GeometryError(pub String);
pub type Result<T> = std::result::Result<T, GeometryError>;
fn group_default() -> String {
    "main".into()
}
fn one() -> f64 {
    0.25
}
fn extent_default() -> u32 {
    256
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Box3 {
    #[serde(default)]
    pub id: u64,
    pub min: [i32; 3],
    pub size: [u32; 3],
    #[serde(default = "style_default")]
    pub style_id: u32,
    #[serde(default)]
    pub tint_id: u8,
    #[serde(default)]
    pub motion_phase: u8,
    #[serde(default = "group_default")]
    pub group: String,
}
fn style_default() -> u32 {
    1
}
impl Box3 {
    pub fn new(min: [i32; 3], size: [u32; 3], style: u32) -> Self {
        Self {
            id: 0,
            min,
            size,
            style_id: style,
            tint_id: 0,
            motion_phase: 0,
            group: group_default(),
        }
    }
    pub fn end(&self) -> [i32; 3] {
        std::array::from_fn(|a| self.min[a] + self.size[a] as i32)
    }
    pub fn volume(&self) -> u64 {
        self.size.iter().map(|x| *x as u64).product()
    }
    pub fn intersects(&self, b: &Self) -> bool {
        let e = self.end();
        let f = b.end();
        (0..3).all(|a| self.min[a] < f[a] && b.min[a] < e[a])
    }
    pub fn contains_cell(&self, p: [i32; 3]) -> bool {
        let e = self.end();
        (0..3).all(|a| p[a] >= self.min[a] && p[a] < e[a])
    }
    pub fn appearance(&self) -> (u32, u8, u8, &str) {
        (self.style_id, self.tint_id, self.motion_phase, &self.group)
    }
    pub fn validate(&self, limits: &Limits) -> Result<()> {
        for a in 0..3 {
            let end = self.min[a] as i64 + self.size[a] as i64;
            if self.size[a] == 0
                || self.size[a] > limits.max_coord as u32 * 2
                || (self.min[a] as i64).abs() > limits.max_coord as i64
                || end.abs() > limits.max_coord as i64
            {
                return Err(GeometryError(
                    "位置或尺寸超出工程资源保护范围，尺寸须为正整数".into(),
                ));
            }
        }
        if self.tint_id > 63 || self.motion_phase > 63 || self.group.len() > 128 {
            return Err(GeometryError(
                "染色/动画预设须为0..63，分组名称不得过长".into(),
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
    pub max_boxes: usize,
    pub target_boxes: usize,
    pub max_coord: i32,
    pub max_ops: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_boxes: 262144,
            target_boxes: 16384,
            max_coord: 1_000_000,
            max_ops: 100000,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scene {
    #[serde(default = "format_default")]
    pub format: String,
    #[serde(default = "one")]
    pub cell_size: f64,
    #[serde(default = "extent_default")]
    pub size: u32,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub boxes: Vec<Box3>,
}
fn format_default() -> String {
    "MorphForgeScene/1".into()
}
impl Default for Scene {
    fn default() -> Self {
        Self {
            format: format_default(),
            cell_size: one(),
            size: 256,
            revision: 0,
            boxes: vec![],
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
    Box {
        #[serde(flatten)]
        value: Box3,
    },
    Erase {
        min: [i32; 3],
        size: [u32; 3],
    },
    Paint {
        min: [i32; 3],
        size: [u32; 3],
        #[serde(rename = "styleId")]
        style_id: u32,
        #[serde(default, rename = "tintId")]
        tint_id: u8,
    },
    Clear,
    Transform {
        ids: Vec<u64>,
        #[serde(default)]
        offset: [i32; 3],
        #[serde(default)]
        turns: i32,
        #[serde(default)]
        mirror: Option<String>,
        #[serde(default)]
        copy: bool,
    },
    Group {
        ids: Vec<u64>,
        name: String,
    },
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub box_count: usize,
    pub voxel_count: u64,
    pub bounds: Option<Bounds>,
    pub style_ids: Vec<u32>,
    pub groups: BTreeMap<String, usize>,
    pub target_boxes: usize,
    pub warnings: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bounds {
    pub min: [i32; 3],
    pub max: [i32; 3],
}

pub fn subtract(a: &Box3, b: &Box3) -> Vec<Box3> {
    if !a.intersects(b) {
        return vec![a.clone()];
    }
    let ae = a.end();
    let be = b.end();
    let lo = std::array::from_fn::<_, 3, _>(|i| a.min[i].max(b.min[i]));
    let hi = std::array::from_fn::<_, 3, _>(|i| ae[i].min(be[i]));
    let mut out = vec![];
    let mut push = |min: [i32; 3], end: [i32; 3]| {
        if (0..3).all(|i| min[i] < end[i]) {
            let mut p = a.clone();
            p.id = 0;
            p.min = min;
            p.size = std::array::from_fn(|i| (end[i] - min[i]) as u32);
            out.push(p);
        }
    };
    push(a.min, [lo[0], ae[1], ae[2]]);
    push([hi[0], a.min[1], a.min[2]], ae);
    push([lo[0], a.min[1], a.min[2]], [hi[0], lo[1], ae[2]]);
    push([lo[0], hi[1], a.min[2]], [hi[0], ae[1], ae[2]]);
    push([lo[0], lo[1], a.min[2]], [hi[0], hi[1], lo[2]]);
    push([lo[0], lo[1], hi[2]], [hi[0], hi[1], ae[2]]);
    out
}
// Sparse chunk index; huge boxes live in a side list rather than expanding millions of chunks.
#[derive(Default)]
struct Index {
    cells: HashMap<[i32; 3], HashSet<u64>>,
    large: HashSet<u64>,
}
fn chunks(b: &Box3) -> Option<Vec<[i32; 3]>> {
    let lo = b.min.map(|x| x.div_euclid(16));
    let hi = b.end().map(|x| (x - 1).div_euclid(16));
    let n = (0..3).map(|a| (hi[a] - lo[a] + 1) as u64).product::<u64>();
    if n > 4096 {
        return None;
    }
    let mut v = Vec::with_capacity(n as usize);
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                v.push([x, y, z])
            }
        }
    }
    Some(v)
}
impl Index {
    fn add(&mut self, b: &Box3) {
        if let Some(c) = chunks(b) {
            for p in c {
                self.cells.entry(p).or_default().insert(b.id);
            }
        } else {
            self.large.insert(b.id);
        }
    }
    fn remove(&mut self, b: &Box3) {
        if let Some(c) = chunks(b) {
            for p in c {
                if let Some(s) = self.cells.get_mut(&p) {
                    s.remove(&b.id);
                    if s.is_empty() {
                        self.cells.remove(&p);
                    }
                }
            }
        } else {
            self.large.remove(&b.id);
        }
    }
    fn query(&self, b: &Box3, all: &BTreeMap<u64, Box3>) -> Vec<u64> {
        let mut ids = self.large.clone();
        if let Some(c) = chunks(b) {
            for p in c {
                if let Some(v) = self.cells.get(&p) {
                    ids.extend(v);
                }
            }
        } else {
            ids.extend(all.keys());
        }
        let mut v: Vec<_> = ids
            .into_iter()
            .filter(|id| all.get(id).is_some_and(|p| p.intersects(b)))
            .collect();
        v.sort();
        v
    }
}
fn canonical_order(b: &Box3) -> ([i32; 3], [u32; 3], u32, u8, u8, &str) {
    (
        b.min,
        b.size,
        b.style_id,
        b.tint_id,
        b.motion_phase,
        &b.group,
    )
}
pub fn merge(mut boxes: Vec<Box3>) -> Vec<Box3> {
    loop {
        let mut changed = false;
        for axis in [0usize, 2, 1] {
            type Key = ([i32; 3], [u32; 3], u32, u8, u8, String);
            let mut groups: BTreeMap<Key, Vec<Box3>> = BTreeMap::new();
            for b in boxes {
                let mut p = b.min;
                let mut s = b.size;
                p[axis] = 0;
                s[axis] = 0;
                groups
                    .entry((p, s, b.style_id, b.tint_id, b.motion_phase, b.group.clone()))
                    .or_default()
                    .push(b);
            }
            boxes = vec![];
            for values in groups.values_mut() {
                values.sort_by_key(|b| b.min[axis]);
                let mut current = values[0].clone();
                for b in values.iter().skip(1) {
                    if current.end()[axis] == b.min[axis] {
                        current.size[axis] += b.size[axis];
                        current.id = current.id.min(b.id);
                        changed = true;
                    } else {
                        boxes.push(current);
                        current = b.clone();
                    }
                }
                boxes.push(current);
            }
        }
        if !changed {
            break;
        }
    }
    boxes.sort_by(|a, b| canonical_order(a).cmp(&canonical_order(b)));
    boxes
}
impl Scene {
    pub fn stats(&self, limits: &Limits) -> Stats {
        let mut groups = BTreeMap::new();
        let mut styles = BTreeSet::new();
        let mut bounds: Option<Bounds> = None;
        for b in &self.boxes {
            *groups.entry(b.group.clone()).or_insert(0) += 1;
            styles.insert(b.style_id);
            let e = b.end();
            if let Some(ref mut v) = bounds {
                for (a, end) in e.iter().enumerate() {
                    v.min[a] = v.min[a].min(b.min[a]);
                    v.max[a] = v.max[a].max(*end);
                }
            } else {
                bounds = Some(Bounds { min: b.min, max: e });
            }
        }
        let mut warnings = vec![];
        if self.boxes.len() > limits.target_boxes {
            warnings.push(format!(
                "超过{} Box设计目标，仍可编辑；请检查目标能力与成本",
                limits.target_boxes
            ));
        }
        Stats {
            box_count: self.boxes.len(),
            voxel_count: self.boxes.iter().map(Box3::volume).sum(),
            bounds,
            style_ids: styles.into_iter().collect(),
            groups,
            target_boxes: limits.target_boxes,
            warnings,
        }
    }
    pub fn validate(&self, limits: &Limits) -> Result<()> {
        if !(8..=65536).contains(&self.size) {
            return Err(GeometryError("场景网格边长须为8..65536".into()));
        }
        if self.boxes.len() > limits.max_boxes {
            return Err(GeometryError("超过配置的Box资源保护值".into()));
        }
        if !self.cell_size.is_finite() || self.cell_size <= 0.0 || self.cell_size > 1000.0 {
            return Err(GeometryError("格长必须为有限正数".into()));
        }
        let mut all = BTreeMap::new();
        let mut idx = Index::default();
        for (i, item) in self.boxes.iter().enumerate() {
            let mut b = item.clone();
            b.id = i as u64 + 1;
            b.validate(limits)?;
            if !idx.query(&b, &all).is_empty() {
                return Err(GeometryError("盒体存在重叠".into()));
            }
            idx.add(&b);
            all.insert(b.id, b);
        }
        Ok(())
    }
    pub fn apply(&self, operations: &[Operation], limits: &Limits) -> Result<Self> {
        if operations.len() > limits.max_ops {
            return Err(GeometryError("超过操作数资源保护值".into()));
        }
        let mut all = BTreeMap::new();
        let mut index = Index::default();
        let mut next_id = 1u64;
        for item in &self.boxes {
            let mut b = item.clone();
            if b.id == 0 || all.contains_key(&b.id) {
                b.id = next_id;
            }
            next_id = next_id.max(b.id + 1);
            index.add(&b);
            all.insert(b.id, b);
        }
        fn insert(b: Box3, all: &mut BTreeMap<u64, Box3>, index: &mut Index, next: &mut u64) {
            let mut b = b;
            if b.id == 0 {
                b.id = *next;
                *next += 1;
            }
            index.add(&b);
            all.insert(b.id, b);
        }
        fn erase(region: &Box3, all: &mut BTreeMap<u64, Box3>, index: &mut Index, next: &mut u64) {
            for id in index.query(region, all) {
                let old = all.remove(&id).unwrap();
                index.remove(&old);
                for p in subtract(&old, region) {
                    insert(p, all, index, next)
                }
            }
        }
        for op in operations {
            match op {
                Operation::Clear => {
                    all.clear();
                    index = Index::default();
                }
                Operation::Box { value } => {
                    value.validate(limits)?;
                    erase(value, &mut all, &mut index, &mut next_id);
                    let mut value = value.clone();
                    value.id = 0;
                    insert(value, &mut all, &mut index, &mut next_id);
                }
                Operation::Erase { min, size } => {
                    let b = Box3::new(*min, *size, 0);
                    b.validate(limits)?;
                    erase(&b, &mut all, &mut index, &mut next_id);
                }
                Operation::Paint {
                    min,
                    size,
                    style_id,
                    tint_id,
                } => {
                    let region = Box3::new(*min, *size, *style_id);
                    region.validate(limits)?;
                    if *tint_id > 63 {
                        return Err(GeometryError("染色预设越界".into()));
                    }
                    for id in index.query(&region, &all) {
                        let old = all.remove(&id).unwrap();
                        index.remove(&old);
                        for p in subtract(&old, &region) {
                            insert(p, &mut all, &mut index, &mut next_id)
                        }
                        let e = old.end();
                        let re = region.end();
                        let mut intersection = old.clone();
                        intersection.id = 0;
                        intersection.min = std::array::from_fn(|i| old.min[i].max(region.min[i]));
                        intersection.size =
                            std::array::from_fn(|i| (e[i].min(re[i]) - intersection.min[i]) as u32);
                        intersection.style_id = *style_id;
                        intersection.tint_id = *tint_id;
                        insert(intersection, &mut all, &mut index, &mut next_id);
                    }
                }
                Operation::Group { ids, name } => {
                    if name.len() > 128 {
                        return Err(GeometryError("分组名称过长".into()));
                    }
                    for id in ids {
                        if let Some(b) = all.get_mut(id) {
                            b.group = name.clone();
                        }
                    }
                }
                Operation::Transform {
                    ids,
                    offset,
                    turns,
                    mirror,
                    copy,
                } => {
                    let mut moving = vec![];
                    for id in ids {
                        if let Some(old) = all.get(id).cloned() {
                            if !copy {
                                all.remove(id);
                                index.remove(&old);
                            }
                            moving.push(old);
                        }
                    }
                    for mut b in moving {
                        b.id = 0;
                        for _ in 0..turns.rem_euclid(4) {
                            let x = b.min[0];
                            let sx = b.size[0];
                            b.min[0] = -b.min[2] - b.size[2] as i32;
                            b.min[2] = x;
                            b.size[0] = b.size[2];
                            b.size[2] = sx;
                        }
                        if let Some(axis) = mirror {
                            let a = match axis.as_str() {
                                "x" => 0,
                                "y" => 1,
                                "z" => 2,
                                _ => return Err(GeometryError("镜像轴须为x/y/z".into())),
                            };
                            b.min[a] = -b.min[a] - b.size[a] as i32;
                        }
                        for (a, delta) in offset.iter().enumerate() {
                            b.min[a] = b.min[a]
                                .checked_add(*delta)
                                .ok_or_else(|| GeometryError("变换溢出".into()))?;
                        }
                        b.validate(limits)?;
                        erase(&b, &mut all, &mut index, &mut next_id);
                        insert(b, &mut all, &mut index, &mut next_id);
                    }
                }
            }
            if all.len() > limits.max_boxes {
                return Err(GeometryError("超过配置的Box资源保护值，事务已回滚".into()));
            }
        }
        let boxes = merge(all.into_values().collect());
        if boxes.len() > limits.max_boxes {
            return Err(GeometryError("超过Box资源保护值".into()));
        }
        Ok(Self {
            revision: self.revision + 1,
            boxes,
            ..self.clone()
        })
    }
    pub fn import_json(value: &serde_json::Value, limits: &Limits) -> Result<Self> {
        let value = value
            .get("project")
            .and_then(|p| p.get("scene"))
            .or_else(|| value.get("scene"))
            .or_else(|| value.get("document"))
            .unwrap_or(value);
        if !value.get("boxes").is_some_and(|b| b.is_array()) {
            return Err(GeometryError(
                "导入场景必须包含boxes数组；清空请显式导入空数组".into(),
            ));
        }
        if let Some(format) = value.get("format").and_then(|f| f.as_str()) {
            if !["MorphForgeScene/1", "MorphForgeEditor/1"].contains(&format) {
                return Err(GeometryError("不支持的场景格式或版本".into()));
            }
        }
        let mut value = value.clone();
        if value.get("format").and_then(|v| v.as_str()) == Some("MorphForgeEditor/1") {
            if let Some(boxes) = value.get_mut("boxes").and_then(|v| v.as_array_mut()) {
                for b in boxes {
                    if let Some(min) = b.get_mut("min").and_then(|v| v.as_array_mut()) {
                        for i in [0, 2] {
                            let x = min
                                .get(i)
                                .and_then(|v| v.as_i64())
                                .ok_or_else(|| GeometryError("导入坐标无效".into()))?;
                            min[i] = serde_json::json!(x - 128);
                        }
                    }
                }
            }
            value["format"] = serde_json::json!(format_default());
        }
        let mut scene: Self =
            serde_json::from_value(value).map_err(|e| GeometryError(e.to_string()))?;
        scene.validate(limits)?;
        for (i, b) in scene.boxes.iter_mut().enumerate() {
            b.id = i as u64 + 1;
        }
        scene.boxes = merge(scene.boxes);
        Ok(scene)
    }
}
