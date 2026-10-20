use crate::store::Workspace;
pub mod unified;
use anyhow::{bail, Context, Result};
use fs2::FileExt;
use morphforge_core::protocol::{self, appearance, key, Command, OscValue};
use morphforge_core::{Box3, Scene};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    net::UdpSocket,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub enabled: bool,
    pub port: u16,
    pub interval: f64,
    pub profile: String,
    pub auto: bool,
    pub capacity: usize,
    pub refresh_seconds: u64,
    #[serde(default)]
    pub loop_replay: bool,
    #[serde(default = "default_send_order")]
    pub send_order: String,
    pub world: bool,
    pub visible: bool,
    pub scale: f32,
    pub level: bool,
}
fn default_send_order() -> String {
    "volume".into()
}
impl Default for Connection {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 9012,
            interval: 0.25,
            profile: "mf3".into(),
            auto: false,
            capacity: 16384,
            refresh_seconds: 0,
            loop_replay: false,
            send_order: default_send_order(),
            world: true,
            visible: true,
            scale: morphforge_core::material_wire::SCALE_REFERENCE,
            level: true,
        }
    }
}
pub struct State {
    pub unified: unified::Session,
    pub config: Connection,
    pub avatar_catalog: Option<crate::target::AvatarCatalog>,
    pub avatar_catalog_error: String,
    pub sent: Vec<Box3>,
    pub desired: Vec<Box3>,
    pub desired_revision: u64,
    pub sent_revision: Option<u64>,
    pub seq: u8,
    pub wide_seqs: [u8; 3],
    pub lanes: BTreeMap<protocol::GeometryKey, usize>,
    pub wire_sent: BTreeMap<protocol::GeometryKey, morphforge_core::material_wire::Surface>,
    pub wire_desired: BTreeMap<protocol::GeometryKey, morphforge_core::material_wire::Surface>,
    pub repair_seen: Option<BTreeSet<protocol::GeometryKey>>,
    pub brush: Option<(u32, u8, u8)>,
    pub resets: u8,
    pub connected: bool,
    pub last_error: String,
    pub sent_count: u64,
    pub replay_cycles: u64,
    pub generation: u64,
    pub controls: BTreeMap<String, Control>,
    pub reset_pulse: u8,
    pub last_refresh: Instant,
    pub stop: bool,
}
#[derive(Clone, PartialEq)]
pub enum Control {
    Bool(bool),
    Float(f32),
}
#[derive(Clone)]
pub struct Transport {
    pub inner: Arc<Mutex<State>>,
}
impl Transport {
    pub fn new(workspace: Arc<Mutex<Workspace>>) -> Self {
        let root = workspace.lock().unwrap().root.clone();
        let pending = root.join("pending-avatar-catalog.json");
        if let Ok(bytes) = fs::read(&pending) {
            if let Ok(catalog) = crate::target::AvatarCatalog::from_bytes(&bytes) {
                if catalog.runtime_materials() {
                    if root.join("avatar-catalog.json").exists() {
                        let _ = fs::copy(
                            root.join("avatar-catalog.json"),
                            root.join("avatar-catalog.previous.json"),
                        );
                    }
                    if crate::store::write_json(&root.join("avatar-catalog.json"), &catalog).is_ok()
                    {
                        let _ = fs::remove_file(&pending);
                    }
                }
            }
        }
        let (avatar_catalog, avatar_catalog_error) = if root.join("avatar-catalog.json").exists() {
            match std::fs::read(root.join("avatar-catalog.json"))
                .map_err(anyhow::Error::from)
                .and_then(|b| crate::target::AvatarCatalog::from_bytes(&b))
            {
                Ok(c) => (Some(c), String::new()),
                Err(e) => (None, e.to_string()),
            }
        } else {
            (None, String::new())
        };
        let config_path = root.join("connection.json");
        let mut config = fs::read(&config_path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Connection>(&b).ok())
            .unwrap_or_default();
        config.enabled = false;
        if let Some(protocol) = avatar_catalog
            .as_ref()
            .and_then(crate::target::AvatarCatalog::wire_protocol)
        {
            config.profile = protocol.into();
            if ["mf5", "mf6"].contains(&protocol) {
                config.scale = config.scale.clamp(
                    morphforge_core::material_wire::SCALE_MIN,
                    morphforge_core::material_wire::SCALE_MAX,
                );
            }
        }
        let t = Self {
            inner: Arc::new(Mutex::new(State {
                unified: unified::Session::default(),
                config,
                avatar_catalog,
                avatar_catalog_error,
                sent: vec![],
                desired: vec![],
                desired_revision: 0,
                sent_revision: None,
                seq: 1,
                wide_seqs: [1; 3],
                lanes: BTreeMap::new(),
                wire_sent: BTreeMap::new(),
                wire_desired: BTreeMap::new(),
                repair_seen: None,
                brush: None,
                resets: 0,
                connected: false,
                last_error: String::new(),
                sent_count: 0,
                replay_cycles: 0,
                generation: 0,
                controls: BTreeMap::new(),
                reset_pulse: 0,
                last_refresh: Instant::now(),
                stop: false,
            })),
        };
        let c = t.clone();
        std::thread::spawn(move || c.worker(workspace));
        t
    }
    pub fn config(&self) -> Connection {
        self.inner.lock().unwrap().config.clone()
    }
    pub fn state(&self) -> Value {
        let s = self.inner.lock().unwrap();
        let planning_sent = if s.resets > 0 { &[][..] } else { &s.sent[..] };
        let commands = protocol::plan(
            planning_sent,
            &s.desired,
            s.brush,
            s.config.profile == "mf2",
        );
        let command_batches = if s.config.profile == "mf6" {
            s.unified.pending_objects().div_ceil(3)
        } else if ["mf4", "mf5"].contains(&s.config.profile.as_str()) {
            wide_queues(&s, planning_sent)
                .iter()
                .map(Vec::len)
                .max()
                .unwrap_or(0)
        } else {
            commands.len()
        };
        let pending = command_batches
            + s.resets as usize
            + usize::from(!s.controls.is_empty())
            + s.reset_pulse as usize;
        json!({"enabled":s.config.enabled,"sending":s.connected,"port":s.config.port,"interval":s.config.interval,"profile":s.config.profile,"auto":s.config.auto,"capacity":s.config.capacity,"refreshSeconds":s.config.refresh_seconds,"loopReplay":s.config.loop_replay,"sendOrder":s.config.send_order,"replayCycles":s.replay_cycles,"pending":pending,"etaSeconds":pending as f64*s.config.interval,"desiredRevision":s.desired_revision,"sentRevision":s.sent_revision,"sentCommands":s.sent_count,"error":s.last_error,"ack":false,"repairing":s.repair_seen.is_some()||s.unified.repairing(),"syncedBits":if ["mf4","mf5","mf6"].contains(&s.config.profile.as_str()){240}else if s.config.profile=="mf3"{56}else{88},"repairDeletions":if s.config.profile=="mf6"{Some(s.unified.repair_history_counts())}else{None},"pendingObjects":if s.config.profile=="mf6"{Some(s.unified.pending_objects())}else{None},"estimateKind":if s.config.profile=="mf6"{"adaptive-layout estimate; no receiver acknowledgement"}else{"legacy queue estimate"},"placement":{"world":s.config.world,"visible":s.config.visible,"scale":s.config.scale,"level":s.config.level}})
    }
    pub fn configure(&self, v: &Value, workspace: &mut Workspace) -> Result<Value> {
        let mut s = self.inner.lock().unwrap();
        let old = s.config.clone();
        let mut next = serde_json::to_value(&old)?;
        for (k, value) in v.as_object().context("设置须为对象")? {
            if next.get(k).is_none() {
                bail!("未知连接设置 {k}")
            }
            next[k] = value.clone();
        }
        let mut config: Connection = serde_json::from_value(next)?;
        if !["mf5", "mf6"].contains(&config.profile.as_str())
            && !v.get("loopReplay").is_some_and(|x| x == true)
        {
            config.loop_replay = false;
        }
        if !["volume", "bottom_up", "material"].contains(&config.send_order.as_str())
            || (config.loop_replay && !["mf5", "mf6"].contains(&config.profile.as_str()))
        {
            bail!("发送顺序需要volume/bottom_up/material；连续补播仅限MF5");
        }
        if config.port == 0
            || !config.interval.is_finite()
            || !(0.05..=5.).contains(&config.interval)
            || !config.scale.is_finite()
            || !scale_valid(config.scale, &config.profile)
            || !["mf6", "mf5", "mf4", "mf3", "mf2"].contains(&config.profile.as_str())
            || config.capacity == 0
            || config.capacity > 50000
        {
            bail!("连接配置越界")
        }
        if ["mf4", "mf5", "mf6"].contains(&config.profile.as_str()) && config.capacity > 16384 {
            bail!("MF4_CAPACITY：三通道头像总容量为16384 Box");
        }
        if config.enabled {
            if config.profile == "mf6"
                && !s
                    .avatar_catalog
                    .as_ref()
                    .is_some_and(crate::target::AvatarCatalog::unified_protocol)
            {
                bail!("MF6_CATALOG_REQUIRED：请载入已验证的MF6配套目标目录");
            }
            if let Some(target) = s
                .avatar_catalog
                .as_ref()
                .and_then(crate::target::AvatarCatalog::wire_protocol)
            {
                if target != config.profile {
                    bail!("PROTOCOL_MISMATCH：目标头像使用 {target}，请切换对应发送协议");
                }
            }
            if config.profile == "mf5"
                && !s
                    .avatar_catalog
                    .as_ref()
                    .is_some_and(crate::target::AvatarCatalog::runtime_materials)
            {
                bail!("MF5_CATALOG_REQUIRED：请载入MF5配套目标目录");
            }
            if config.profile == "mf4"
                && !s
                    .avatar_catalog
                    .as_ref()
                    .is_some_and(crate::target::AvatarCatalog::wide_protocol)
            {
                bail!("MF4_CATALOG_REQUIRED：请载入MF4配套目标目录");
            }
        }
        if config.enabled && !s.avatar_catalog_error.is_empty() {
            bail!("MATERIAL_CATALOG_INVALID：{}", s.avatar_catalog_error);
        }
        let restart = config.enabled
            && (!old.enabled || old.port != config.port || old.profile != config.profile);
        if restart || (config.enabled && config.auto && !old.auto) {
            let projected = crate::target::project(
                &workspace.project.scene,
                &workspace.project.materials,
                s.avatar_catalog.as_ref(),
            )?;
            validate_scene(&projected, config.capacity, &config.profile)?;
        }
        s.config = config;
        if restart {
            s.generation += 1;
            s.repair_seen = None;
            s.sent.clear();
            s.lanes.clear();
            s.wide_seqs = [1; 3];
            s.seq = 1;
            s.unified.clear_sent();
            s.wire_sent.clear();
            s.sent_revision = None;
            s.brush = None;
            s.resets = 2;
            s.last_error.clear();
            s.controls.clear();
            let c = s.config.clone();
            s.controls.insert("MF_World".into(), Control::Bool(c.world));
            s.controls
                .insert("MF_Handle".into(), Control::Bool(c.visible));
            s.controls
                .insert("MF_Scale".into(), Control::Float(c.scale));
            if c.profile != "mf2" {
                s.controls.insert("MF_Level".into(), Control::Bool(c.level));
            }
            validate_scene(
                &workspace.project.scene,
                s.config.capacity,
                &s.config.profile,
            )?;
            let projection = crate::target::project_wire(
                &workspace.project.scene,
                &workspace.project.materials,
                s.avatar_catalog.as_ref(),
            )?;
            if s.config.profile == "mf6" {
                s.unified
                    .set_target(&projection.scene, &projection.surfaces)?;
            }
            s.desired = projection.scene.boxes;
            s.wire_desired = projection.surfaces;
            s.desired_revision = workspace.project.scene.revision;
        }
        if !s.config.enabled {
            s.generation += 1;
            s.connected = false;
        }
        if s.config.enabled && s.config.auto && !old.auto {
            validate_scene(
                &workspace.project.scene,
                s.config.capacity,
                &s.config.profile,
            )?;
            let projection = crate::target::project_wire(
                &workspace.project.scene,
                &workspace.project.materials,
                s.avatar_catalog.as_ref(),
            )?;
            if s.config.profile == "mf6" {
                s.unified
                    .set_target(&projection.scene, &projection.surfaces)?;
            }
            s.desired = projection.scene.boxes;
            s.wire_desired = projection.surfaces;
            s.desired_revision = workspace.project.scene.revision;
        }
        let mut saved = s.config.clone();
        saved.enabled = false;
        crate::store::write_json(&workspace.root.join("connection.json"), &saved)?;
        let event = json!(&s.config);
        drop(s);
        workspace.event("transport.configured", None, event)?;
        Ok(self.state())
    }
    pub fn material_report(
        &self,
        scene: &Scene,
        materials: &[morphforge_core::materials::Material],
    ) -> Value {
        let state = self.inner.lock().unwrap();
        if !state.avatar_catalog_error.is_empty() {
            return json!({"status":"invalid","matched":false,"missingIds":[],"differentIds":[],"note":state.avatar_catalog_error,"remoteVerified":false});
        }
        crate::target::compatibility(scene, materials, state.avatar_catalog.as_ref())
    }
    pub fn set_catalog(
        &self,
        catalog: Option<crate::target::AvatarCatalog>,
        root: &std::path::Path,
    ) -> Result<()> {
        let mut s = self.inner.lock().unwrap();
        if s.config.enabled {
            bail!("请先停止OSC发送，再更换Avatar目录");
        }
        if let Some(ref c) = catalog {
            crate::store::write_json(&root.join("avatar-catalog.json"), c)?;
        } else {
            let p = root.join("avatar-catalog.json");
            if p.exists() {
                std::fs::remove_file(p)?;
            }
        }
        s.avatar_catalog = catalog;
        s.avatar_catalog_error.clear();
        Ok(())
    }
    pub fn publish_with_materials(
        &self,
        scene: &Scene,
        materials: &[morphforge_core::materials::Material],
        resync: bool,
    ) -> Result<Value> {
        let profile = {
            let state = self.inner.lock().unwrap();
            if !state.avatar_catalog_error.is_empty() {
                bail!("MATERIAL_CATALOG_INVALID：{}", state.avatar_catalog_error);
            }
            state.avatar_catalog.clone()
        };
        let projected = crate::target::project_wire(scene, materials, profile.as_ref())?;
        self.publish_projection(&projected.scene, projected.surfaces, resync)
    }
    pub fn publish(&self, scene: &Scene, resync: bool) -> Result<Value> {
        self.publish_projection(scene, BTreeMap::new(), resync)
    }
    fn publish_projection(
        &self,
        scene: &Scene,
        surfaces: BTreeMap<protocol::GeometryKey, morphforge_core::material_wire::Surface>,
        resync: bool,
    ) -> Result<Value> {
        let mut s = self.inner.lock().unwrap();
        validate_scene(scene, s.config.capacity, &s.config.profile)?;
        if s.config.profile == "mf6" {
            s.unified.set_target(scene, &surfaces)?;
        }
        if s.desired_revision != scene.revision || s.wire_desired != surfaces {
            // New edits take priority over an in-progress maintenance replay.
            s.repair_seen = None;
        }
        s.wire_desired = surfaces;
        s.desired = scene.boxes.clone();
        s.desired_revision = scene.revision;
        if resync {
            s.repair_seen = None;
            s.resets = s.resets.max(2);
            s.brush = None;
            s.sent_revision = None;
        }
        drop(s);
        Ok(self.state())
    }
    /// Idempotent replay preserves native pool assignments and existing display.
    /// This is blind repair, never a receiver acknowledgement.
    pub fn repair(&self) -> Result<Value> {
        let mut s = self.inner.lock().unwrap();
        if !["mf5", "mf6"].contains(&s.config.profile.as_str()) {
            bail!("补播需要MF5头像；旧协议请使用清空并重建");
        }
        if s.config.profile == "mf6" {
            s.unified.start_replay();
        } else {
            s.repair_seen = Some(BTreeSet::new());
        }
        s.sent_revision = None;
        s.last_refresh = Instant::now();
        drop(s);
        Ok(self.state())
    }
    pub fn placement(&self, v: &Value) -> Result<Value> {
        let mut updates = vec![];
        for key in ["world", "visible", "level"] {
            if let Some(value) = v.get(key) {
                updates.push((key, value.as_bool().context("开关须为布尔值")?));
            }
        }
        let scale = v
            .get("scale")
            .map(|v| v.as_f64().context("缩放须为数字"))
            .transpose()?;
        if scale.is_some_and(|f| !f.is_finite() || !scale_valid(f as f32, &self.config().profile)) {
            bail!("缩放超出当前头像范围：MF5为.0005.. .5（新1%..1000%），旧协议为.05..1")
        }
        let mut s = self.inner.lock().unwrap();
        for (key, b) in updates {
            match key {
                "world" => s.config.world = b,
                "visible" => s.config.visible = b,
                _ => s.config.level = b,
            };
            s.controls.insert(
                match key {
                    "world" => "MF_World",
                    "visible" => "MF_Handle",
                    _ => "MF_Level",
                }
                .into(),
                Control::Bool(b),
            );
        }
        if let Some(f) = scale {
            s.config.scale = f as f32;
            s.controls
                .insert("MF_Scale".into(), Control::Float(f as f32));
        }
        if v.get("reset").and_then(Value::as_bool) == Some(true) {
            s.reset_pulse = 2;
        }
        drop(s);
        Ok(self.state())
    }
    fn worker(&self, workspace: Arc<Mutex<Workspace>>) {
        let mut channel: Option<(fs::File, UdpSocket, u64)> = None;
        let mut next = Instant::now();
        loop {
            let (enabled, generation, stop) = {
                let s = self.inner.lock().unwrap();
                (s.config.enabled, s.generation, s.stop)
            };
            if stop {
                break;
            }
            if channel
                .as_ref()
                .is_some_and(|(_, _, g)| !enabled || *g != generation)
            {
                channel = None;
                self.inner.lock().unwrap().connected = false;
            }
            if !enabled {
                std::thread::sleep(Duration::from_millis(30));
                continue;
            }
            if channel.is_none() {
                let result = (|| -> Result<_> {
                    let directory = std::env::var_os("USERPROFILE")
                        .or_else(|| std::env::var_os("HOME"))
                        .map(PathBuf::from)
                        .context("没有用户目录")?
                        .join("tmp/morphforge");
                    fs::create_dir_all(&directory)?;
                    let file = fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create(true)
                        .truncate(false)
                        .open(directory.join("stage1-osc.lock"))?;
                    file.try_lock_exclusive()
                        .context("另一进程正在占用OSC通道")?;
                    let socket = UdpSocket::bind("127.0.0.1:0")?;
                    Ok((file, socket, generation))
                })();
                match result {
                    Ok(c) => {
                        channel = Some(c);
                        let mut s = self.inner.lock().unwrap();
                        s.connected = true;
                        s.last_error.clear();
                    }
                    Err(e) => {
                        let mut s = self.inner.lock().unwrap();
                        s.config.enabled = false;
                        s.last_error = e.to_string();
                        continue;
                    }
                }
            }
            if Instant::now() < next {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            enum Job {
                Unified(unified::Plan),
                Batch(Vec<(usize, Command)>),
                Command(Command),
                Controls(Vec<(String, Control)>),
                Pulse(u8),
            }
            let mut s = self.inner.lock().unwrap();
            if !s.config.enabled || channel.as_ref().is_none_or(|(_, _, g)| *g != s.generation) {
                continue;
            }
            let job = {
                if (!s.desired.is_empty()
                    || (s.config.profile == "mf6" && s.unified.has_replay_work()))
                    && (s.config.loop_replay
                        || (s.config.refresh_seconds > 0
                            && s.last_refresh.elapsed().as_secs() >= s.config.refresh_seconds))
                    && s.sent_revision == Some(s.desired_revision)
                {
                    if s.config.profile == "mf6" {
                        s.unified.start_replay();
                    } else if s.config.profile == "mf5" {
                        s.repair_seen = Some(BTreeSet::new());
                    } else {
                        s.resets = 2;
                    }
                    s.sent_revision = None;
                    s.last_refresh = Instant::now();
                }
                if s.resets > 0 {
                    Some(if s.config.profile == "mf6" {
                        Job::Unified(unified::Plan {
                            frame: morphforge_core::mf6::Frame::Clear,
                            actions: vec![],
                        })
                    } else if ["mf4", "mf5"].contains(&s.config.profile.as_str()) {
                        Job::Batch(
                            (0..protocol::WIDE_LANES)
                                .map(|i| {
                                    (
                                        if s.config.profile == "mf5" { i * 2 } else { i },
                                        Command::Clear,
                                    )
                                })
                                .collect(),
                        )
                    } else {
                        Job::Command(Command::Clear)
                    })
                } else if s.reset_pulse > 0 {
                    Some(Job::Pulse(s.reset_pulse))
                } else if !s.controls.is_empty() {
                    Some(Job::Controls(
                        s.controls
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect(),
                    ))
                } else {
                    let command = if s.config.profile == "mf6" {
                        match s.unified.next(&s.config.send_order) {
                            Ok(plan) => plan.map(Job::Unified),
                            Err(error) => {
                                s.config.enabled = false;
                                s.last_error = error.to_string();
                                continue;
                            }
                        }
                    } else if ["mf4", "mf5"].contains(&s.config.profile.as_str()) {
                        let batch: Vec<_> = wide_queues(&s, &s.sent)
                            .into_iter()
                            .enumerate()
                            .filter_map(|(i, q)| {
                                q.into_iter().next().map(|(secondary, c)| {
                                    (
                                        if s.config.profile == "mf5" {
                                            i * 2 + secondary
                                        } else {
                                            i
                                        },
                                        c,
                                    )
                                })
                            })
                            .collect();
                        if batch.is_empty() {
                            None
                        } else {
                            Some(Job::Batch(batch))
                        }
                    } else {
                        protocol::plan(&s.sent, &s.desired, s.brush, s.config.profile == "mf2")
                            .into_iter()
                            .next()
                            .map(Job::Command)
                    };
                    if command.is_none() {
                        if s.repair_seen.is_some() || s.unified.repairing() {
                            s.replay_cycles += 1;
                        }
                        s.repair_seen = None;
                        s.unified.finish_replay();
                        if s.sent_revision != Some(s.desired_revision) {
                            s.last_refresh = Instant::now();
                        }
                        s.sent_revision = Some(s.desired_revision);
                    }
                    command
                }
            };
            let Some(job) = job else {
                drop(s);
                std::thread::sleep(Duration::from_millis(20));
                continue;
            };
            let (seq, port, interval, legacy) = (
                s.seq,
                s.config.port,
                s.config.interval,
                s.config.profile == "mf2",
            );
            let encoded = match &job {
                Job::Unified(plan) => plan.frame.packet(seq),
                Job::Command(c) => protocol::packet(c, seq, legacy),
                Job::Batch(batch) => batch
                    .iter()
                    .try_fold(Vec::new(), |mut values, (lane, c)| {
                        values.extend(if s.config.profile == "mf5" {
                            let surface = command_box(c)
                                .map(|b| surface_for(&s, b))
                                .unwrap_or_else(|| {
                                    morphforge_core::material_wire::Surface::catalog(0, 0, 0)
                                });
                            protocol::mf5_pool_values(
                                c,
                                s.wide_seqs[*lane / 2],
                                *lane / 2,
                                surface,
                                *lane % 2 == 1,
                            )?
                        } else {
                            protocol::wide_values(c, s.wide_seqs[*lane], *lane)?
                        });
                        Ok(values)
                    })
                    .map(|values| protocol::bundle(&values)),
                Job::Controls(v) => Ok(protocol::bundle(
                    &v.iter()
                        .map(|(k, v)| {
                            (
                                k.clone(),
                                match v {
                                    Control::Bool(b) => OscValue::Bool(*b),
                                    Control::Float(f) => OscValue::Float(
                                        if k == "MF_Scale"
                                            && ["mf5", "mf6"].contains(&s.config.profile.as_str())
                                        {
                                            morphforge_core::material_wire::scale_wire(*f)
                                        } else {
                                            *f
                                        },
                                    ),
                                },
                            )
                        })
                        .collect::<Vec<_>>(),
                )),
                Job::Pulse(p) => {
                    let mut v = vec![("MF_Reset".into(), OscValue::Bool(*p == 2))];
                    if *p == 2 {
                        if legacy {
                            v.push(("MF_Grab".into(), OscValue::Int(0)))
                        } else {
                            v.push(("MF_GrabL".into(), OscValue::Bool(false)));
                            v.push(("MF_GrabR".into(), OscValue::Bool(false)));
                        }
                    }
                    Ok(protocol::bundle(&v))
                }
            };
            let result = encoded.map_err(anyhow::Error::from).and_then(|bytes| {
                let n = channel
                    .as_ref()
                    .unwrap()
                    .1
                    .send_to(&bytes, ("127.0.0.1", port))?;
                Ok(n)
            });
            match result {
                Err(e) => {
                    s.config.enabled = false;
                    s.last_error = e.to_string();
                    continue;
                }
                Ok(_) => {
                    s.sent_count += 1;
                }
            }
            match &job {
                Job::Unified(plan) => {
                    s.seq = (seq + 1) % 16;
                    if matches!(plan.frame, morphforge_core::mf6::Frame::Clear) {
                        s.unified.clear_sent();
                        s.sent.clear();
                        s.lanes.clear();
                        s.wire_sent.clear();
                        s.sent_revision = None;
                        s.resets = s.resets.saturating_sub(1);
                    } else {
                        s.unified.commit(plan);
                    }
                }
                Job::Batch(batch) => {
                    for (lane, _) in batch {
                        let index = if s.config.profile == "mf5" {
                            *lane / 2
                        } else {
                            *lane
                        };
                        s.wide_seqs[index] = (s.wide_seqs[index] + 1)
                            % if s.config.profile == "mf5" { 8 } else { 16 };
                    }
                    let clear = batch.iter().any(|(_, c)| matches!(c, Command::Clear));
                    if clear {
                        s.sent.clear();
                        s.lanes.clear();
                        s.wire_sent.clear();
                        s.sent_revision = None;
                        s.brush = None;
                        s.resets = s.resets.saturating_sub(1);
                    } else {
                        for (lane, c) in batch {
                            match c {
                                Command::Remove { value } => {
                                    s.sent.retain(|b| key(b) != key(value));
                                    s.lanes.remove(&key(value));
                                    s.wire_sent.remove(&key(value));
                                }
                                Command::Box { value } | Command::Voxel { value } => {
                                    s.sent.retain(|b| key(b) != key(value));
                                    s.sent.push(value.clone());
                                    if let Some(seen) = &mut s.repair_seen {
                                        seen.insert(key(value));
                                    }
                                    s.lanes.insert(key(value), *lane);
                                    let surface = surface_for(&s, value);
                                    s.wire_sent.insert(key(value), surface);
                                }
                                Command::Pair { first, second } => {
                                    for (offset, value) in [first, second].into_iter().enumerate() {
                                        s.sent.retain(|b| key(b) != key(value));
                                        s.sent.push(value.clone());
                                        if let Some(seen) = &mut s.repair_seen {
                                            seen.insert(key(value));
                                        }
                                        s.lanes.insert(key(value), *lane + offset);
                                        let surface = surface_for(&s, value);
                                        s.wire_sent.insert(key(value), surface);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Job::Command(c) => {
                    s.seq = (seq + 1) % if legacy { 32 } else { 16 };
                    match c {
                        Command::Clear => {
                            s.sent.clear();
                            s.sent_revision = None;
                            s.brush = None;
                            s.resets = s.resets.saturating_sub(1)
                        }
                        Command::Brush {
                            style,
                            tint,
                            motion,
                        } => s.brush = Some((*style, *tint, *motion)),
                        Command::Remove { value } => s.sent.retain(|b| key(b) != key(value)),
                        Command::Box { value } | Command::Voxel { value } => {
                            s.sent.retain(|b| key(b) != key(value));
                            s.sent.push(value.clone());
                            s.brush = Some(appearance(value));
                        }
                        Command::Erase { .. } | Command::Pair { .. } => {}
                    }
                }
                Job::Controls(v) => {
                    for (k, v) in v {
                        if s.controls.get(k) == Some(v) {
                            s.controls.remove(k);
                        }
                    }
                }
                Job::Pulse(_) => s.reset_pulse = s.reset_pulse.saturating_sub(1),
            }
            let event = match &job {
                Job::Unified(plan) => {
                    json!({"seq":seq,"port":port,"protocol":"mf6","objects":plan.frame.objects(),"frame":plan.frame})
                }
                Job::Command(c) => json!({"seq":seq,"port":port,"command":c}),
                Job::Batch(batch) => json!({"seq":seq,"port":port,"lanes":batch}),
                Job::Controls(_) => json!({"port":port,"placement":true}),
                Job::Pulse(_) => json!({"port":port,"resetPulse":true}),
            };
            drop(s);
            if let Ok(mut w) = workspace.lock() {
                let _ = w.event("osc.sent", None, event);
            }
            next = Instant::now() + Duration::from_secs_f64(interval);
        }
        // Release the exclusive OSC lease before reporting shutdown complete.
        drop(channel);
        self.inner.lock().unwrap().connected = false;
    }
}

fn scale_valid(v: f32, profile: &str) -> bool {
    if ["mf5", "mf6"].contains(&profile) {
        (morphforge_core::material_wire::SCALE_MIN..=morphforge_core::material_wire::SCALE_MAX)
            .contains(&v)
    } else {
        (0.05..=1.).contains(&v)
    }
}
fn command_box(c: &Command) -> Option<&Box3> {
    match c {
        Command::Box { value } | Command::Voxel { value } => Some(value),
        Command::Pair { first, .. } => Some(first),
        _ => None,
    }
}
fn surface_for(s: &State, b: &Box3) -> morphforge_core::material_wire::Surface {
    s.wire_desired.get(&key(b)).copied().unwrap_or_else(|| {
        morphforge_core::material_wire::Surface::catalog(b.style_id, b.tint_id, b.motion_phase)
    })
}
fn wide_queues(s: &State, sent: &[Box3]) -> [Vec<(usize, Command)>; 3] {
    if s.config.profile != "mf5" {
        return protocol::wide_plan(sent, &s.desired, &s.lanes)
            .map(|v| v.into_iter().map(|c| (0, c)).collect());
    }
    let mut pools = protocol::pool_plan_by::<6>(sent, &s.desired, &s.lanes, |a, b| {
        (s.repair_seen
            .as_ref()
            .is_none_or(|seen| seen.contains(&key(a))))
            && s.wire_sent.get(&key(a)).copied().unwrap_or_else(|| {
                morphforge_core::material_wire::Surface::catalog(
                    a.style_id,
                    a.tint_id,
                    a.motion_phase,
                )
            }) == surface_for(s, b)
    });
    let deleting = pools
        .iter()
        .flatten()
        .any(|c| matches!(c, Command::Remove { .. } | Command::Erase { .. }));
    for queue in &mut pools {
        if deleting {
            queue.retain(|c| matches!(c, Command::Remove { .. } | Command::Erase { .. }));
        }
        queue.sort_by_key(|c| {
            let b = match c {
                Command::Remove { value } | Command::Erase { value } => Some(value),
                _ => command_box(c),
            };
            let volume = b.map(Box3::volume).unwrap_or(0);
            let pos = b
                .map(|b| (b.min[1], b.min[0], b.min[2]))
                .unwrap_or_default();
            let surface = b
                .map(|b| surface_for(s, b))
                .unwrap_or_else(|| morphforge_core::material_wire::Surface::catalog(0, 0, 0));
            match s.config.send_order.as_str() {
                "material" => (
                    ((surface.mode as u64) << 24)
                        | morphforge_core::material_wire::bits(surface.bytes) as u64,
                    pos.0,
                    pos.1,
                    pos.2,
                    u64::MAX - volume,
                ),
                "bottom_up" => (0, pos.0, pos.1, pos.2, u64::MAX - volume),
                _ => (u64::MAX - volume, pos.0, pos.1, pos.2, 0),
            }
        });
    }
    let paired = s
        .avatar_catalog
        .as_ref()
        .is_some_and(|c| c.capabilities.iter().any(|v| v == "mf5-dual-pool/1"));
    std::array::from_fn(|lane| {
        let mut a = std::collections::VecDeque::from(std::mem::take(&mut pools[lane * 2]));
        let mut b = std::collections::VecDeque::from(std::mem::take(&mut pools[lane * 2 + 1]));
        let mut out = Vec::new();
        let mut next = false;
        while !a.is_empty() || !b.is_empty() {
            if paired {
                if s.config.send_order == "material" {
                    if let Some(Command::Voxel { value: first }) = a.front() {
                        let surface = surface_for(s, first);
                        if surface.mode <= 2 {
                            if let Some(index) = b.iter().take(16).position(
                            |c| matches!(c,Command::Voxel{value} if surface_for(s,value)==surface),
                        ) {
                            if index > 0 {
                                let candidate = b.remove(index).unwrap();
                                b.push_front(candidate);
                            }
                        }
                        }
                    }
                }
                if let (
                    Some(Command::Voxel { value: first }),
                    Some(Command::Voxel { value: second }),
                ) = (a.front(), b.front())
                {
                    let surface = surface_for(s, first);
                    if surface.mode <= 2 && surface == surface_for(s, second) {
                        out.push((
                            0,
                            Command::Pair {
                                first: first.clone(),
                                second: second.clone(),
                            },
                        ));
                        a.pop_front();
                        b.pop_front();
                        continue;
                    }
                }
            }
            let secondary = if a.is_empty() {
                true
            } else if b.is_empty() {
                false
            } else if a.len() == b.len() {
                next = !next;
                next
            } else {
                b.len() > a.len()
            };
            out.push((
                usize::from(secondary),
                if secondary {
                    b.pop_front().unwrap()
                } else {
                    a.pop_front().unwrap()
                },
            ));
        }
        out
    })
}
fn validate_scene(scene: &Scene, capacity: usize, profile: &str) -> Result<()> {
    if profile != "mf6" && !scene.triangles.is_empty() {
        bail!("TRIANGLE_PROTOCOL_REQUIRED：三角面需要MF6头像，旧协议不会静默丢弃三角面");
    }
    if !["mf5", "mf6"].contains(&profile) {
        return Ok(protocol::validate_target(scene, capacity)?);
    }
    if scene.boxes.len() + scene.triangles.len() > capacity {
        bail!("MF5容量超限：{capacity} Box");
    }
    for t in &scene.triangles {
        t.wire_vertices()?;
    }
    for b in &scene.boxes {
        if profile != "mf6" && b.shape != 0 {
            bail!("SHAPE_PROTOCOL_REQUIRED：体素斜面需要MF6头像");
        }
        protocol::validate_geometry(b)?;
        if b.tint_id > 63 || b.motion_phase > 63 {
            bail!("染色/运动编码须在0..63");
        }
    }
    Ok(())
}

#[cfg(test)]
mod order_tests {
    use super::*;
    #[test]
    fn per_pool_volume_and_bottom_up_orders_are_deterministic() {
        let dir = tempfile::tempdir().unwrap();
        let w = Arc::new(Mutex::new(Workspace::open(dir.path().into()).unwrap()));
        let t = Transport::new(w);
        let mut s = t.inner.lock().unwrap();
        s.stop = true;
        s.config.profile = "mf5".into();
        let large = Box3::new([0, 20, 0], [8, 8, 8], 1);
        let small = Box3::new([0, 0, 0], [1; 3], 1);
        let other = Box3::new([10, 0, 0], [1; 3], 1);
        s.sent = vec![small.clone(), large.clone(), other.clone()];
        s.desired = s.sent.clone();
        s.lanes = [(key(&small), 0), (key(&large), 0), (key(&other), 0)]
            .into_iter()
            .collect();
        s.repair_seen = Some(Default::default());
        let q = wide_queues(&s, &s.sent);
        assert_eq!(command_box(&q[0][0].1).unwrap().size, [8, 8, 8]);
        s.config.send_order = "bottom_up".into();
        let q = wide_queues(&s, &s.sent);
        assert_eq!(command_box(&q[0][0].1).unwrap().min, [0, 0, 0]);
        assert_eq!(command_box(&q[0][1].1).unwrap().min, [10, 0, 0]);
    }
}
