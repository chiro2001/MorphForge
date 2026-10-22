//! Bounded native Avatar animation. Editor clips remain unrestricted by this adapter.
use crate::{
    creative::{self, Clip},
    text_wire::Message,
};
use anyhow::{bail, Context, Result};
use morphforge_core::Scene;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, VecDeque},
    time::Instant,
};
pub const CAPABILITY: &str = "mf6-motion4/1";
pub const DURATIONS: [f64; 10] = [0.125, 0.25, 0.5, 1., 2., 4., 8., 16., 32., 64.];
pub const SPEEDS: [f64; 4] = [0.25, 0.5, 1., 2.];
fn word(d: &mut [u8], i: usize, v: u16) {
    d[i..i + 2].copy_from_slice(&v.to_le_bytes());
}
fn fixed(x: f64, scale: f64, offset: f64) -> Result<u16> {
    let n = (x * scale + offset).round();
    if !n.is_finite() || !(0.0..=65535.).contains(&n) {
        bail!("MOTION_RANGE：动画数据超出目标量化范围")
    };
    Ok(n as u16)
}
fn signed(x: f64) -> Result<u16> {
    fixed(x, 64., 32768.)
}
fn msg(op: u8, group: u8, bank: u8) -> Message {
    let mut d = [0; 29];
    d[0] = op;
    d[1] = group;
    d[2] = bank;
    d[28] = 254;
    Message(d)
}
fn native_q(e: [f64; 3]) -> [f64; 4] {
    let q = creative::quaternion(e);
    [-q[0], -q[1], q[2], q[3]]
}
fn rotate(p: [f64; 3], q: [f64; 4]) -> [f64; 3] {
    let [x, y, z, w] = q;
    let c = [
        y * p[2] - z * p[1],
        z * p[0] - x * p[2],
        x * p[1] - y * p[0],
    ];
    let cc = [
        y * c[2] - z * c[1],
        z * c[0] - x * c[2],
        x * c[1] - y * c[0],
    ];
    std::array::from_fn(|a| p[a] + 2. * w * c[a] + 2. * cc[a])
}
#[derive(Clone)]
pub struct Definition {
    pub groups: BTreeMap<String, u8>,
    pub messages: Vec<Message>,
    pub clip: Clip,
    pub speed: f64,
    pub bank: u8,
    pub generation: u8,
}
pub fn encode(
    scene: &Scene,
    clip: &Clip,
    speed: f64,
    bank: u8,
    generation: u8,
) -> Result<Definition> {
    creative::validate_clip(clip)?;
    if bank > 1 || generation > 3 {
        bail!("MOTION_VERSION：bank/代次越界")
    }
    if clip.tracks.len() > 3 || clip.tracks.iter().any(|t| t.keys.len() > 4) {
        bail!("AVATAR_MOTION_CAPACITY：目标最多3个动画组、每组4关键帧；编辑器/GLB不受此限制")
    }
    let duration = DURATIONS
        .iter()
        .position(|x| (*x - clip.duration).abs() < 1e-6)
        .context("AVATAR_MOTION_DURATION：时长须为0.125/0.25/0.5/1/2/4/8/16/32/64秒")?;
    let speed_index = SPEEDS
        .iter()
        .position(|x| (*x - speed).abs() < 1e-6)
        .context("AVATAR_MOTION_SPEED：速度须为0.25/0.5/1/2")?;
    let mut groups = BTreeMap::new();
    let mut messages = Vec::new();
    for (i, track) in clip.tracks.iter().enumerate() {
        let group = (i + 1) as u8;
        let points = group_points(scene, &track.group);
        if points.is_empty() {
            bail!("MOTION_GROUP_MISSING：分组{}没有几何或文字", track.group)
        }
        groups.insert(track.group.clone(), group);
        for key in &track.keys {
            if key.scale.iter().any(|s| (*s - key.scale[0]).abs() > 1e-8)
                || !(0.0625..=8.).contains(&key.scale[0])
            {
                bail!("AVATAR_MOTION_SCALE：Avatar首版只支持1/16..8倍的正均匀缩放")
            }
        }
        let quantized_times = track
            .keys
            .iter()
            .map(|k| (k.time / clip.duration * 65535.).round() as u16)
            .collect::<Vec<_>>();
        if quantized_times.windows(2).any(|p| p[0] >= p[1]) {
            bail!("AVATAR_MOTION_TIME_PRECISION：相邻关键帧量化后重合，请增大时间间隔")
        }
        validate_sweep(track, &points)?;
        let mut header = msg(16, group, bank);
        header.0[3] = generation;
        header.0[4] = duration as u8;
        header.0[5] = u8::from(clip.looping);
        header.0[6] = track.keys.len() as u8;
        header.0[7] = u8::from(track.interpolation == "linear");
        for a in 0..3 {
            word(&mut header.0, 8 + a * 2, signed(track.pivot[a])?);
        }
        header.0[15] = speed_index as u8;
        messages.push(header);
        for (k, key) in track.keys.iter().enumerate() {
            let mut m = msg(17, group, bank);
            m.0[3] = k as u8;
            word(&mut m.0, 4, fixed(key.time / clip.duration, 65535., 0.)?);
            for a in 0..3 {
                word(&mut m.0, 6 + a * 2, signed(key.translation[a])?);
            }
            for (a, q) in native_q(key.rotation).iter().enumerate() {
                word(&mut m.0, 12 + a * 2, fixed(*q, 32767., 32768.)?);
            }
            word(&mut m.0, 20, fixed(key.scale[0], 4096., 0.)?);
            m.0[24] = generation;
            messages.push(m);
        }
    }
    Ok(Definition {
        groups,
        messages,
        clip: clip.clone(),
        speed,
        bank,
        generation,
    })
}
fn group_points(scene: &Scene, group: &str) -> Vec<[f64; 3]> {
    let mut out = Vec::new();
    for b in scene.boxes.iter().filter(|b| b.group == group) {
        for n in 0..8 {
            out.push(std::array::from_fn(|a| {
                b.min[a] as f64
                    + if n & (1 << a) != 0 {
                        b.size[a] as f64
                    } else {
                        0.
                    }
            }));
        }
    }
    for t in scene.triangles.iter().filter(|t| t.group == group) {
        out.extend(t.vertices.iter().map(|p| p.map(|v| v as f64)));
    }
    for p in scene.text_panels.iter().filter(|p| p.group == group) {
        out.extend(p.corners());
    }
    if out.is_empty() {
        return out;
    }
    let lo: [f64; 3] =
        std::array::from_fn(|a| out.iter().map(|p| p[a]).fold(f64::INFINITY, f64::min));
    let hi: [f64; 3] =
        std::array::from_fn(|a| out.iter().map(|p| p[a]).fold(f64::NEG_INFINITY, f64::max));
    (0..8)
        .map(|n| std::array::from_fn(|a| if n & (1 << a) != 0 { hi[a] } else { lo[a] }))
        .collect()
}
fn slerp(a: [f64; 4], mut b: [f64; 4], t: f64) -> [f64; 4] {
    let mut dot = a.iter().zip(b).map(|(a, b)| a * b).sum::<f64>();
    if dot < 0. {
        b = b.map(|v| -v);
        dot = -dot;
    }
    let q = if dot > 0.9995 {
        std::array::from_fn(|i| a[i] * (1. - t) + b[i] * t)
    } else {
        let omega = dot.clamp(0., 1.).acos();
        std::array::from_fn(|i| {
            (a[i] * ((1. - t) * omega).sin() + b[i] * (t * omega).sin()) / omega.sin()
        })
    };
    let n = q.iter().map(|x| x * x).sum::<f64>().sqrt();
    q.map(|x| x / n)
}
fn validate_sweep(track: &creative::Track, points: &[[f64; 3]]) -> Result<()> {
    let mut pairs = track
        .keys
        .windows(2)
        .map(|p| (&p[0], &p[1]))
        .collect::<Vec<_>>();
    if pairs.is_empty() {
        pairs.push((&track.keys[0], &track.keys[0]));
    }
    let radius = points
        .iter()
        .map(|p| {
            p.iter()
                .zip(track.pivot)
                .map(|(v, c)| (v - c).powi(2))
                .sum::<f64>()
                .sqrt()
        })
        .fold(0., f64::max);
    for (a, b) in pairs {
        let qa = native_q(a.rotation);
        let mut qb = native_q(b.rotation);
        if qa.iter().zip(qb).map(|(a, b)| a * b).sum::<f64>() < 0. {
            qb = qb.map(|v| -v)
        }
        // Relative quaternion gives a fixed world axis for shortest-path slerp.
        let [x, y, z, w] = qa;
        let [bx, by, bz, bw] = qb;
        let relative = [
            -bw * x + bx * w - by * z + bz * y,
            -bw * y + bx * z + by * w - bz * x,
            -bw * z - bx * y + by * x + bz * w,
            bw * w + bx * x + by * y + bz * z,
        ];
        let angle = 2. * relative[3].abs().clamp(0., 1.).acos();
        let axis_length = relative[..3].iter().map(|v| v * v).sum::<f64>().sqrt();
        let omega: [f64; 3] = if axis_length < 1e-10 {
            [0.; 3]
        } else {
            std::array::from_fn(|i| relative[i].abs() * angle / axis_length)
        };
        let max_scale = a.scale[0].max(b.scale[0]);
        let derivative: [f64; 3] = std::array::from_fn(|i| {
            (b.translation[i] - a.translation[i]).abs()
                + radius * (b.scale[0] - a.scale[0]).abs()
                + radius * max_scale * (omega[(i + 1) % 3] + omega[(i + 2) % 3])
        });
        let steps = if track.interpolation == "step" {
            1
        } else {
            128
        };
        for sample in 0..steps {
            let t = if track.interpolation == "step" {
                0.
            } else {
                (sample as f64 + 0.5) / steps as f64
            };
            let q = slerp(qa, qb, t);
            let scale = a.scale[0] * (1. - t) + b.scale[0] * t;
            let translation: [f64; 3] =
                std::array::from_fn(|i| a.translation[i] * (1. - t) + b.translation[i] * t);
            for point in points {
                let v = rotate(
                    std::array::from_fn(|i| (point[i] - track.pivot[i]) * scale),
                    q,
                );
                for i in 0..3 {
                    let p = track.pivot[i] + translation[i] + v[i];
                    let margin = if track.interpolation == "step" {
                        0.
                    } else {
                        derivative[i] / (steps as f64 * 2.)
                    };
                    let min = if i == 1 { 0. } else { -128. };
                    let max = if i == 1 { 256. } else { 128. };
                    if p - margin < min - 1e-6 || p + margin > max + 1e-6 {
                        bail!(
                            "MOTION_BOUNDS：分组{}的运动包络超出256目标空间",
                            track.group
                        )
                    }
                }
            }
        }
    }
    // Step tracks still need their final held pose checked.
    let final_key = track.keys.last().unwrap();
    let q = native_q(final_key.rotation);
    for p in points {
        let r = rotate(
            std::array::from_fn(|i| (p[i] - track.pivot[i]) * final_key.scale[0]),
            q,
        );
        for i in 0..3 {
            let value = track.pivot[i] + final_key.translation[i] + r[i];
            if value < if i == 1 { 0. } else { -128. } - 1e-6
                || value > if i == 1 { 256. } else { 128. } + 1e-6
            {
                bail!("MOTION_BOUNDS：末帧越界")
            }
        }
    }
    Ok(())
}
#[derive(Clone)]
enum Item {
    Definition(Message),
    Control(u8),
}
#[derive(Clone)]
pub struct Plan {
    pub message: Message,
    token: u64,
}
#[derive(Clone)]
pub struct Session {
    definition: Option<Definition>,
    queue: VecDeque<(u64, Item)>,
    serial: u64,
    ticket: u8,
    state: u8,
    position: f64,
    started: Instant,
    clip_id: String,
    detached: bool,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            definition: None,
            queue: VecDeque::new(),
            serial: 0,
            ticket: 0,
            state: 0,
            position: 0.,
            started: Instant::now(),
            clip_id: String::new(),
            detached: false,
        }
    }
}
impl Session {
    fn push(&mut self, item: Item) {
        self.serial += 1;
        self.queue.push_back((self.serial, item));
    }
    pub fn position(&self) -> f64 {
        let Some(d) = &self.definition else { return 0. };
        let value = self.position
            + if self.state == 1 {
                self.started.elapsed().as_secs_f64() * d.speed
            } else {
                0.
            };
        if d.clip.looping {
            value.rem_euclid(d.clip.duration)
        } else {
            value.clamp(0., d.clip.duration)
        }
    }
    pub fn publish(
        &mut self,
        scene: &Scene,
        clip: &Clip,
        id: &str,
        speed: f64,
        position: f64,
        playing: bool,
    ) -> Result<BTreeMap<String, u8>> {
        if !position.is_finite() || position < 0. || position > clip.duration {
            bail!("MOTION_POSITION：定位越界")
        };
        let bank = self.definition.as_ref().map(|d| 1 - d.bank).unwrap_or(0);
        let generation = self
            .definition
            .as_ref()
            .map(|d| (d.generation + 1) % 4)
            .unwrap_or(0);
        let definition = encode(scene, clip, speed, bank, generation)?;
        let groups = definition.groups.clone();
        self.definition = Some(definition);
        self.detached = false;
        self.clip_id = id.into();
        self.position = position;
        self.started = Instant::now();
        self.state = if playing { 1 } else { 2 };
        self.ticket = (self.ticket + 1) % 16;
        self.replay();
        Ok(groups)
    }
    pub fn validate_target(&self, scene: &Scene) -> Result<()> {
        if self.detached {
            return Ok(());
        }
        if let Some(d) = &self.definition {
            for track in &d.clip.tracks {
                let points = group_points(scene, &track.group);
                if !points.is_empty() {
                    validate_sweep(track, &points).with_context(||format!("MOTION_BOUNDS：已绑定动画分组{}的运动范围无效；请调整片段或解除Avatar动画绑定后发布静态作品",track.group))?;
                }
            }
        }
        Ok(())
    }
    pub fn detach(&mut self) {
        // Keep bank/generation/ticket history so republishing cannot be mistaken
        // for an old idempotent definition already cached by the Avatar.
        if !self.detached {
            self.ticket = (self.ticket + 1) % 16;
        }
        self.detached = true;
        self.state = 0;
        self.position = 0.;
        self.clip_id.clear();
        self.replay();
    }
    pub fn control(&mut self, action: &str, position: Option<f64>) -> Result<()> {
        if self.detached {
            bail!("尚未发布Avatar动画")
        }
        let d = self.definition.as_ref().context("尚未发布Avatar动画")?;
        let current = self.position();
        let desired = position.unwrap_or(if action == "stop" { 0. } else { current });
        if !desired.is_finite() || !(0.0..=d.clip.duration).contains(&desired) {
            bail!("MOTION_POSITION：定位越界")
        };
        let state = match action {
            "play" | "resume" => 1,
            "pause" => 2,
            "stop" => 0,
            "seek" => self.state,
            _ => bail!("动画控制须为play/resume/pause/stop/seek"),
        };
        self.position = desired;
        self.started = Instant::now();
        self.state = state;
        self.ticket = (self.ticket + 1) % 16;
        self.queue.retain(|(_, i)| matches!(i, Item::Definition(_)));
        for group in self.groups().values().copied().collect::<Vec<_>>() {
            self.push(Item::Control(group));
        }
        Ok(())
    }
    pub fn groups(&self) -> BTreeMap<String, u8> {
        if self.detached {
            return BTreeMap::new();
        }
        self.definition
            .as_ref()
            .map(|d| d.groups.clone())
            .unwrap_or_default()
    }
    pub fn replay(&mut self) {
        self.queue.clear();
        if self.detached {
            if let Some(d) = &self.definition {
                let groups = d.groups.values().copied().collect::<Vec<_>>();
                for group in groups {
                    self.push(Item::Control(group));
                }
            }
            return;
        }
        if let Some(d) = &self.definition {
            let messages = d.messages.clone();
            let groups = d.groups.values().copied().collect::<Vec<_>>();
            for m in messages {
                self.push(Item::Definition(m));
            }
            for group in groups {
                self.push(Item::Control(group));
            }
        }
    }
    pub fn next(&self) -> Option<Plan> {
        let (token, item) = self.queue.front()?;
        let message = match item {
            Item::Definition(m) => m.clone(),
            Item::Control(group) => {
                let d = self.definition.as_ref()?;
                let mut m = msg(18, *group, d.bank);
                m.0[3] = d.generation;
                m.0[4] = self.ticket;
                m.0[5] = self.state;
                m.0[6] = (self.position() / d.clip.duration * 255.)
                    .round()
                    .clamp(0., 255.) as u8;
                m
            }
        };
        Some(Plan {
            message,
            token: *token,
        })
    }
    pub fn commit(&mut self, p: &Plan) {
        if self
            .queue
            .front()
            .is_some_and(|(token, _)| *token == p.token)
        {
            self.queue.pop_front();
        }
    }
    pub fn pending(&self) -> usize {
        self.queue.len()
    }
    pub fn active(&self) -> bool {
        self.definition.is_some() && !self.detached
    }
    pub fn replay_needed(&self) -> bool {
        self.definition.is_some()
    }
    pub fn state(&self) -> Value {
        json!({"attached":self.active(),"clipId":self.clip_id,"state":self.state,"position":self.position(),"duration":self.definition.as_ref().filter(|_|!self.detached).map(|d|d.clip.duration),"speed":self.definition.as_ref().filter(|_|!self.detached).map(|d|d.speed),"groups":self.groups(),"pending":self.pending(),"capacity":{"groups":3,"keysPerGroup":4},"receiverConfirmed":false})
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use morphforge_core::Box3;
    fn fixture() -> (Scene, Clip) {
        let mut scene = Scene::default();
        let mut b = Box3::new([-4, 24, -4], [8; 3], 1);
        b.group = "rotor".into();
        scene.boxes.push(b);
        let key = |time, rotation| creative::Keyframe {
            time,
            translation: [0.; 3],
            rotation: [0., rotation, 0.],
            scale: [1.; 3],
        };
        let clip = Clip {
            name: "spin".into(),
            duration: 4.,
            looping: true,
            tracks: vec![creative::Track {
                group: "rotor".into(),
                pivot: [0., 28., 0.],
                interpolation: "linear".into(),
                keys: vec![key(0., 0.), key(2., 180.), key(4., 360.)],
            }],
        };
        (scene, clip)
    }
    #[test]
    fn definitions_are_bounded_and_replay_keeps_ticket() {
        let (s, c) = fixture();
        let mut m = Session::default();
        assert_eq!(m.publish(&s, &c, "spin", 1., 0., true).unwrap()["rotor"], 1);
        let mut packets = vec![];
        while let Some(p) = m.next() {
            packets.push(p.message.0);
            m.commit(&p)
        }
        assert_eq!(packets.len(), 5);
        let ticket = packets.last().unwrap()[4];
        m.replay();
        while let Some(p) = m.next() {
            if p.message.0[0] == 18 {
                assert_eq!(p.message.0[4], ticket);
            }
            m.commit(&p)
        }
        m.control("pause", Some(1.)).unwrap();
        let p = m.next().unwrap();
        assert_eq!(p.message.0[5], 2);
        assert_ne!(p.message.0[4], ticket);
        assert_eq!(p.message.0[6], 64);
    }
    #[test]
    fn detach_keeps_version_history_for_republish() {
        let (scene, clip) = fixture();
        let mut session = Session::default();
        session
            .publish(&scene, &clip, "first", 1., 0., false)
            .unwrap();
        session.detach();
        assert!(!session.active());
        assert!(session.groups().is_empty());
        session.replay();
        assert_eq!(session.pending(), 1);
        let stop = session.next().unwrap();
        assert_eq!(stop.message.0[0], 18);
        assert_eq!(stop.message.0[5], 0);
        assert_eq!(stop.message.0[4], 2);
        session.commit(&stop);
        session.replay();
        assert_eq!(session.next().unwrap().message.0[4], 2);
        assert!(session.control("resume", None).is_err());
        session
            .publish(&scene, &clip, "second", 1., 0., false)
            .unwrap();
        let header = session.next().unwrap();
        assert_eq!(header.message.0[2], 1);
        assert_eq!(header.message.0[3], 1);
        while let Some(p) = session.next() {
            if p.message.0[0] == 18 {
                assert_eq!(p.message.0[4], 3);
            }
            session.commit(&p);
        }
    }
    #[test]
    fn bad_native_clip_never_mutates_playback() {
        let (s, c) = fixture();
        let mut m = Session::default();
        m.publish(&s, &c, "good", 1., 0., false).unwrap();
        let old = m.state();
        let mut bad = c.clone();
        bad.tracks[0].keys[0].scale = [1., 2., 1.];
        assert!(m.publish(&s, &bad, "bad", 1., 0., true).is_err());
        assert_eq!(m.state()["clipId"], old["clipId"]);
        bad = c.clone();
        bad.tracks[0].keys[1].translation = [0., 300., 0.];
        assert!(encode(&s, &bad, 1., 0, 0).is_err());
    }
}
