use crate::store::Workspace;
mod compact;
pub mod strategy;
pub mod unified;
use anyhow::{bail, Context, Result};
use fs2::FileExt;
use morphforge_core::protocol::{self, OscValue};
use morphforge_core::{Box3, Scene};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
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
    #[serde(default = "default_auto_detect")]
    pub auto_detect: bool,
    pub auto: bool,
    #[serde(default = "default_strategy")]
    pub publish_strategy: String,
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
fn default_auto_detect() -> bool {
    true
}
fn default_strategy() -> String {
    "auto".into()
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
            profile: "mf6".into(),
            auto: false,
            auto_detect: true,
            publish_strategy: default_strategy(),
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
    pub publication: Option<strategy::Decision>,
    pub discovery: Value,
    pub config: Connection,
    pub avatar_catalog: Option<crate::target::AvatarCatalog>,
    pub avatar_catalog_error: String,
    pub desired: Vec<Box3>,
    pub desired_revision: u64,
    pub source_signature: String,
    pub sent_revision: Option<u64>,
    pub seq: u8,
    pub wire_desired: BTreeMap<protocol::GeometryKey, morphforge_core::material_wire::Surface>,
    pub resets: u8,
    pub connected: bool,
    pub last_error: String,
    pub sent_count: u64,
    pub sent_frame_count: u64,
    pub queued_frame_estimate: Option<(u64, usize)>,
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
        let (mut avatar_catalog, mut avatar_catalog_error) =
            if root.join("avatar-catalog.json").exists() {
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
        config.profile = "mf6".into();
        if avatar_catalog
            .as_ref()
            .is_some_and(|c| !c.unified_protocol())
        {
            avatar_catalog = None;
            avatar_catalog_error = "旧协议目录已停用，请识别或导入当前统一协议头像目录".into();
        }

        if let Some(protocol) = avatar_catalog
            .as_ref()
            .and_then(crate::target::AvatarCatalog::wire_protocol)
        {
            config.profile = protocol.into();
            if protocol == "mf6" {
                config.scale = config.scale.clamp(
                    morphforge_core::material_wire::SCALE_MIN,
                    morphforge_core::material_wire::SCALE_MAX,
                );
            }
        }
        let t = Self {
            inner: Arc::new(Mutex::new(State {
                unified: unified::Session::default(),
                publication: None,
                discovery: json!({"status":"not_queried","executionAck":false}),
                config,
                avatar_catalog,
                avatar_catalog_error,
                desired: vec![],
                desired_revision: 0,
                source_signature: String::new(),
                sent_revision: None,
                seq: 1,
                wire_desired: BTreeMap::new(),
                resets: 0,
                connected: false,
                last_error: String::new(),
                sent_count: 0,
                sent_frame_count: 0,
                queued_frame_estimate: None,
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
        let command_batches = s
            .queued_frame_estimate
            .map(|(start, n)| n.saturating_sub(s.sent_frame_count.saturating_sub(start) as usize))
            .unwrap_or_else(|| s.unified.pending_objects().div_ceil(3) + s.resets as usize);
        let pending =
            command_batches + usize::from(!s.controls.is_empty()) + s.reset_pulse as usize;
        let progress = if let Some((start, total)) = s.queued_frame_estimate {
            let sent = (s.sent_frame_count.saturating_sub(start) as usize).min(total);
            json!({"totalFrames":total,"sentFrames":sent,"remainingFrames":total-sent,
                "percent":if total==0{100.}else{sent as f64/total as f64*100.},
                "kind":"local_udp_sent","receiverConfirmed":false})
        } else {
            json!({"totalFrames":null,"kind":"local_udp_sent","receiverConfirmed":false})
        };
        json!({"progress":progress,"enabled":s.config.enabled,"sending":s.connected,"port":s.config.port,"interval":s.config.interval,"profile":s.config.profile,"auto":s.config.auto,"capacity":s.config.capacity,"refreshSeconds":s.config.refresh_seconds,"loopReplay":s.config.loop_replay,"sendOrder":s.config.send_order,"replayCycles":s.replay_cycles,"pending":pending,"etaSeconds":pending as f64*s.config.interval,"desiredRevision":s.desired_revision,"sentRevision":s.sent_revision,"sentCommands":s.sent_count,"error":s.last_error,"ack":false,"baselineKind":"local_udp_sent","publication":s.publication,"publishStrategy":s.config.publish_strategy,"autoDetect":s.config.auto_detect,"discovery":s.discovery,"catalogIdentity":s.avatar_catalog.as_ref().map(|c|c.identity_hash()),"remoteAcknowledged":false,"sentObjects":json!({"boxes":s.unified.sent_counts().0,"triangles":s.unified.sent_counts().1}),"repairing":s.unified.repairing(),"syncedBits":240,"repairDeletions":s.unified.repair_history_counts(),"pendingObjects":s.unified.pending_objects(),"estimateKind":"adaptive-layout estimate; no receiver acknowledgement","placement":{"world":s.config.world,"visible":s.config.visible,"scale":s.config.scale,"level":s.config.level}})
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
        let config: Connection = serde_json::from_value(next)?;
        if !["volume", "bottom_up", "material"].contains(&config.send_order.as_str()) {
            bail!("发送顺序需要volume/bottom_up/material");
        }
        if !["auto", "incremental", "rebuild"].contains(&config.publish_strategy.as_str()) {
            bail!("发布策略须为auto/incremental/rebuild");
        }
        if config.port == 0
            || !config.interval.is_finite()
            || !(0.05..=5.).contains(&config.interval)
            || !config.scale.is_finite()
            || !scale_valid(config.scale, &config.profile)
            || config.profile != "mf6"
            || config.capacity == 0
            || config.capacity > 50000
        {
            bail!("连接配置越界")
        }
        if config.capacity > 16384 {
            bail!("MF6_CAPACITY：头像总容量为16384对象");
        }
        if config.enabled {
            let declared = s.discovery["info"]["catalogHash"].as_str();
            if let Some(hash) = declared {
                if !s
                    .avatar_catalog
                    .as_ref()
                    .is_some_and(|c| c.identity_hash() == hash)
                {
                    bail!("AVATAR_CATALOG_NOT_MATCHED：当前Avatar的材质目录尚未匹配，请先导入其构建目录");
                }
            }
            if config.port == 9000
                && s.avatar_catalog.as_ref().is_some_and(|c| {
                    c.capabilities
                        .iter()
                        .any(|v| v == morphforge_core::mf6::REGION_CAPABILITY)
                })
                && (declared.is_none() || s.discovery["info"]["region8"] != true)
            {
                bail!("AVATAR_REGION_NOT_CONFIRMED：请更新/重新切换头像并识别能力后使用区域编码");
            }
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
            s.queued_frame_estimate = None;
            s.unified.region_enabled = s.avatar_catalog.as_ref().is_some_and(|c| {
                c.capabilities
                    .iter()
                    .any(|v| v == morphforge_core::mf6::REGION_CAPABILITY)
            });
            s.generation += 1;
            s.seq = 1;
            s.unified.clear_sent();
            s.sent_revision = None;
            s.resets = 2;
            s.last_error.clear();
            s.controls.clear();
            let c = s.config.clone();
            s.controls.insert("MF_World".into(), Control::Bool(c.world));
            s.controls
                .insert("MF_Handle".into(), Control::Bool(c.visible));
            s.controls
                .insert("MF_Scale".into(), Control::Float(c.scale));
            s.controls.insert("MF_Level".into(), Control::Bool(c.level));
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
        if catalog.as_ref().is_some_and(|c| !c.unified_protocol()) {
            bail!("旧OSC协议已移除；需要统一240bit头像目录");
        }
        let mut s = self.inner.lock().unwrap();
        if s.config.enabled {
            bail!("请先停止OSC发送，再更换Avatar目录");
        }
        if let Some(ref c) = catalog {
            crate::avatar_discovery::cache(root, c)?;
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
        self.publish_projection(&projected.scene, projected.surfaces, resync, true)
    }
    pub fn publish(&self, scene: &Scene, resync: bool) -> Result<Value> {
        self.publish_projection(scene, BTreeMap::new(), resync, true)
    }
    pub fn publish_for_repair(
        &self,
        scene: &Scene,
        materials: &[morphforge_core::materials::Material],
    ) -> Result<Value> {
        let catalog = self.inner.lock().unwrap().avatar_catalog.clone();
        let projected = crate::target::project_wire(scene, materials, catalog.as_ref())?;
        let signature = source_signature(&projected.scene, &projected.surfaces)?;
        let same = self.inner.lock().unwrap().source_signature == signature;
        if !same {
            self.publish_projection(&projected.scene, projected.surfaces, false, false)?;
        }
        self.repair()
    }
    fn publish_projection(
        &self,
        scene: &Scene,
        surfaces: BTreeMap<protocol::GeometryKey, morphforge_core::material_wire::Surface>,
        resync: bool,
        allow_auto: bool,
    ) -> Result<Value> {
        let signature = source_signature(scene, &surfaces)?;
        let mut s = self.inner.lock().unwrap();
        validate_scene(scene, s.config.capacity, &s.config.profile)?;
        let regions = s.avatar_catalog.as_ref().is_some_and(|c| {
            c.capabilities
                .iter()
                .any(|v| v == morphforge_core::mf6::REGION_CAPABILITY)
        });
        let mut original = s.unified.clone();
        original.region_enabled = regions;
        original.set_target(scene, &surfaces)?;
        let policy = if resync {
            "rebuild"
        } else if !allow_auto {
            "incremental"
        } else {
            &s.config.publish_strategy
        };
        let mut decision = strategy::compare(
            &original,
            &s.config.send_order,
            s.config.interval,
            s.resets as usize,
            policy,
        );
        let catalog = s.avatar_catalog.as_ref();
        let (compact_scene, compact_surfaces) =
            compact::candidate(scene, &surfaces, |b, surface, axis| {
                if surface.mode == 1 {
                    return true;
                }
                if surface.mode == 2 {
                    return surface.decode().is_some_and(|v| v.opacity >= 1.);
                }
                if surface.mode != 0 && surface.mode != 7 {
                    return false;
                }
                let id = morphforge_core::material_wire::bits(surface.bytes) & 4095;
                let Some(m) = catalog.and_then(|c| c.materials.iter().find(|m| m.id == id)) else {
                    return false;
                };
                if m.opacity < 1.
                    || m.render_mode != "opaque"
                    || ["water", "cloud"].contains(&m.family.as_str())
                {
                    return false;
                }
                if m.textures.is_empty() {
                    return true;
                }
                if m.texture_scale <= 0. {
                    return false;
                }
                let periods = b.size[axis] as f32 / m.texture_scale;
                (periods - periods.round()).abs() < 0.00001
            });
        let mut selected_scene = scene.clone();
        let mut selected_surfaces = surfaces;
        let mut selected = original;
        if compact_scene.boxes.len() < scene.boxes.len() {
            let mut merged = s.unified.clone();
            merged.region_enabled = regions;
            merged.set_target(&compact_scene, &compact_surfaces)?;
            let mut alternative = strategy::compare(
                &merged,
                &s.config.send_order,
                s.config.interval,
                s.resets as usize,
                policy,
            );
            let cost = |d: &strategy::Decision| {
                if d.selected == "rebuild" {
                    d.rebuild_frames
                } else {
                    d.incremental_frames
                }
            };
            if cost(&alternative).is_some_and(|v| cost(&decision).is_none_or(|old| v < old)) {
                alternative.layout = "run_boxes".into();
                alternative.compacted_boxes = scene.boxes.len() - compact_scene.boxes.len();
                decision = alternative;
                selected = merged;
                selected_scene = compact_scene;
                selected_surfaces = compact_surfaces;
            }
        }
        if !allow_auto && decision.incremental_frames.is_none() {
            bail!("保留显示补播无法重新平衡当前池，请使用清空重建");
        }
        s.unified = selected;
        s.source_signature = signature;
        if allow_auto && decision.selected == "rebuild" {
            s.resets = s.resets.max(2);
            s.sent_revision = None;
        }
        let frames = if decision.selected == "rebuild" {
            decision.rebuild_frames
        } else {
            decision.incremental_frames
        };
        s.queued_frame_estimate = frames.map(|n| (s.sent_frame_count, n));
        s.publication = Some(decision);
        let scene = &selected_scene;
        let surfaces = selected_surfaces;
        s.wire_desired = surfaces;
        s.desired = scene.boxes.clone();
        s.desired_revision = scene.revision;
        if resync {
            s.resets = s.resets.max(2);
            s.sent_revision = None;
        }
        drop(s);
        Ok(self.state())
    }
    /// Idempotent replay preserves native pool assignments and existing display.
    /// This is blind repair, never a receiver acknowledgement.
    pub fn repair(&self) -> Result<Value> {
        let mut s = self.inner.lock().unwrap();
        s.unified.start_replay();
        s.queued_frame_estimate = strategy::frames(s.unified.clone(), &s.config.send_order)
            .ok()
            .map(|n| (s.sent_frame_count, n + s.resets as usize));
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
            bail!("缩放范围为1%..1000%（实际scale .0005.. .5）")
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
        enum Job {
            Frame(unified::Plan),
            Controls(Vec<(String, Control)>),
            Pulse(u8),
        }
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
                std::thread::sleep(Duration::from_millis(20));
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
                    Ok((file, UdpSocket::bind("127.0.0.1:0")?, generation))
                })();
                match result {
                    Ok(c) => {
                        channel = Some(c);
                        self.inner.lock().unwrap().connected = true;
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
            let mut s = self.inner.lock().unwrap();
            if !s.config.enabled || s.generation != generation {
                continue;
            }
            let job = if s.resets > 0 {
                Some(Job::Frame(unified::Plan {
                    frame: morphforge_core::mf6::Frame::Clear,
                    actions: vec![],
                }))
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
                // A replay starts only after the previous complete target was
                // sent. Looping never adds an implicit Clear.
                let should_refresh = s.sent_revision == Some(s.desired_revision)
                    && !s.unified.repairing()
                    && s.unified.has_replay_work()
                    && (s.config.loop_replay
                        || (s.config.refresh_seconds > 0
                            && s.last_refresh.elapsed().as_secs() >= s.config.refresh_seconds));
                if should_refresh {
                    s.unified.start_replay();
                    s.queued_frame_estimate =
                        strategy::frames(s.unified.clone(), &s.config.send_order)
                            .ok()
                            .map(|n| (s.sent_frame_count, n));
                    s.sent_revision = None;
                    s.last_refresh = Instant::now();
                }
                match s.unified.next(&s.config.send_order) {
                    Ok(Some(plan)) => Some(Job::Frame(plan)),
                    Ok(None) => {
                        if s.unified.repairing() {
                            s.replay_cycles += 1;
                            s.unified.finish_replay();
                        }
                        if s.sent_revision != Some(s.desired_revision) {
                            s.last_refresh = Instant::now();
                        }
                        s.sent_revision = Some(s.desired_revision);
                        None
                    }
                    Err(e) => {
                        s.config.enabled = false;
                        s.last_error = e.to_string();
                        continue;
                    }
                }
            };
            let Some(job) = job else {
                drop(s);
                std::thread::sleep(Duration::from_millis(10));
                continue;
            };
            let seq = s.seq;
            let port = s.config.port;
            let interval = s.config.interval;
            let encoded = match &job {
                Job::Frame(p) => p.frame.packet(seq),
                Job::Controls(values) => Ok(protocol::bundle(
                    &values
                        .iter()
                        .map(|(name, value)| {
                            (
                                name.clone(),
                                match value {
                                    Control::Bool(v) => OscValue::Bool(*v),
                                    Control::Float(v) => OscValue::Float(if name == "MF_Scale" {
                                        morphforge_core::material_wire::scale_wire(*v)
                                    } else {
                                        *v
                                    }),
                                },
                            )
                        })
                        .collect::<Vec<_>>(),
                )),
                Job::Pulse(p) => {
                    let mut v = vec![("MF_Reset".into(), OscValue::Bool(*p == 2))];
                    if *p == 2 {
                        v.push(("MF_GrabL".into(), OscValue::Bool(false)));
                        v.push(("MF_GrabR".into(), OscValue::Bool(false)));
                    }
                    Ok(protocol::bundle(&v))
                }
            };
            let sent = encoded.map_err(anyhow::Error::from).and_then(|bytes| {
                Ok(channel
                    .as_ref()
                    .unwrap()
                    .1
                    .send_to(&bytes, ("127.0.0.1", port))?)
            });
            if let Err(e) = sent {
                s.config.enabled = false;
                s.last_error = e.to_string();
                continue;
            }
            s.sent_count += 1;
            let event = match &job {
                Job::Frame(p) => {
                    s.sent_frame_count += 1;
                    s.seq = (seq + 1) % 16;
                    if matches!(p.frame, morphforge_core::mf6::Frame::Clear) {
                        s.unified.clear_sent();
                        s.sent_revision = None;
                        s.resets = s.resets.saturating_sub(1);
                    } else {
                        s.unified.commit(p);
                    }
                    json!({"seq":seq,"port":port,"protocol":"mf6","objects":p.frame.objects(),"frame":p.frame})
                }
                Job::Controls(values) => {
                    for (k, v) in values {
                        if s.controls.get(k) == Some(v) {
                            s.controls.remove(k);
                        }
                    }
                    json!({"port":port,"placement":true})
                }
                Job::Pulse(_) => {
                    s.reset_pulse = s.reset_pulse.saturating_sub(1);
                    json!({"port":port,"resetPulse":true})
                }
            };
            drop(s);
            if let Ok(mut w) = workspace.lock() {
                let _ = w.event("osc.sent", None, event);
            }
            next = Instant::now() + Duration::from_secs_f64(interval);
        }
        drop(channel);
        self.inner.lock().unwrap().connected = false;
    }
}
fn source_signature(
    scene: &Scene,
    surfaces: &BTreeMap<protocol::GeometryKey, morphforge_core::material_wire::Surface>,
) -> Result<String> {
    Ok(crate::store::hash(&serde_json::to_vec(&(
        scene,
        surfaces.iter().collect::<Vec<_>>(),
    ))?))
}
fn scale_valid(value: f32, _profile: &str) -> bool {
    (morphforge_core::material_wire::SCALE_MIN..=morphforge_core::material_wire::SCALE_MAX)
        .contains(&value)
}
fn validate_scene(scene: &Scene, capacity: usize, _profile: &str) -> Result<()> {
    if scene.boxes.len() + scene.triangles.len() > capacity {
        bail!("MF6容量超限：{capacity}对象")
    }
    for t in &scene.triangles {
        t.wire_vertices()?;
    }
    for b in &scene.boxes {
        protocol::validate_geometry(b)?;
        if b.shape > 4 || b.tint_id > 63 || b.motion_phase > 63 {
            bail!("几何模板/染色/运动编码越界")
        }
    }
    Ok(())
}
