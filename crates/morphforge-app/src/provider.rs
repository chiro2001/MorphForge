use crate::store::{now, Workspace};
use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tokio::sync::Notify;
fn default_max_run_seconds() -> u64 {
    900
}
fn default_language() -> String {
    "zh-CN".into()
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettings {
    pub model: String,
    pub reasoning: String,
    pub max_output_tokens: u32,
    pub max_steps: u32,
    pub max_run_tokens: u64,
    #[serde(default = "default_max_run_seconds")]
    pub max_run_seconds: u64,
    pub compact_tokens: usize,
    pub auto_compact: bool,
    pub upload_images: bool,
    #[serde(default = "default_language")]
    pub language: String,
}
impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            model: "deepseek-flash".into(),
            reasoning: "high".into(),
            max_output_tokens: 16384,
            max_steps: 24,
            max_run_tokens: 250000,
            max_run_seconds: default_max_run_seconds(),
            compact_tokens: 60000,
            auto_compact: true,
            upload_images: true,
            language: default_language(),
        }
    }
}
#[derive(Clone)]
pub struct Provider {
    client: reqwest::Client,
    key: Option<String>,
    pub base: String,
    files: Arc<Mutex<BTreeMap<String, (String, i64)>>>,
    file_path: PathBuf,
}
struct CancellationInner {
    stopped: AtomicBool,
    notify: Notify,
}

#[derive(Clone)]
pub struct Cancellation(Arc<CancellationInner>);
impl Cancellation {
    pub fn new() -> Self {
        Self(Arc::new(CancellationInner {
            stopped: AtomicBool::new(false),
            notify: Notify::new(),
        }))
    }
    pub fn stop(&self) {
        self.0.stopped.store(true, Ordering::SeqCst);
        self.0.notify.notify_waiters();
    }
    pub fn stopped(&self) -> bool {
        self.0.stopped.load(Ordering::SeqCst)
    }
    pub async fn wait(&self) {
        loop {
            if self.stopped() {
                return;
            }
            let notified = self.0.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.stopped() {
                return;
            }
            notified.await;
        }
    }
}

impl Default for Cancellation {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Default)]
pub struct ProviderRequestTrace {
    pub request_id: Option<String>,
    pub prompt_version: Option<String>,
    pub tools_version: Option<String>,
    pub language: Option<String>,
    pub epoch: Option<u64>,
}

