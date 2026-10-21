pub mod config;
pub mod engine;
mod models;
pub mod native;
pub mod tencent;
use crate::{harness::Harness, store::Workspace, transport::Transport};
use anyhow::{bail, Context, Result};
use base64::Engine as _;
use config::{Credentials, Settings};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU8, Ordering},
        mpsc::{self, SyncSender},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
#[derive(Clone)]
pub struct SpeechContext {
    pub workspace: Arc<Mutex<Workspace>>,
    pub harness: Harness,
    pub transport: Transport,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub kind: String,
    pub panel_id: String,
    pub project_id: String,
}
pub struct Frame {
    pub samples: Vec<f32>,
    pub captured_ms: f64,
}
struct Session {
    id: String,
    source: String,
    tx: SyncSender<Frame>,
    stop: Arc<AtomicU8>,
    start: Instant,
}
pub struct Speech {
    runtime: Mutex<Option<tokio::runtime::Handle>>,
    download: Mutex<Value>,
    download_stop: AtomicU8,
    root: PathBuf,
    settings: Mutex<Settings>,
    credentials: Mutex<Credentials>,
    state: Mutex<Value>,
    session: Mutex<Option<Session>>,
    engines: Mutex<Option<engine::Engines>>,
}
impl Speech {
    pub fn new(root: PathBuf) -> Result<Arc<Self>> {
        let settings = Settings::load(&root)?;
        config::ensure_packed_vad(&settings.vad_path())?;
        Ok(Arc::new(Self {
            runtime: Mutex::new(tokio::runtime::Handle::try_current().ok()),
            download: Mutex::new(json!({"active":false})),
            download_stop: AtomicU8::new(0),
            settings: Mutex::new(settings),
            credentials: Mutex::new(Credentials::load(&root)?),
            root,
            state: Mutex::new(
                json!({"active":false,"phase":"idle","partial":"","finalText":"","draft":"","sequence":0,"sentences":[],"level":0.,"error":null,"metrics":{}}),
            ),
            session: Mutex::new(None),
            engines: Mutex::new(None),
        }))
    }
    pub fn brief_status(&self) -> Value {
        let s = self.state.lock().unwrap();
        json!({"active":s["active"],"phase":s["phase"],"target":s["target"]})
    }
    pub fn status(&self) -> Value {
        let settings = self.settings.lock().unwrap().clone();
        let state = self.state.lock().unwrap().clone();
        let configured = self.credentials.lock().unwrap().configured();
        json!({"settings":settings,"credentialsConfigured":configured,"models":settings.models(),"state":state,"download":self.download.lock().unwrap().clone()})
    }
    pub fn configure(&self, patch: &Value) -> Result<Value> {
        let fields = patch.as_object().context("语音配置需要对象")?;
        if self.download.lock().unwrap()["active"] == true {
            bail!("请等待模型下载完成后修改语音配置")
        }
        if self.state.lock().unwrap()["active"] == true
            && fields
                .keys()
                .any(|k| !matches!(k.as_str(), "autoCaption" | "keywordEnabled" | "keyword"))
        {
            bail!("请结束录音后再更换模型、设备或凭证")
        }
        if patch["importReference"] == true {
            let c = Credentials::import_reference()?;
            c.validate()?;
            self.save_credentials(&c)?;
            *self.credentials.lock().unwrap() = c;
            return Ok(self.status());
        }
        if let Some(v) = patch.get("credentials") {
            let c: Credentials = serde_json::from_value(v.clone())?;
            c.validate()?;
            self.save_credentials(&c)?;
            *self.credentials.lock().unwrap() = c;
        }
        let mut current = serde_json::to_value(self.settings.lock().unwrap().clone())?;
        for (k, v) in fields {
            if k == "credentials" {
                continue;
            }
            if current.get(k).is_none() {
                bail!("未知语音设置{k}")
            }
            current[k] = v.clone();
        }
        let mut next: Settings = serde_json::from_value(current)?;
        if next.model_root.as_os_str().is_empty() {
            next.model_root = self.root.join("models/speech");
        }
        next.validate()?;
        crate::store::write_json(&self.root.join("speech-settings.json"), &next)?;
        *self.settings.lock().unwrap() = next;
        Ok(self.status())
    }
    fn save_credentials(&self, c: &Credentials) -> Result<()> {
        let path = self.root.join("speech-credentials.json");
        crate::store::write_json(&path, c)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }
    fn update(&self, id: &str, change: impl FnOnce(&mut Value)) {
        let mut state = self.state.lock().unwrap();
        if state["sessionId"] == id {
            change(&mut state)
        }
    }
    fn target(context: &SpeechContext, value: &Value) -> Result<Target> {
        let mut t: Target = if value.is_null() {
            Target::default()
        } else {
            serde_json::from_value(value.clone())?
        };
        let w = context.workspace.lock().unwrap();
        if t.kind.is_empty() {
            t.kind = "modeling".into()
        }
        if !matches!(t.kind.as_str(), "modeling" | "text_panel" | "voxel_input") {
            bail!("语音目标须为modeling/text_panel")
        }
        if !t.project_id.is_empty() && t.project_id != w.project.id {
            bail!("WORKSPACE_CONFLICT")
        };
        t.project_id = w.project.id.clone();
        if t.kind == "text_panel"
            && !w
                .project
                .scene
                .text_panels
                .iter()
                .any(|p| p.id == t.panel_id)
        {
            bail!("请先选择一个文本框")
        };
        Ok(t)
    }
    pub fn start(self: &Arc<Self>, context: SpeechContext, value: &Value) -> Result<Value> {
        if self.download.lock().unwrap()["active"] == true {
            bail!("模型下载中，请稍后开始录音")
        }
        let runtime = tokio::runtime::Handle::try_current().context("语音服务需要异步运行时")?;
        *self.runtime.lock().unwrap() = Some(runtime.clone());
        let mut owner = self.session.lock().unwrap();
        if owner.is_some() {
            bail!("语音会话仍在运行或结束中")
        }
        let settings = self.settings.lock().unwrap().clone();
        settings.validate()?;
        let credentials = self.credentials.lock().unwrap().clone();
        if settings.provider == "tencent" && !credentials.configured() {
            bail!("腾讯云凭证尚未配置")
        }
        let target = Self::target(&context, &value["target"])?;
        let source =
            value["source"]
                .as_str()
                .unwrap_or(if cfg!(windows) { "device" } else { "pcm" });
        if !matches!(source, "device" | "pcm") {
            bail!("语音输入源无效")
        }
        let id = uuid::Uuid::new_v4().to_string();
        let start = Instant::now();
        let stop = Arc::new(AtomicU8::new(0));
        let (tx, rx) = mpsc::sync_channel(512);
        *self.state.lock().unwrap() = json!({"sessionId":id,"active":true,"phase":"loading","provider":settings.provider,"source":source,"target":target,"partial":"","finalText":"","draft":"","sequence":0,"sentences":[],"level":0.,"error":null,"metrics":{"cloudSessions":0,"audioSamples":0}});
        *owner = Some(Session {
            id: id.clone(),
            source: source.into(),
            tx,
            stop: stop.clone(),
            start,
        });
        drop(owner);
        let service = self.clone();
        let source = source.to_string();

        let failed_id = id.clone();
        let launched = std::thread::Builder::new()
            .name("morphforge-speech".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    service.worker(
                        &id,
                        start,
                        &settings,
                        credentials,
                        &source,
                        rx,
                        stop.clone(),
                        target,
                        context,
                        runtime,
                    )
                }))
                .unwrap_or_else(|_| Err(anyhow::anyhow!("语音引擎异常，请重启程序")));
                service.update(&id, |v| {
                    v["active"] = json!(false);
                    v["level"] = json!(0.);
                    v["partial"] = json!("");
                    v["phase"] = json!(if stop.load(Ordering::SeqCst) == 2 {
                        "cancelled"
                    } else if result.is_err() {
                        "error"
                    } else {
                        "idle"
                    });
                    if let Err(e) = &result {
                        v["error"] = json!(e.to_string());
                    }
                });
                let mut session = service.session.lock().unwrap();
                if session.as_ref().is_some_and(|s| s.id == id) {
                    *session = None;
                }
            });
        if let Err(error) = launched {
            self.update(&failed_id, |v| {
                v["active"] = json!(false);
                v["phase"] = json!("error");
                v["error"] = json!("无法创建语音线程");
            });
            let mut session = self.session.lock().unwrap();
            if session.as_ref().is_some_and(|s| s.id == failed_id) {
                *session = None;
            }
            return Err(error.into());
        }
        Ok(self.status())
    }
    pub fn audio(&self, value: &Value) -> Result<Value> {
        let session = self.session.lock().unwrap();
        let s = session.as_ref().context("没有正在录音的会话")?;
        if value["sessionId"] != s.id || s.source != "pcm" {
            bail!("语音会话或输入源不匹配")
        };
        let data = value["pcm"]
            .as_str()
            .context("需要16kHz单声道PCM16的base64")?;
        if data.len() > 44000 {
            bail!("音频块超过1秒")
        };
        let bytes = base64::engine::general_purpose::STANDARD.decode(data)?;
        if bytes.len() % 2 != 0 || bytes.len() > 32000 {
            bail!("无效PCM块")
        };
        let samples = bytes
            .chunks_exact(2)
            .map(|x| i16::from_le_bytes([x[0], x[1]]) as f32 / 32768.)
            .collect();
        s.tx.try_send(Frame {
            samples,
            captured_ms: s.start.elapsed().as_secs_f64() * 1000.,
        })
        .map_err(|_| anyhow::anyhow!("音频处理积压，请停止后重试"))?;
        Ok(json!({"ok":true}))
    }
    pub fn control(&self, value: &Value) -> Result<Value> {
        let action = value["action"].as_str().unwrap_or("");
        if action == "clear" {
            let mut v = self.state.lock().unwrap();
            v["draft"] = json!("");
            v["finalText"] = json!("");
            drop(v);
            return Ok(self.status());
        }
        let session = self.session.lock().unwrap();
        let Some(s) = session.as_ref() else {
            drop(session);
            if matches!(action, "stop" | "cancel") {
                return Ok(self.status());
            }
            bail!("没有正在录音的会话")
        };
        match action {
            "stop" => s.stop.store(1, Ordering::SeqCst),
            "cancel" => s.stop.store(2, Ordering::SeqCst),
            _ => bail!("语音控制须为stop/cancel/clear"),
        };
        self.update(&s.id, |v| {
            v["phase"] = json!(if action == "cancel" {
                "cancelling"
            } else {
                "finishing"
            })
        });
        drop(session);
        Ok(self.status())
    }
    pub fn apply(&self, context: &SpeechContext, value: &Value) -> Result<Value> {
        let text = value["text"].as_str().context("需要文字")?;
        let target = Self::target(context, &value["target"])?;
        let key = value["requestId"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        self.deliver(context, &target, text, value["submit"] == true, &key)?;
        {
            let mut state = self.state.lock().unwrap();
            state["target"] = json!(target);
            state["draft"] = json!(text);
            state["applyRequestId"] = json!(key);
            let n = state["draftVersion"].as_u64().unwrap_or(0) + 1;
            state["draftVersion"] = json!(n);
        }
        Ok(json!({"ok":true}))
    }
    fn deliver(
        &self,
        context: &SpeechContext,
        target: &Target,
        text: &str,
        submit: bool,
        key: &str,
    ) -> Result<()> {
        if text.len() > 32000 {
            bail!("识别文字超过32k字节")
        }
        {
            let w = context.workspace.lock().unwrap();
            if w.project.id != target.project_id {
                bail!("WORKSPACE_CONFLICT：录音目标项目已切换，文字保留但未应用")
            }
        }
        if target.kind == "modeling" || target.kind == "voxel_input" {
            {
                let mut v = self.state.lock().unwrap();
                v["draft"] = json!(text);
                let n = v["draftVersion"].as_u64().unwrap_or(0);
                v["draftVersion"] = json!(n + 1);
            }
            if submit && target.kind == "modeling" && !text.trim().is_empty() {
                let runtime = tokio::runtime::Handle::try_current()
                    .ok()
                    .or_else(|| self.runtime.lock().unwrap().clone())
                    .context("模型服务运行时不可用")?;
                let _entered = runtime.enter();
                context
                    .harness
                    .message(&json!({"prompt":text,"mode":"steer","requestId":key,"projectId":target.project_id}))?;
                self.state.lock().unwrap()["submittedRequestId"] = json!(key);
            }
            return Ok(());
        }
        context.harness.ensure_manual_edit().context("字幕已保留在语音草稿；请暂停AI后点击应用，避免修改正在建模的场景")?;
        let mut w = context.workspace.lock().unwrap();
        if w.project.id != target.project_id {
            bail!("WORKSPACE_CONFLICT：录音目标项目已切换");
        }
        let mut panel = w
            .project
            .scene
            .text_panels
            .iter()
            .find(|p| p.id == target.panel_id)
            .cloned()
            .context("目标文本框已删除")?;
        panel.text = text.into();
        let base = w.project.scene.revision;
        w.apply(
            &[morphforge_core::Operation::TextPanel { value: panel }],
            Some(base),
            Some(key),
            None,
        )?;
        if context.transport.config().enabled && context.transport.config().auto {
            w.commit(None, "语音字幕更新", None)?;
            context.transport.publish_with_materials(
                &w.project.scene,
                &w.project.materials,
                false,
            )?;
        }
        Ok(())
    }
    fn partial(&self, id: &str, text: String) {
        self.update(id, |v| {
            v["partial"] = json!(text);
        });
    }
    fn publish(
        &self,
        id: &str,
        text: String,
        inference_ms: f64,
        latency_ms: Option<f64>,
        target: &Target,
        context: &SpeechContext,
        stop: &AtomicU8,
    ) {
        if stop.load(Ordering::SeqCst) >= 2
            || text.trim().is_empty()
            || self.state.lock().unwrap()["sessionId"] != id
        {
            return;
        }
        let settings = self.settings.lock().unwrap().clone();
        let mut key = String::new();
        let mut draft = String::new();
        self.update(id,|v|{let seq=v["sequence"].as_u64().unwrap_or(0)+1;v["sequence"]=json!(seq);let version=v["draftVersion"].as_u64().unwrap_or(0)+1;v["draftVersion"]=json!(version);v["partial"]=json!("");v["finalText"]=json!(text);draft=format!("{}{}{}",v["draft"].as_str().unwrap_or(""),if v["draft"].as_str().unwrap_or("").is_empty(){""}else{"\n"},text);if draft.len()<=32000{v["draft"]=json!(draft);}else{v["error"]=json!("识别草稿超过32k字节，请先发送或清空");stop.store(1,Ordering::SeqCst);}key=format!("speech-{id}-{seq}");let records=v["sentences"].as_array_mut().unwrap();records.push(json!({"sequence":seq,"text":text,"inferenceMs":inference_ms,"finalLatencyMs":latency_ms}));if records.len()>32{records.remove(0);}});
        let result = if target.kind == "text_panel" && settings.auto_caption {
            self.deliver(context, target, &draft, false, &key)
        } else if target.kind == "modeling" && settings.keyword_enabled {
            if let Some(prompt) = keyword_prompt(&draft, &settings.keyword) {
                if prompt.is_empty() {
                    Ok(())
                } else {
                    self.deliver(context, target, &prompt, true, &key).map(|_| {
                        self.update(id, |v| v["draft"] = json!(""));
                    })
                }
            } else {
                Ok(())
            }
        } else {
            Ok(())
        };
        if let Err(e) = result {
            self.update(id, |v| v["error"] = json!(e.to_string()));
        }
    }
    fn worker(
        self: &Arc<Self>,
        id: &str,
        start: Instant,
        settings: &Settings,
        credentials: Credentials,
        source: &str,
        rx: mpsc::Receiver<Frame>,
        stop: Arc<AtomicU8>,
        target: Target,
        context: SpeechContext,
        runtime: tokio::runtime::Handle,
    ) -> Result<()> {
        let mut cache = self.engines.lock().unwrap();
        let signature = serde_json::to_string(&json!([
            settings.provider,
            settings.language,
            settings.model_root,
            settings.threads
        ]))?;
        let warm = cache.as_ref().is_some_and(|c| c.signature == signature);
        if !warm {
            *cache = Some(engine::Engines::load(settings)?)
        }
        let engines = cache.as_ref().unwrap();
        let vad = engine::vad(settings)?;
        let tx = {
            let session = self.session.lock().unwrap();
            session
                .as_ref()
                .filter(|s| s.id == id)
                .context("语音会话已取消")?
                .tx
                .clone()
        };
        let capture = if source == "device" {
            Some(native::capture(
                settings.device.as_deref(),
                tx,
                stop.clone(),
                start,
            )?)
        } else {
            None
        };
        self.update(id, |v| {
            v["phase"] = json!("recording");
            v["metrics"]["loadingMs"] = json!(start.elapsed().as_secs_f64() * 1000.);
            v["metrics"]["warmModel"] = json!(warm);
        });
        let mut pre = std::collections::VecDeque::with_capacity(4800);
        let mut online_stream = None;
        let mut cloud: Option<tokio::sync::mpsc::Sender<Vec<f32>>> = None;
        let mut cloud_tasks = vec![];
        let mut speaking = false;
        let mut last_ms = 0.;
        let mut samples_total = 0u64;
        let mut first_partial = false;
        let mut last_input = Instant::now();
        loop {
            if stop.load(Ordering::SeqCst) >= 2 {
                break;
            }
            if start.elapsed() > Duration::from_secs(1800) {
                stop.store(1, Ordering::SeqCst);
                break;
            }
            let frame = if stop.load(Ordering::SeqCst) == 1 {
                match rx.try_recv() {
                    Ok(f) => f,
                    Err(_) => break,
                }
            } else {
                match rx.recv_timeout(Duration::from_millis(20)) {
                    Ok(f) => f,
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        if source == "pcm"
                            && last_input.elapsed()
                                > Duration::from_secs(if samples_total == 0 { 60 } else { 10 })
                        {
                            bail!("浏览器音频输入已断开");
                        }
                        continue;
                    }
                    Err(_) => break,
                }
            };
            last_input = Instant::now();
            if frame.samples.is_empty() {
                continue;
            }
            last_ms = frame.captured_ms;
            samples_total += frame.samples.len() as u64;
            let level = (frame.samples.iter().map(|v| v * v).sum::<f32>()
                / frame.samples.len() as f32)
                .sqrt()
                .clamp(0., 1.);
            self.update(id, |v| {
                v["level"] = json!(level);
                v["metrics"]["audioSamples"] = json!(samples_total);
            });
            for &sample in &frame.samples {
                pre.push_back(sample);
                if pre.len() > 4800 {
                    pre.pop_front();
                }
            }
            vad.accept_waveform(&frame.samples);
            let onset = !speaking && vad.detected();
            if onset {
                speaking = true;
                if let Some(online) = &engines.online {
                    online_stream = Some(online.create_stream());
                }
                if settings.provider == "tencent" {
                    let (tx, rx) = tokio::sync::mpsc::channel(128);
                    cloud = Some(tx);
                    self.update(id, |v| {
                        let n = v["metrics"]["cloudSessions"].as_u64().unwrap_or(0);
                        v["metrics"]["cloudSessions"] = json!(n + 1);
                    });
                    let partial_service = self.clone();
                    let partial_id = id.to_string();
                    let final_service = self.clone();
                    let final_id = id.to_string();
                    let final_target = target.clone();
                    let final_context = context.clone();
                    let final_stop = stop.clone();
                    let error_service = self.clone();
                    let error_id = id.to_string();
                    let config = settings.clone();
                    let creds = credentials.clone();
                    let task_stop = stop.clone();
                    cloud_tasks.push(runtime.spawn(async move {
                        let result = tencent::recognize(
                            creds,
                            config,
                            rx,
                            task_stop.clone(),
                            Arc::new(move |text| partial_service.partial(&partial_id, text)),
                            Arc::new(move |text| {
                                final_service.publish(
                                    &final_id,
                                    text,
                                    0.,
                                    None,
                                    &final_target,
                                    &final_context,
                                    &final_stop,
                                )
                            }),
                        )
                        .await;
                        if let Err(e) = result {
                            error_service.update(&error_id, |v| v["error"] = json!(e.to_string()));
                            task_stop.store(4, Ordering::SeqCst);
                        }
                    }));
                }
            }
            let audio = if onset {
                pre.iter().copied().collect::<Vec<_>>()
            } else {
                frame.samples.clone()
            };
            if let Some(tx) = &cloud {
                self.update(id, |v| {
                    let n = v["metrics"]["cloudAudioSamples"].as_u64().unwrap_or(0);
                    v["metrics"]["cloudAudioSamples"] = json!(n + audio.len() as u64);
                });
                tx.try_send(audio.clone())
                    .map_err(|_| anyhow::anyhow!("在线语音积压，已停止以避免丢音"))?;
            }
            if let (Some(online), Some(stream)) = (&engines.online, &online_stream) {
                stream.accept_waveform(16000, &audio);
                while online.is_ready(stream) {
                    online.decode(stream);
                }
                if let Some(result) = online.get_result(stream) {
                    let text = engine::clean(&result.text);
                    if !text.is_empty() {
                        if !first_partial {
                            first_partial = true;
                            self.update(id, |v| {
                                v["metrics"]["firstPartialMs"] =
                                    json!(start.elapsed().as_secs_f64() * 1000.)
                            });
                        }
                        self.partial(id, text);
                    }
                }
            }
            if self.drain_segments(
                id, start, engines, &vad, last_ms, settings, &target, &context, &stop,
            )? > 0
            {
                cloud.take();
                online_stream = None;
                speaking = false;
            }
        }
        drop(capture);
        if stop.load(Ordering::SeqCst) == 1 {
            vad.flush();
            self.drain_segments(
                id, start, engines, &vad, last_ms, settings, &target, &context, &stop,
            )?;
        }
        cloud.take();
        for mut task in cloud_tasks {
            if stop.load(Ordering::SeqCst) >= 2 {
                task.abort();
                continue;
            }
            if runtime
                .block_on(async { tokio::time::timeout(Duration::from_secs(10), &mut task).await })
                .is_err()
            {
                task.abort();
                self.update(id, |v| v["error"] = json!("在线句末结果等待超时"));
            }
        }
        match stop.load(Ordering::SeqCst) {
            3 => bail!("音频处理队列积压，录音已停止，未静默丢弃音频"),
            4 => bail!("语音设备或在线识别连接失败，请查看状态后重试"),
            _ => Ok(()),
        }
    }
    fn drain_segments(
        &self,
        id: &str,
        start: Instant,
        engines: &engine::Engines,
        vad: &sherpa_onnx::VoiceActivityDetector,
        last_ms: f64,
        settings: &Settings,
        target: &Target,
        context: &SpeechContext,
        stop: &AtomicU8,
    ) -> Result<usize> {
        let mut count = 0;
        while let Some(segment) = vad.front() {
            let samples = segment.samples().to_vec();
            vad.pop();
            count += 1;
            if settings.provider == "tencent" || stop.load(Ordering::SeqCst) >= 2 {
                continue;
            }
            let begin = Instant::now();
            let text = engines.recognize(&samples)?;
            let ms = begin.elapsed().as_secs_f64() * 1000.;
            let speech_end = (last_ms - settings.silence_seconds as f64 * 1000.).max(0.);
            let latency = (start.elapsed().as_secs_f64() * 1000. - speech_end).max(0.);
            self.publish(id, text, ms, Some(latency), target, context, stop);
        }
        Ok(count)
    }
}
/// Only an explicit command at a sentence edge can trigger submission. A final
/// standalone keyword submits the previously dictated buffer; partials never do.
pub fn keyword_prompt(text: &str, keyword: &str) -> Option<String> {
    let trim = |s: &str| {
        s.trim_matches(|c: char| c.is_whitespace() || "，。！？、,:;：；.!?".contains(c))
            .to_owned()
    };
    let text = text.trim();
    let last = text.lines().last().unwrap_or("");
    if last.ends_with('?') || last.ends_with('？') {
        return None;
    }
    let line = trim(last);
    if line == keyword {
        let before = text.rsplit_once(last).map(|p| p.0).unwrap_or("");
        return Some(trim(before));
    }
    if let Some(rest) = line.strip_prefix(keyword) {
        let rest = trim(rest);
        if rest.is_empty()
            || ["吗", "了吗", "是什么", "这个词", "功能"]
                .iter()
                .any(|s| rest.starts_with(s))
        {
            return None;
        }
        let before = text.rsplit_once(last).map(|p| p.0.trim()).unwrap_or("");
        return Some(if before.is_empty() {
            rest
        } else {
            format!("{before}\n{rest}")
        });
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keyword_is_explicit_and_preserves_buffer() {
        assert_eq!(
            keyword_prompt("一个红色房子\n开始建模", "开始建模"),
            Some("一个红色房子".into())
        );
        assert_eq!(
            keyword_prompt("开始建模，一个红色房子。", "开始建模"),
            Some("一个红色房子".into())
        );
        for text in [
            "不要开始建模",
            "我说的开始建模这个词",
            "开始建模了吗",
            "开始建模是什么",
        ] {
            assert_eq!(keyword_prompt(text, "开始建模"), None);
        }
    }
    #[test]
    fn tags_are_removed_without_changing_transcript() {
        assert_eq!(engine::clean("<|zh|><|NEUTRAL|>你好<|Speech|>"), "你好");
    }
}
