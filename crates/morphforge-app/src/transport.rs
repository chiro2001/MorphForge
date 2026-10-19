use crate::store::Workspace;
use anyhow::{bail, Context, Result};
use fs2::FileExt;
use morphforge_core::protocol::{self, appearance, key, Command, OscValue};
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
    pub auto: bool,
    pub capacity: usize,
    pub refresh_seconds: u64,
    pub world: bool,
    pub visible: bool,
    pub scale: f32,
    pub level: bool,
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
            world: true,
            visible: true,
            scale: 1.,
            level: true,
        }
    }
}
pub struct State {
    pub config: Connection,
    pub avatar_catalog: Option<crate::target::AvatarCatalog>,
    pub avatar_catalog_error: String,
    pub sent: Vec<Box3>,
    pub desired: Vec<Box3>,
    pub desired_revision: u64,
    pub sent_revision: Option<u64>,
    pub seq: u8,
    pub brush: Option<(u32, u8, u8)>,
    pub resets: u8,
    pub connected: bool,
    pub last_error: String,
    pub sent_count: u64,
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
        let t = Self {
            inner: Arc::new(Mutex::new(State {
                config,
                avatar_catalog,
                avatar_catalog_error,
                sent: vec![],
                desired: vec![],
                desired_revision: 0,
                sent_revision: None,
                seq: 1,
                brush: None,
                resets: 0,
                connected: false,
                last_error: String::new(),
                sent_count: 0,
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
        let commands = protocol::plan(&s.sent, &s.desired, s.brush, s.config.profile == "mf2");
        let pending = commands.len()
            + s.resets as usize
            + usize::from(!s.controls.is_empty())
            + s.reset_pulse as usize;
        json!({"enabled":s.config.enabled,"sending":s.connected,"port":s.config.port,"interval":s.config.interval,"profile":s.config.profile,"auto":s.config.auto,"capacity":s.config.capacity,"refreshSeconds":s.config.refresh_seconds,"pending":pending,"etaSeconds":pending as f64*s.config.interval,"desiredRevision":s.desired_revision,"sentRevision":s.sent_revision,"sentCommands":s.sent_count,"error":s.last_error,"ack":false,"syncedBits":if s.config.profile=="mf3"{56}else{88},"placement":{"world":s.config.world,"visible":s.config.visible,"scale":s.config.scale,"level":s.config.level}})
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
        if config.port == 0
            || !config.interval.is_finite()
            || !(0.05..=5.).contains(&config.interval)
            || !config.scale.is_finite()
            || !(0.05..=1.).contains(&config.scale)
            || !["mf3", "mf2"].contains(&config.profile.as_str())
            || config.capacity == 0
            || config.capacity > 50000
        {
            bail!("连接配置越界")
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
            protocol::validate_target(&projected, config.capacity)?;
        }
        s.config = config;
        if restart {
            s.generation += 1;
            s.sent.clear();
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
            if c.profile == "mf3" {
                s.controls.insert("MF_Level".into(), Control::Bool(c.level));
            }
            protocol::validate_target(&workspace.project.scene, s.config.capacity)?;
            s.desired = crate::target::project(
                &workspace.project.scene,
                &workspace.project.materials,
                s.avatar_catalog.as_ref(),
            )?
            .boxes;
            s.desired_revision = workspace.project.scene.revision;
        }
        if !s.config.enabled {
            s.generation += 1;
            s.connected = false;
        }
        if s.config.enabled && s.config.auto && !old.auto {
            protocol::validate_target(&workspace.project.scene, s.config.capacity)?;
            s.desired = crate::target::project(
                &workspace.project.scene,
                &workspace.project.materials,
                s.avatar_catalog.as_ref(),
            )?
            .boxes;
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
        let projected = crate::target::project(scene, materials, profile.as_ref())?;
        self.publish(&projected, resync)
    }
    pub fn publish(&self, scene: &Scene, resync: bool) -> Result<Value> {
        let mut s = self.inner.lock().unwrap();
        protocol::validate_target(scene, s.config.capacity)?;
        s.desired = scene.boxes.clone();
        s.desired_revision = scene.revision;
        if resync {
            s.resets = s.resets.max(2);
            s.brush = None;
            s.sent_revision = None;
        }
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
        if scale.is_some_and(|f| !f.is_finite() || !(0.05..=1.).contains(&f)) {
            bail!("缩放范围0.05..1")
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
                Command(Command),
                Controls(Vec<(String, Control)>),
                Pulse(u8),
            }
            let mut s = self.inner.lock().unwrap();
            if !s.config.enabled || channel.as_ref().is_none_or(|(_, _, g)| *g != s.generation) {
                continue;
            }
            let job = {
                if s.config.refresh_seconds > 0
                    && s.last_refresh.elapsed().as_secs() >= s.config.refresh_seconds
                    && s.sent_revision == Some(s.desired_revision)
                {
                    s.resets = 2;
                    s.sent_revision = None;
                    s.last_refresh = Instant::now();
                }
                if s.resets > 0 {
                    Some(Job::Command(Command::Clear))
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
                    let command =
                        protocol::plan(&s.sent, &s.desired, s.brush, s.config.profile == "mf2")
                            .into_iter()
                            .next();
                    if command.is_none() {
                        if s.sent_revision != Some(s.desired_revision) {
                            s.last_refresh = Instant::now();
                        }
                        s.sent_revision = Some(s.desired_revision);
                    }
                    command.map(Job::Command)
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
                Job::Command(c) => protocol::packet(c, seq, legacy),
                Job::Controls(v) => Ok(protocol::bundle(
                    &v.iter()
                        .map(|(k, v)| {
                            (
                                k.clone(),
                                match v {
                                    Control::Bool(b) => OscValue::Bool(*b),
                                    Control::Float(f) => OscValue::Float(*f),
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
                        Command::Erase { .. } => {}
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
                Job::Command(c) => json!({"seq":seq,"port":port,"command":c}),
                Job::Controls(_) => json!({"port":port,"placement":true}),
                Job::Pulse(_) => json!({"port":port,"resetPulse":true}),
            };
            drop(s);
            if let Ok(mut w) = workspace.lock() {
                let _ = w.event("osc.sent", None, event);
            }
            next = Instant::now() + Duration::from_secs_f64(interval);
        }
    }
}
