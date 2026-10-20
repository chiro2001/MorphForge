//! MF6 planning is independent of UDP and rendering. Committing a frame means
//! it was written locally, never that a remote avatar acknowledged it.
use anyhow::{bail, Result};
use morphforge_core::{
    material_wire::Surface,
    mf6::{self, BoxRecord, EraseRecord, Frame, Geometry, Strip, TriangleRecord},
    protocol::{key, GeometryKey},
    Box3, Scene, Triangle3,
};
use std::collections::{BTreeMap, BTreeSet};
const POOL_CAPACITY: usize = 2048;
#[derive(Clone, Debug, PartialEq, Eq)]
struct LiveBox {
    value: Box3,
    surface: Surface,
    pool: u8,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct LiveTriangle {
    value: Triangle3,
    wire_id: u16,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Object {
    Box(GeometryKey),
    Triangle(u64),
}
#[derive(Clone, Debug)]
pub enum Action {
    BoxSet {
        value: Box3,
        surface: Surface,
        pool: u8,
    },
    BoxRemove {
        key: GeometryKey,
    },
    TriangleSet {
        value: Triangle3,
        wire_id: u16,
    },
    TriangleRemove {
        id: u64,
    },
    ReplayBoxRemove {
        pool: u8,
        key: GeometryKey,
    },
    ReplayTriangleRemove {
        wire_id: u16,
    },
}
#[derive(Clone, Debug)]
pub struct Plan {
    pub frame: Frame,
    pub actions: Vec<Action>,
}
#[derive(Clone, Debug, Default)]
pub struct Session {
    desired_boxes: BTreeMap<GeometryKey, (Box3, Surface)>,
    desired_triangles: BTreeMap<u64, Triangle3>,
    boxes: BTreeMap<GeometryKey, LiveBox>,
    triangles: BTreeMap<u64, LiveTriangle>,
    replay: Option<BTreeSet<Object>>,
    turn: u64,
    // A retired geometry can still exist in several native pools after lost
    // erases and subsequent pool reassignment. Keep every physical deletion.
    box_tombstones: BTreeMap<(u8, GeometryKey), LiveBox>,
    triangle_tombstones: BTreeSet<u16>,
    replayed_box_deletions: BTreeSet<(u8, GeometryKey)>,
    replayed_triangle_deletions: BTreeSet<u16>,
}
impl Session {
    pub fn set_target(
        &mut self,
        scene: &Scene,
        surfaces: &BTreeMap<GeometryKey, Surface>,
    ) -> Result<()> {
        if scene.boxes.len() + scene.triangles.len() > 16384 {
            bail!("MF6_CAPACITY：几何总容量为16384对象");
        }
        let mut boxes = BTreeMap::new();
        for b in &scene.boxes {
            Geometry::from_box(b)?;
            let surface = surfaces
                .get(&key(b))
                .copied()
                .unwrap_or_else(|| Surface::catalog(b.style_id, b.tint_id, b.motion_phase));
            if surface.mode > 7 {
                bail!("MF6_SURFACE：未知表面模式");
            }
            boxes.insert(key(b), (b.clone(), surface));
        }
        let mut triangles = BTreeMap::new();
        for (i, t) in scene.triangles.iter().enumerate() {
            t.wire_vertices()?;
            let mut t = t.clone();
            if t.id == 0 {
                t.id = u64::MAX - i as u64;
            }
            if triangles.insert(t.id, t).is_some() {
                bail!("MF6_ID：重复的三角面逻辑ID");
            }
        }
        if self.desired_boxes != boxes || self.desired_triangles != triangles {
            self.replay = None;
        }
        self.desired_boxes = boxes;
        self.desired_triangles = triangles;
        Ok(())
    }
    pub fn clear_sent(&mut self) {
        self.boxes.clear();
        self.triangles.clear();
        self.replay = None;
        self.box_tombstones.clear();
        self.triangle_tombstones.clear();
        self.replayed_box_deletions.clear();
        self.replayed_triangle_deletions.clear();
    }
    pub fn has_replay_work(&self) -> bool {
        !self.target_empty()
            || !self.box_tombstones.is_empty()
            || !self.triangle_tombstones.is_empty()
    }
    pub fn start_replay(&mut self) {
        self.replay = Some(BTreeSet::new());
        self.replayed_box_deletions.clear();
        self.replayed_triangle_deletions.clear();
    }
    pub fn repairing(&self) -> bool {
        self.replay.is_some()
    }
    pub fn finish_replay(&mut self) {
        self.replay = None;
    }
    pub fn target_empty(&self) -> bool {
        self.desired_boxes.is_empty() && self.desired_triangles.is_empty()
    }
    fn seen(&self, key: &Object) -> bool {
        self.replay.as_ref().is_none_or(|seen| seen.contains(key))
    }
    pub fn pending_objects(&self) -> usize {
        let deletes = if self.repairing() {
            self.box_tombstones
                .keys()
                .filter(|k| !self.replayed_box_deletions.contains(k))
                .count()
                + self
                    .triangle_tombstones
                    .iter()
                    .filter(|id| !self.replayed_triangle_deletions.contains(id))
                    .count()
        } else {
            0
        };
        deletes
            + self
                .desired_boxes
                .iter()
                .filter(|(k, (b, s))| {
                    self.boxes
                        .get(k)
                        .is_none_or(|old| old.surface != *s || old.value.shape != b.shape)
                        || !self.seen(&Object::Box(**k))
                })
                .count()
            + self
                .desired_triangles
                .iter()
                .filter(|(id, t)| {
                    self.triangles.get(id).is_none_or(|old| old.value != **t)
                        || !self.seen(&Object::Triangle(**id))
                })
                .count()
            + self
                .boxes
                .keys()
                .filter(|k| !self.desired_boxes.contains_key(k))
                .count()
            + self
                .triangles
                .keys()
                .filter(|id| !self.desired_triangles.contains_key(id))
                .count()
    }
    fn counts(&self) -> [usize; 8] {
        let mut counts = [0; 8];
        for b in self.boxes.values() {
            counts[b.pool as usize] += 1;
        }
        for t in self.triangles.values() {
            counts[t.wire_id as usize % 8] += 1;
        }
        counts
    }
    pub fn next(&self, order: &str) -> Result<Option<Plan>> {
        // Whole-target deletion barrier: retire changed geometry before births.
        let mut records = Vec::new();
        let mut actions = Vec::new();
        let mut used = BTreeSet::new();
        for (k, b) in &self.boxes {
            if !self.desired_boxes.contains_key(k) && used.insert(b.pool) {
                records.push(EraseRecord {
                    pool: b.pool,
                    geometry: Geometry::from_box(&b.value)?,
                });
                actions.push(Action::BoxRemove { key: *k });
                if records.len() == 4 {
                    break;
                }
            }
        }
        if !records.is_empty() {
            return Ok(Some(Plan {
                frame: Frame::RemoveBoxes { records },
                actions,
            }));
        }
        let mut ids = Vec::new();
        used.clear();
        for (id, t) in &self.triangles {
            if !self.desired_triangles.contains_key(id) && used.insert((t.wire_id % 8) as u8) {
                ids.push(t.wire_id);
                actions.push(Action::TriangleRemove { id: *id });
                if ids.len() == 8 {
                    break;
                }
            }
        }
        if !ids.is_empty() {
            return Ok(Some(Plan {
                frame: Frame::RemoveTriangles { ids },
                actions,
            }));
        }
        if self.repairing() {
            let mut records = Vec::new();
            let mut actions = Vec::new();
            used.clear();
            for (key, b) in &self.box_tombstones {
                if !self.replayed_box_deletions.contains(key) && used.insert(b.pool) {
                    records.push(EraseRecord {
                        pool: b.pool,
                        geometry: Geometry::from_box(&b.value)?,
                    });
                    actions.push(Action::ReplayBoxRemove {
                        pool: key.0,
                        key: key.1,
                    });
                    if records.len() == 4 {
                        break;
                    }
                }
            }
            if !records.is_empty() {
                return Ok(Some(Plan {
                    frame: Frame::RemoveBoxes { records },
                    actions,
                }));
            }
            let mut ids = Vec::new();
            let mut actions = Vec::new();
            used.clear();
            for id in &self.triangle_tombstones {
                if !self.replayed_triangle_deletions.contains(id) && used.insert((id % 8) as u8) {
                    ids.push(*id);
                    actions.push(Action::ReplayTriangleRemove { wire_id: *id });
                    if ids.len() == 8 {
                        break;
                    }
                }
            }
            if !ids.is_empty() {
                return Ok(Some(Plan {
                    frame: Frame::RemoveTriangles { ids },
                    actions,
                }));
            }
        }
        let has_boxes = self.desired_boxes.iter().any(|(k, (b, s))| {
            self.boxes
                .get(k)
                .is_none_or(|old| old.surface != *s || old.value.shape != b.shape)
                || !self.seen(&Object::Box(*k))
        });
        let has_triangles = self.desired_triangles.iter().any(|(id, t)| {
            self.triangles.get(id).is_none_or(|old| old.value != *t)
                || !self.seen(&Object::Triangle(*id))
        });
        if has_triangles && (!has_boxes || self.turn % 3 == 2) {
            return self.triangle_plan();
        }
        if has_boxes {
            return self.box_plan(order);
        }
        Ok(None)
    }
    fn box_plan(&self, order: &str) -> Result<Option<Plan>> {
        let mut pending: Vec<_> = self
            .desired_boxes
            .iter()
            .filter(|(k, (b, s))| {
                self.boxes
                    .get(k)
                    .is_none_or(|old| old.surface != *s || old.value.shape != b.shape)
                    || !self.seen(&Object::Box(**k))
            })
            .map(|(_, v)| v)
            .collect();
        pending.sort_by(|(a, sa), (b, sb)| match order {
            "bottom_up" => a.min[1]
                .cmp(&b.min[1])
                .then(a.min[2].cmp(&b.min[2]))
                .then(a.min[0].cmp(&b.min[0])),
            "material" => sa.cmp(sb).then(a.min.cmp(&b.min)),
            _ => b
                .volume()
                .cmp(&a.volume())
                .then(a.min[1].cmp(&b.min[1]))
                .then(a.min[2].cmp(&b.min[2]))
                .then(a.min[0].cmp(&b.min[0])),
        });
        let Some((first, surface)) = pending.first().copied() else {
            return Ok(None);
        };
        let mut best: Option<Plan> = None;
        for variant in 0..3 {
            if variant == 0 && (first.size != [1; 3] || first.shape != 0) {
                continue;
            }
            if variant == 1 && (surface.mode > 2 || first.shape != 0) {
                continue;
            }
            let max = if variant == 0 {
                8
            } else if variant == 1 {
                4
            } else {
                3
            };
            let mut counts = self.counts();
            let mut used = BTreeSet::new();
            let mut records = Vec::new();
            let mut actions = Vec::new();
            for (b, s) in pending.iter().copied() {
                if b.shape != first.shape
                    || (variant < 2 && *s != *surface)
                    || (variant == 0 && b.size != [1; 3])
                    || (variant == 2 && s.mode != surface.mode)
                {
                    continue;
                }
                let pool = if let Some(old) = self.boxes.get(&key(b)) {
                    if used.contains(&old.pool) {
                        continue;
                    }
                    old.pool
                } else {
                    let Some(pool) = (0..8)
                        .filter(|p| !used.contains(&(*p as u8)) && counts[*p] < POOL_CAPACITY)
                        .min_by_key(|p| (counts[*p], *p))
                    else {
                        continue;
                    };
                    counts[pool] += 1;
                    pool as u8
                };
                used.insert(pool);
                records.push(BoxRecord {
                    pool,
                    geometry: Geometry::from_box(b)?,
                    surface: *s,
                });
                actions.push(Action::BoxSet {
                    value: b.clone(),
                    surface: *s,
                    pool,
                });
                if records.len() == max {
                    break;
                }
            }
            if records.is_empty() {
                continue;
            }
            let frame = if variant == 0 {
                records.sort_by_key(|r| r.pool);
                Frame::Voxels { records }
            } else if variant == 1 {
                Frame::SharedBoxes { records }
            } else {
                Frame::Boxes {
                    shape: first.shape,
                    records,
                }
            };
            let candidate = Plan { frame, actions };
            if best
                .as_ref()
                .is_none_or(|b| candidate.actions.len() > b.actions.len())
            {
                best = Some(candidate);
            }
        }
        if best.is_none() {
            bail!("MF6_POOL_CAPACITY：目标分布超过单池2048对象，请清空并重新平衡");
        }
        Ok(best)
    }
    fn triangle_plan(&self) -> Result<Option<Plan>> {
        let pending: Vec<_> = self
            .desired_triangles
            .iter()
            .filter(|(id, t)| {
                self.triangles.get(id).is_none_or(|old| old.value != **t)
                    || !self.seen(&Object::Triangle(**id))
            })
            .map(|(_, t)| t)
            .collect();
        let Some(first) = pending.first().copied() else {
            return Ok(None);
        };
        let occupied: BTreeSet<_> = self.triangles.values().map(|t| t.wire_id).collect();
        let counts = self.counts();
        let base = if let Some(t) = self.triangles.get(&first.id) {
            t.wire_id
        } else {
            (0..mf6::ID_LIMIT)
                .find(|id| !occupied.contains(id) && counts[*id as usize % 8] < POOL_CAPACITY)
                .ok_or_else(|| anyhow::anyhow!("MF6_POOL_CAPACITY：没有空闲三角面槽位"))?
        };
        let mut consecutive = Vec::new();
        for (i, t) in pending.iter().take(4).enumerate() {
            let id = base as usize + i;
            if id >= mf6::ID_LIMIT as usize {
                break;
            }
            let id = id as u16;
            if let Some(old) = self.triangles.get(&t.id) {
                if old.wire_id != id {
                    break;
                }
            } else if occupied.contains(&id) || counts[id as usize % 8] >= POOL_CAPACITY {
                break;
            }
            if t.double_sided != first.double_sided {
                break;
            }
            consecutive.push((*t, id));
        }
        let mut selected: Option<Strip> = None;
        for rotation in 0..3 {
            let v = first.wire_vertices()?;
            let mut strip = Strip {
                first_id: base,
                vertices: (0..3).map(|i| v[(i + rotation) % 3]).collect(),
                colors: vec![first.color],
                double_sided: first.double_sided,
            };
            for (i, (t, _)) in consecutive.iter().enumerate().skip(1) {
                let n = strip.vertices.len();
                let (a, b) = if i % 2 == 0 {
                    (strip.vertices[n - 2], strip.vertices[n - 1])
                } else {
                    (strip.vertices[n - 1], strip.vertices[n - 2])
                };
                let vertices = t.wire_vertices()?;
                let next = (0..3).find_map(|j| {
                    if vertices[j] == a && vertices[(j + 1) % 3] == b {
                        Some(vertices[(j + 2) % 3])
                    } else {
                        None
                    }
                });
                if let Some(c) = next {
                    strip.vertices.push(c);
                    strip.colors.push(t.color);
                } else {
                    break;
                }
            }
            if selected
                .as_ref()
                .is_none_or(|s| strip.colors.len() > s.colors.len())
            {
                selected = Some(strip);
            }
        }
        let mut strip = selected.unwrap();
        if strip.colors.len() >= 2 {
            let min: [u8; 3] =
                std::array::from_fn(|a| strip.vertices.iter().map(|v| v[a]).min().unwrap());
            let max: [u8; 3] =
                std::array::from_fn(|a| strip.vertices.iter().map(|v| v[a]).max().unwrap());
            let local = (0..3).all(|a| max[a] - min[a] < 16);
            if !local && strip.colors.len() > 3 {
                strip.colors.truncate(3);
                strip.vertices.truncate(5);
            }
            let actions = consecutive
                .iter()
                .take(strip.colors.len())
                .map(|(t, id)| Action::TriangleSet {
                    value: (*t).clone(),
                    wire_id: *id,
                })
                .collect();
            let frame = if local {
                Frame::LocalTriangleStrip { origin: min, strip }
            } else {
                Frame::TriangleStrip { strip }
            };
            return Ok(Some(Plan { frame, actions }));
        }
        let mut used = BTreeSet::new();
        let mut reserved = occupied;
        let mut actions = Vec::new();
        let mut records = Vec::new();
        for t in pending {
            let wire_id = if let Some(old) = self.triangles.get(&t.id) {
                old.wire_id
            } else {
                let Some(id) = (0..mf6::ID_LIMIT).find(|id| {
                    !reserved.contains(id)
                        && !used.contains(&(*id % 8))
                        && counts[*id as usize % 8] < POOL_CAPACITY
                }) else {
                    continue;
                };
                id
            };
            if !used.insert(wire_id % 8) {
                continue;
            }
            reserved.insert(wire_id);
            records.push(TriangleRecord {
                id: wire_id,
                vertices: t.wire_vertices()?,
                color: t.color,
                double_sided: t.double_sided,
            });
            actions.push(Action::TriangleSet {
                value: t.clone(),
                wire_id,
            });
            if records.len() == 2 {
                break;
            }
        }
        if records.is_empty() {
            bail!("MF6_POOL_CAPACITY：三角面池分布已满");
        }
        Ok(Some(Plan {
            frame: Frame::Triangles { records },
            actions,
        }))
    }
    pub fn commit(&mut self, plan: &Plan) {
        for action in &plan.actions {
            match action {
                Action::BoxSet {
                    value,
                    surface,
                    pool,
                } => {
                    let k = key(value);
                    let center = std::array::from_fn::<_, 3, _>(|a| {
                        2 * value.min[a] as i64 + value.size[a] as i64
                    });
                    // A native birth replaces its center only within its pool.
                    self.box_tombstones.retain(|_, b| {
                        b.pool != *pool
                            || std::array::from_fn::<_, 3, _>(|a| {
                                2 * b.value.min[a] as i64 + b.value.size[a] as i64
                            }) != center
                    });
                    self.boxes.insert(
                        k,
                        LiveBox {
                            value: value.clone(),
                            surface: *surface,
                            pool: *pool,
                        },
                    );
                    if let Some(seen) = &mut self.replay {
                        seen.insert(Object::Box(k));
                    }
                }
                Action::BoxRemove { key } => {
                    if let Some(old) = self.boxes.remove(key) {
                        self.box_tombstones.insert((old.pool, *key), old);
                        if self.box_tombstones.len() > 32768 {
                            self.box_tombstones.pop_first();
                        }
                    }
                }
                Action::TriangleSet { value, wire_id } => {
                    self.triangle_tombstones.remove(wire_id);
                    self.triangles.insert(
                        value.id,
                        LiveTriangle {
                            value: value.clone(),
                            wire_id: *wire_id,
                        },
                    );
                    if let Some(seen) = &mut self.replay {
                        seen.insert(Object::Triangle(value.id));
                    }
                }
                Action::TriangleRemove { id } => {
                    if let Some(old) = self.triangles.remove(id) {
                        self.triangle_tombstones.insert(old.wire_id);
                    }
                }
                Action::ReplayBoxRemove { pool, key } => {
                    self.replayed_box_deletions.insert((*pool, *key));
                }
                Action::ReplayTriangleRemove { wire_id } => {
                    self.replayed_triangle_deletions.insert(*wire_id);
                }
            }
        }
        self.turn += 1;
    }
    pub fn repair_history_counts(&self) -> (usize, usize) {
        (self.box_tombstones.len(), self.triangle_tombstones.len())
    }
    pub fn sent_counts(&self) -> (usize, usize) {
        (self.boxes.len(), self.triangles.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use morphforge_core::{Limits, Operation};
    fn scene() -> Scene {
        let mut ops = Vec::new();
        for i in 0..80 {
            ops.push(Operation::Box {
                value: Box3::new([i % 20 * 3 - 30, i / 20 * 3, 20], [1; 3], 1),
            });
        }
        let points = [
            [0, 0, 0],
            [1, 1, 0],
            [2, 0, 0],
            [3, 1, 0],
            [4, 0, 0],
            [5, 1, 0],
        ];
        for i in 0..4 {
            let vertices = if i % 2 == 0 {
                [points[i], points[i + 1], points[i + 2]]
            } else {
                [points[i + 1], points[i], points[i + 2]]
            };
            ops.push(Operation::Triangle {
                value: Triangle3::new(vertices, [(i * 50) as u8, 140, 250]),
            });
        }
        Scene::default().apply(&ops, &Limits::default()).unwrap()
    }
    fn drain(s: &mut Session) -> Vec<Frame> {
        let mut frames = Vec::new();
        for seq in 0..1000 {
            let Some(plan) = s.next("volume").unwrap() else {
                return frames;
            };
            let bytes = plan.frame.encode((seq % 16) as u8).unwrap();
            assert_eq!(Frame::decode(&bytes).unwrap().1, plan.frame);
            frames.push(plan.frame.clone());
            s.commit(&plan);
        }
        panic!("planner did not converge")
    }

    // Mirrors native center-based replacement inside each physical pool. The
    // input is round-tripped wire frames, never the planner's Action ledger.
    #[derive(Default)]
    struct BoxReceiver(BTreeMap<(u8, [u16; 3]), (Geometry, Surface)>);
    impl BoxReceiver {
        fn apply(&mut self, frame: &Frame) {
            let center = |g: &Geometry| {
                std::array::from_fn(|a| 2 * g.min[a] as u16 + g.extent[a] as u16 + 1)
            };
            let wire = frame.encode(0).unwrap();
            match Frame::decode(&wire).unwrap().1 {
                Frame::Voxels { records }
                | Frame::SharedBoxes { records }
                | Frame::Boxes { records, .. } => {
                    for r in records {
                        self.0
                            .insert((r.pool, center(&r.geometry)), (r.geometry, r.surface));
                    }
                }
                Frame::RemoveBoxes { records } => {
                    for r in records {
                        self.0.remove(&(r.pool, center(&r.geometry)));
                    }
                }
                Frame::Clear => self.0.clear(),
                _ => panic!("Box fixture received a different primitive"),
            }
        }
        fn contents(&self) -> Vec<([u8; 3], [u8; 3], Surface)> {
            let mut values = self
                .0
                .values()
                .map(|(g, s)| (g.min, g.extent, *s))
                .collect::<Vec<_>>();
            values.sort();
            values
        }
    }

    #[test]
    fn partial_publish_uses_sent_objects_even_when_a_new_target_skips_the_draft() {
        let mut s = Session::default();
        let mut receiver = BoxReceiver::default();
        let a = Scene {
            boxes: (0..4)
                .map(|i| Box3::new([i * 3, 0, 0], [1; 3], 1))
                .collect(),
            ..Default::default()
        };
        s.set_target(&a, &BTreeMap::new()).unwrap();
        for frame in drain(&mut s) {
            receiver.apply(&frame);
        }
        let b = Scene {
            boxes: vec![
                Box3::new([0, 0, 0], [1; 3], 2),
                Box3::new([3, 0, 0], [1; 3], 2),
                a.boxes[2].clone(),
                Box3::new([12, 0, 0], [1; 3], 2),
            ],
            ..Default::default()
        };
        s.set_target(&b, &BTreeMap::new()).unwrap();
        let first = s.next("volume").unwrap().unwrap();
        assert!(matches!(first.frame, Frame::RemoveBoxes { .. }));
        receiver.apply(&first.frame);
        s.commit(&first);
        // B's new color at x=0 was desired but never emitted. C requests the
        // same color: diffing B rather than the sent ledger would miss it.
        let c = Scene {
            boxes: vec![
                b.boxes[0].clone(),
                a.boxes[2].clone(),
                Box3::new([15, 0, 0], [1; 3], 3),
            ],
            ..Default::default()
        };
        s.set_target(&c, &BTreeMap::new()).unwrap();
        for frame in drain(&mut s) {
            receiver.apply(&frame);
        }
        let mut expected = Session::default();
        let mut fresh = BoxReceiver::default();
        expected.set_target(&c, &BTreeMap::new()).unwrap();
        for frame in drain(&mut expected) {
            fresh.apply(&frame);
        }
        assert_eq!(receiver.contents(), fresh.contents());
        assert_eq!(s.pending_objects(), 0);
    }

    #[test]
    fn replay_keeps_both_erasures_when_the_same_geometry_moves_between_pools() {
        let a = Box3::new([0, 0, 0], [1; 3], 1);
        let b = Box3::new([10, 0, 0], [1; 3], 1);
        let mut s = Session::default();
        let mut receiver = BoxReceiver::default();
        for boxes in [
            vec![a.clone()],
            vec![b.clone()],
            vec![a, b.clone()],
            vec![b.clone()],
        ] {
            s.set_target(
                &Scene {
                    boxes,
                    ..Default::default()
                },
                &BTreeMap::new(),
            )
            .unwrap();
            // Lose every initial erase while still committing successful local
            // UDP writes. A is born in pool 0, then pool 1 after B takes pool 0.
            for frame in drain(&mut s) {
                if !matches!(frame, Frame::RemoveBoxes { .. }) {
                    receiver.apply(&frame);
                }
            }
        }
        assert_eq!(
            receiver.0.len(),
            3,
            "receiver retained both physical copies of A"
        );
        s.start_replay();
        for frame in drain(&mut s) {
            receiver.apply(&frame);
        }
        assert_eq!(receiver.0.len(), 1, "repair must erase A from both pools");
        assert_eq!(
            receiver.0.values().next().unwrap().0,
            Geometry::from_box(&b).unwrap()
        );
    }
    #[test]
    fn compression_replay_and_deletion_barrier_converge() {
        let source = scene();
        let mut session = Session::default();
        session.set_target(&source, &BTreeMap::new()).unwrap();
        let first = drain(&mut session);
        assert_eq!(session.sent_counts(), (source.boxes.len(), 4));
        assert!(first
            .iter()
            .any(|f| matches!(f,Frame::Voxels{records}if records.len()==8)));
        assert!(first
            .iter()
            .any(|f| matches!(f,Frame::LocalTriangleStrip{strip,..}if strip.colors.len()==4)));
        let counts = session.counts();
        assert!(counts.iter().max().unwrap() - counts.iter().min().unwrap() <= 1);
        session.start_replay();
        let replay = drain(&mut session);
        assert!(replay.iter().all(|f| !matches!(f, Frame::Clear)));
        assert_eq!(session.sent_counts(), (source.boxes.len(), 4));
        assert_eq!(session.pending_objects(), 0);
        let mut next = source.clone();
        next.boxes.remove(0);
        next.triangles.remove(0);
        next.boxes[0].style_id = 2;
        session.set_target(&next, &BTreeMap::new()).unwrap();
        let frames = drain(&mut session);
        assert!(matches!(frames[0], Frame::RemoveBoxes { .. }));
        assert!(matches!(frames[1], Frame::RemoveTriangles { .. }));
        assert_eq!(session.sent_counts(), (next.boxes.len(), 3));
    }
    #[test]
    fn whole_box_shared_surface_and_independent_colors_remain_exact() {
        let mut source = Scene::default();
        for i in 0..9 {
            source.boxes.push(Box3::new([i * 4 - 20, 0, 10], [2; 3], 1));
        }
        let mut s = Session::default();
        s.set_target(&source, &BTreeMap::new()).unwrap();
        let frames = drain(&mut s);
        assert!(matches!(&frames[0],Frame::SharedBoxes{records}if records.len()==4));
        assert_eq!(s.pending_objects(), 0);
    }
    #[test]
    fn dropped_erasures_are_replayed_even_for_an_empty_target() {
        let source = scene();
        let mut s = Session::default();
        s.set_target(&source, &BTreeMap::new()).unwrap();
        drain(&mut s);
        let original_boxes = s.boxes.len();
        let original_faces = s.triangles.len();
        s.set_target(&Scene::default(), &BTreeMap::new()).unwrap();
        // Local UDP succeeds but the receiver loses every initial erase.
        let lost = drain(&mut s);
        assert!(lost
            .iter()
            .all(|f| matches!(f, Frame::RemoveBoxes { .. } | Frame::RemoveTriangles { .. })));
        assert_eq!(s.sent_counts(), (0, 0));
        assert!(s.has_replay_work());
        for _ in 0..2 {
            s.start_replay();
            let frames = drain(&mut s);
            let boxes: usize = frames
                .iter()
                .map(|f| {
                    if let Frame::RemoveBoxes { records } = f {
                        records.len()
                    } else {
                        0
                    }
                })
                .sum();
            let faces: usize = frames
                .iter()
                .map(|f| {
                    if let Frame::RemoveTriangles { ids } = f {
                        ids.len()
                    } else {
                        0
                    }
                })
                .sum();
            assert_eq!((boxes, faces), (original_boxes, original_faces));
            assert_eq!(s.pending_objects(), 0);
            assert!(!frames.iter().any(|f| matches!(f, Frame::Clear)));
            s.finish_replay();
        }
        s.clear_sent();
        assert!(!s.has_replay_work());
    }
    #[test]
    fn retired_ids_and_centers_never_erase_their_live_replacements() {
        let mut s = Session::default();
        let original = scene();
        s.set_target(&original, &BTreeMap::new()).unwrap();
        drain(&mut s);
        s.set_target(&Scene::default(), &BTreeMap::new()).unwrap();
        drain(&mut s);
        let mut replacement = original.clone();
        for t in &mut replacement.triangles {
            t.id += 1000;
            t.color = [255, 0, 0];
        }
        s.set_target(&replacement, &BTreeMap::new()).unwrap();
        drain(&mut s);
        s.start_replay();
        let frames = drain(&mut s);
        for frame in frames {
            match frame {
                Frame::RemoveTriangles { ids } => assert!(ids
                    .iter()
                    .all(|id| !s.triangles.values().any(|t| t.wire_id == *id))),
                Frame::RemoveBoxes { records } => {
                    for r in records {
                        assert!(!s.boxes.values().any(|b| b.pool == r.pool
                            && Geometry::from_box(&b.value).unwrap() == r.geometry));
                    }
                }
                Frame::Clear => panic!("maintenance must not clear the display"),
                _ => {}
            }
        }
        assert_eq!(
            s.sent_counts(),
            (replacement.boxes.len(), replacement.triangles.len())
        );
    }
}