impl ProviderRequestTrace {
    fn event_fields(&self) -> Value {
        let mut fields = serde_json::Map::new();
        if let Some(value) = &self.prompt_version {
            fields.insert("promptVersion".into(), json!(value));
        }
        if let Some(value) = &self.tools_version {
            fields.insert("toolsVersion".into(), json!(value));
        }
        if let Some(value) = &self.language {
            fields.insert("language".into(), json!(value));
        }
        if let Some(value) = self.epoch {
            fields.insert("epoch".into(), json!(value));
        }
        Value::Object(fields)
    }
}
impl Provider {
    pub fn new(root: &Path, env_file: Option<&Path>) -> Result<Self> {
        let mut key = std::env::var("DEEPSEEK_API_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty());
        let mut base = std::env::var("MORPHFORGE_DEEPSEEK_BASE_URL")
            .unwrap_or_else(|_| "https://api.deepseek.com".into());
        let candidates = if let Some(p) = env_file {
            vec![p.to_path_buf()]
        } else {
            let home = std::env::var_os("USERPROFILE")
                .or_else(|| std::env::var_os("HOME"))
                .map(PathBuf::from)
                .unwrap_or_default();
            vec![
                home.join("litellm/.env"),
                home.join("AppData/Local/MorphForge/provider.env"),
            ]
        };
        for path in candidates {
            if key.is_some() {
                break;
            }
            if let Ok(iter) = dotenvy::from_path_iter(path) {
                for pair in iter.flatten() {
                    match pair.0.as_str() {
                        "DEEPSEEK_API_KEY" => {
                            if !pair.1.is_empty() {
                                key = Some(pair.1)
                            }
                        }
                        "MORPHFORGE_DEEPSEEK_BASE_URL" => base = pair.1,
                        _ => {}
                    }
                }
            }
        }
        Self::build(root, key, base)
    }
    #[doc(hidden)]
    pub fn new_for_loopback_test(root: &Path, base: &str) -> Result<Self> {
        let url = reqwest::Url::parse(base)?;
        if url.scheme() != "http" || url.host_str() != Some("127.0.0.1") {
            bail!("测试Provider只允许HTTP loopback")
        }
        Self::build(root, None, base.to_owned())
    }
    fn build(root: &Path, key: Option<String>, base: String) -> Result<Self> {
        let url = reqwest::Url::parse(&base)?;
        if (url.scheme() != "https" || url.host_str() != Some("api.deepseek.com"))
            && ![Some("127.0.0.1"), Some("localhost")].contains(&url.host_str())
        {
            bail!("Provider主机只支持官方DeepSeek或本机测试服务")
        }
        let path = root.join("provider-files.json");
        let files = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(240))
                .build()?,
            key,
            base: base.trim_end_matches('/').into(),
            files: Arc::new(Mutex::new(files)),
            file_path: path,
        })
    }
    pub fn redact(&self, text: &str) -> String {
        if let Some(key) = &self.key {
            text.replace(key, "[REDACTED]")
        } else {
            text.into()
        }
    }
    pub fn configured(&self) -> bool {
        self.key.is_some() || self.base.starts_with("http://127.0.0.1")
    }
    pub fn status(&self) -> Value {
        json!({"configured":self.configured(),"baseUrl":self.base,"interface":"responses","fileUpload":true,"visionModel":"deepseek-flash"})
    }
    fn auth(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(key) = &self.key {
            request.bearer_auth(key)
        } else {
            request
        }
    }
    pub async fn models(&self) -> Result<Value> {
        if !self.configured() {
            bail!("未配置DEEPSEEK_API_KEY")
        };
        let response = self
            .auth(self.client.get(format!("{}/models", self.base)))
            .send()
            .await?;
        let status = response.status();
        let value: Value = response.json().await?;
        if !status.is_success() {
            bail!("DeepSeek鉴权/模型列表失败 HTTP {}", status.as_u16())
        };
        Ok(value)
    }
    pub async fn image_part(
        &self,
        workspace: Arc<Mutex<Workspace>>,
        artifact: &str,
        upload: bool,
        cancel: &Cancellation,
    ) -> Result<Value> {
        use base64::Engine;
        crate::store::validate_id(artifact)?;
        let now = chrono::Utc::now().timestamp();
        if upload {
            if let Some((id, expires)) = self.files.lock().unwrap().get(artifact) {
                if *expires > now + 60 {
                    return Ok(json!({"type":"input_image","file_id":id}));
                }
            }
        }
        let bytes = {
            let w = workspace.lock().unwrap();
            std::fs::read(w.root.join("artifacts").join(artifact))?
        };
        if bytes.len() > 16 * 1024 * 1024 {
            bail!("截图超过16MiB本地保护值")
        }
        if !upload {
            return Ok(
                json!({"type":"input_image","image_url":format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes)),"detail":"high"}),
            );
        }
        let form = reqwest::multipart::Form::new()
            .text("purpose", "user_data")
            .text("expires_after[anchor]", "created_at")
            .text("expires_after[seconds]", "86400")
            .part(
                "file",
                reqwest::multipart::Part::bytes(bytes)
                    .file_name(artifact.to_owned())
                    .mime_str("image/png")?,
            );
        let request = self
            .auth(self.client.post(format!("{}/files", self.base)))
            .multipart(form);
        let response =
            tokio::select! {r=request.send()=>r?,_=cancel.wait()=>bail!("RUN_CANCELLED")};
        let status = response.status();
        let value: Value = response.json().await?;
        if !status.is_success() {
            bail!("DeepSeek图片上传失败 HTTP {}", status.as_u16())
        }
        let id = value["id"]
            .as_str()
            .context("上传响应没有file_id")?
            .to_owned();
        let expires = value["expires_at"].as_i64().unwrap_or(now + 86400);
        let mut files = self.files.lock().unwrap();
        files.insert(artifact.into(), (id.clone(), expires));
        crate::store::write_json(&self.file_path, &*files)?;
        drop(files);
        workspace.lock().unwrap().event(
            "provider.file_uploaded",
            None,
            json!({"artifact":artifact,"fileId":id,"expiresAt":expires}),
        )?;
        Ok(json!({"type":"input_image","file_id":id}))
    }
    pub async fn response(
        &self,
        payload: Value,
        workspace: Arc<Mutex<Workspace>>,
        run: &str,
        cancel: &Cancellation,
    ) -> Result<Value> {
        self.response_with_trace(
            payload,
            workspace,
            run,
            cancel,
            ProviderRequestTrace::default(),
        )
        .await
    }

    pub async fn response_with_trace(
        &self,
        payload: Value,
        workspace: Arc<Mutex<Workspace>>,
        run: &str,
        cancel: &Cancellation,
        trace: ProviderRequestTrace,
    ) -> Result<Value> {
        if !self.configured() {
            bail!("未配置DEEPSEEK_API_KEY；Windows请配置本机provider.env")
        };
        let start = std::time::Instant::now();
        let request_id = trace
            .request_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        workspace.lock().unwrap().event(
            "provider.request",
            Some(run),
            json!({"requestId":request_id,"time":now(),"body":payload,"trace":trace.event_fields()}),
        )?;
        for attempt in 0..4u32 {
            if cancel.stopped() {
                workspace.lock().unwrap().event(
                    "provider.cancelled",
                    Some(run),
                    json!({"requestId":request_id,"epoch":trace.epoch,"phase":"before_send"}),
                )?;
                bail!("RUN_CANCELLED")
            }
            let request = self
                .auth(self.client.post(format!("{}/responses", self.base)))
                .json(&payload);
            let response = tokio::select! {
                biased;
                r=request.send()=>r?,
                _=cancel.wait()=>{
                    workspace.lock().unwrap().event(
                        "provider.cancelled",
                        Some(run),
                        json!({"requestId":request_id,"epoch":trace.epoch,"phase":"sending"}),
                    )?;
                    bail!("RUN_CANCELLED")
                }
            };
            let status = response.status();
            if !status.is_success() {
                let value = response.text().await.unwrap_or_default();
                let message = serde_json::from_str::<Value>(&value)
                    .ok()
                    .and_then(|v| v["error"]["message"].as_str().map(str::to_owned))
                    .unwrap_or_else(|| format!("HTTP {}", status.as_u16()));
                let message = if let Some(key) = &self.key {
                    message.replace(key, "[REDACTED]")
                } else {
                    message
                };
                workspace.lock().unwrap().event("provider.error",Some(run),json!({"requestId":request_id,"status":status.as_u16(),"attempt":attempt,"message":message}))?;
                if [429, 500, 502, 503].contains(&status.as_u16()) && attempt < 3 {
                    tokio::select! {_=tokio::time::sleep(std::time::Duration::from_secs(1<<attempt))=>{},_=cancel.wait()=>bail!("RUN_CANCELLED")};
                    continue;
                }
                bail!("DeepSeek HTTP {}: {}", status.as_u16(), message)
            }
            if !payload["stream"].as_bool().unwrap_or(false) {
                let value: Value = response.json().await?;
                self.record_response(
                    &workspace,
                    run,
                    &request_id,
                    &value,
                    start.elapsed().as_secs_f64(),
                )?;
                if value["status"] == "incomplete" {
                    workspace.lock().unwrap().event(
                        "provider.incomplete",
                        Some(run),
                        json!({"requestId":request_id,"trace":trace.event_fields(),"response":value}),
                    )?;
                    return Ok(value);
                }
                if value["status"] != "completed" {
                    bail!("响应未完成：{}", value["status"])
                }
                return Ok(value);
            }
            let mut stream = response.bytes_stream();
            let mut buffer = Vec::<u8>::new();
            let mut stream_frames = Vec::<Value>::new();
            let mut first = true;
            let mut bytes_total = 0usize;
            loop {
                let chunk = tokio::select! {
                    biased;
                    r=stream.next()=>r,
                    _=cancel.wait()=>{
                        workspace.lock().unwrap().event(
                            "provider.cancelled",
                            Some(run),
                            json!({
                                "requestId":request_id,
                                "epoch":trace.epoch,
                                "phase":"streaming",
                                "receivedBytes":bytes_total,
                                "frames":stream_frames,
                            }),
                        )?;
                        bail!("RUN_CANCELLED")
                    }
                };
                let Some(chunk) = chunk else { break };
                let chunk = chunk?;
                bytes_total += chunk.len();
                if bytes_total > 16 * 1024 * 1024 {
                    bail!("Provider流超过本地保护值")
                };
                buffer.extend_from_slice(&chunk);
                if first {
                    first = false;
                    workspace.lock().unwrap().event("provider.first_byte",Some(run),json!({"requestId":request_id,"latencySeconds":start.elapsed().as_secs_f64()}))?;
                }
                while let Some((pos, skip)) = buffer
                    .windows(2)
                    .position(|w| w == b"\n\n")
                    .map(|p| (p, 2))
                    .or_else(|| {
                        buffer
                            .windows(4)
                            .position(|w| w == b"\r\n\r\n")
                            .map(|p| (p, 4))
                    })
                {
                    let frame = String::from_utf8(buffer[..pos].to_vec())?;
                    buffer.drain(..pos + skip);
                    let data = frame
                        .lines()
                        .filter_map(|l| l.strip_prefix("data:"))
                        .map(str::trim_start)
                        .collect::<Vec<_>>()
                        .join("\n");
                    if data.is_empty() {
                        continue;
                    }
                    let event: Value = serde_json::from_str(&data)?;
                    let kind = event["type"].as_str().unwrap_or("");
                    if kind == "response.output_text.delta" {
                        workspace.lock().unwrap().event(
                            "provider.text_delta",
                            Some(run),
                            json!({"requestId":request_id,"text":event["delta"]}),
                        )?;
                    }
                    if kind == "response.completed" {
                        let result = event["response"].clone();
                        self.record_response(
                            &workspace,
                            run,
                            &request_id,
                            &result,
                            start.elapsed().as_secs_f64(),
                        )?;
                        return Ok(result);
                    }
                    if ["response.failed", "response.incomplete", "error"].contains(&kind) {
                        workspace.lock().unwrap().event(
                            "provider.incomplete",
                            Some(run),
                            json!({
                                "requestId":request_id,
                                "trace":trace.event_fields(),
                                "frames":stream_frames,
                                "terminal":event,
                            }),
                        )?;
                        if kind == "response.incomplete" {
                            return Ok(event["response"].clone());
                        }
                        bail!("Provider响应未完成，未执行部分工具：{}", kind)
                    }
                    stream_frames.push(event);
                }
            }
            workspace.lock().unwrap().event(
                "provider.incomplete",
                Some(run),
                json!({
                    "requestId":request_id,
                    "trace":trace.event_fields(),
                    "reason":"stream_ended_without_response.completed",
                    "frames":stream_frames,
                }),
            )?;
            bail!("SSE_STREAM_INTERRUPTED：没有收到response.completed，未执行部分工具")
        }
        bail!("Provider重试耗尽")
    }
    fn record_response(
        &self,
        workspace: &Arc<Mutex<Workspace>>,
        run: &str,
        id: &str,
        value: &Value,
        seconds: f64,
    ) -> Result<()> {
        workspace.lock().unwrap().event(
            "provider.response",
            Some(run),
            json!({"requestId":id,"latencySeconds":seconds,"body":value}),
        )?;
        Ok(())
    }
}
