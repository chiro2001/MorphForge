mod compat;
mod config;
mod usage;
pub use config::{ApiType, Connection, ImageDelivery};

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
#[serde(default, rename_all = "camelCase")]
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
    pub preset: String,
    pub context_window_tokens: usize,
    pub send_images: bool,
    pub detail_instructions: String,
    pub goal_mode: bool,
    pub goal_min_review_rounds: usize,
    pub goal_max_continuations: u32,
    pub goal_criteria: String,
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
            preset: "standard".into(),
            context_window_tokens: 131072,
            send_images: true,
            detail_instructions: String::new(),
            goal_mode: false,
            goal_min_review_rounds: 1,
            goal_max_continuations: 6,
            goal_criteria: String::new(),
        }
    }
}
#[derive(Clone)]
pub struct Provider {
    client: reqwest::Client,
    connection: Arc<Mutex<Connection>>,
    root: PathBuf,
    redaction_keys: Arc<Mutex<Vec<String>>>,
    files: Arc<Mutex<BTreeMap<String, (String, i64)>>>,
    usage: Arc<Mutex<usage::UsageLedger>>,
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
        Self::build(root, Connection::load(root, env_file)?)
    }
    #[doc(hidden)]
    pub fn new_for_loopback_test(root: &Path, base: &str) -> Result<Self> {
        let url = reqwest::Url::parse(base)?;
        if url.scheme() != "http" || url.host_str() != Some("127.0.0.1") {
            bail!("测试Provider只允许HTTP loopback")
        }
        Self::build(
            root,
            Connection {
                base_url: base.into(),
                ..Connection::default()
            },
        )
    }
    fn build(root: &Path, mut connection: Connection) -> Result<Self> {
        connection.validate()?;
        let file_path = root.join("provider-files.json");
        let files = std::fs::read(&file_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let redaction_keys = connection.api_key.iter().cloned().collect();
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(240))
                .build()?,
            connection: Arc::new(Mutex::new(connection)),
            root: root.into(),
            redaction_keys: Arc::new(Mutex::new(redaction_keys)),
            files: Arc::new(Mutex::new(files)),
            usage: Arc::new(Mutex::new(usage::UsageLedger::load(root)?)),
            file_path,
        })
    }
    pub fn configure(&self, patch: &Value) -> Result<Value> {
        let mut current = self.connection.lock().unwrap();
        let next = current.patched(patch)?;
        next.save(&self.root)?;
        if let Some(key) = &next.api_key {
            let mut keys = self.redaction_keys.lock().unwrap();
            if !keys.contains(key) {
                keys.push(key.clone());
            }
        }
        *current = next;
        Ok(current.status())
    }
    pub fn usage(&self, run: Option<&str>) -> Value {
        self.usage.lock().unwrap().value(run)
    }
    fn record_event(
        &self,
        workspace: &Arc<Mutex<Workspace>>,
        kind: &str,
        run: Option<&str>,
        data: Value,
    ) -> Result<()> {
        let event = workspace.lock().unwrap().event(kind, run, data)?;
        if usage::UsageLedger::relevant(kind) {
            self.usage.lock().unwrap().observe(&event);
        }
        Ok(())
    }
    pub fn connection(&self) -> Connection {
        self.connection.lock().unwrap().clone()
    }
    pub fn redact(&self, text: &str) -> String {
        self.redaction_keys
            .lock()
            .unwrap()
            .iter()
            .fold(text.to_owned(), |text, key| text.replace(key, "[REDACTED]"))
    }
    pub fn configured(&self) -> bool {
        self.connection.lock().unwrap().configured()
    }
    pub fn status(&self) -> Value {
        self.connection.lock().unwrap().status()
    }
    fn auth(connection: &Connection, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(key) = &connection.api_key {
            request.bearer_auth(key)
        } else {
            request
        }
    }
    fn log_value(&self, value: &Value) -> Value {
        match value {
            Value::String(s) if s.starts_with("data:image/") => {
                json!("[INLINE_IMAGE: local artifact retained]")
            }
            Value::String(s) => json!(self.redact(s)),
            Value::Array(values) => {
                Value::Array(values.iter().map(|v| self.log_value(v)).collect())
            }
            Value::Object(values) => Value::Object(
                values
                    .iter()
                    .map(|(k, v)| (k.clone(), self.log_value(v)))
                    .collect(),
            ),
            _ => value.clone(),
        }
    }
    pub async fn models(&self) -> Result<Value> {
        if !self.configured() {
            bail!("未配置Provider API Key")
        };
        let connection = self.connection();
        let response = Self::auth(
            &connection,
            self.client.get(format!("{}/models", connection.base_url)),
        )
        .send()
        .await?;
        let status = response.status();
        let value: Value = response.json().await?;
        if !status.is_success() {
            bail!("Provider鉴权/模型列表失败 HTTP {}", status.as_u16())
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
        let connection = self.connection();
        self.image_part_for(workspace, artifact, upload, cancel, &connection)
            .await
    }
    async fn image_part_for(
        &self,
        workspace: Arc<Mutex<Workspace>>,
        artifact: &str,
        upload: bool,
        cancel: &Cancellation,
        connection: &Connection,
    ) -> Result<Value> {
        use base64::Engine;
        crate::store::validate_id(artifact)?;
        let upload = upload
            && connection.supports_file_upload()
            && connection.image_delivery == ImageDelivery::Files;
        let cache_key = format!("{}:{artifact}", connection.identity());
        let now = chrono::Utc::now().timestamp();
        if upload {
            if let Some((id, expires)) = self.files.lock().unwrap().get(&cache_key) {
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
        let request = Self::auth(
            connection,
            self.client.post(format!("{}/files", connection.base_url)),
        )
        .multipart(form);
        let response =
            tokio::select! {r=request.send()=>r?,_=cancel.wait()=>bail!("RUN_CANCELLED")};
        let status = response.status();
        let value: Value = response.json().await?;
        if !status.is_success() {
            bail!("Provider图片上传失败 HTTP {}", status.as_u16())
        }
        let id = value["id"]
            .as_str()
            .context("上传响应没有file_id")?
            .to_owned();
        let expires = value["expires_at"].as_i64().unwrap_or(now + 86400);
        let mut files = self.files.lock().unwrap();
        files.insert(cache_key, (id.clone(), expires));
        crate::store::write_json(&self.file_path, &*files)?;
        drop(files);
        self.record_event(
            &workspace,
            "provider.file_uploaded",
            None,
            json!({"artifact":artifact,"fileId":id,"expiresAt":expires}),
        )?;
        Ok(json!({"type":"input_image","file_id":id}))
    }
    fn resolve_images<'a>(
        &'a self,
        value: &'a mut Value,
        workspace: Arc<Mutex<Workspace>>,
        cancel: &'a Cancellation,
        connection: &'a Connection,
    ) -> futures_util::future::BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            if value["type"] == "input_image" {
                let artifact = value["artifact"].as_str().map(str::to_owned).or_else(|| {
                    let id = value["file_id"].as_str()?;
                    self.files
                        .lock()
                        .unwrap()
                        .iter()
                        .find(|(_, (file, _))| file == id)
                        .map(|(key, _)| key.rsplit(':').next().unwrap_or(key).to_owned())
                });
                if let Some(artifact) = artifact {
                    *value = self
                        .image_part_for(workspace, &artifact, true, cancel, connection)
                        .await?;
                } else if value["file_id"].is_string() {
                    bail!("图片文件引用没有本地映射；请重新截图或重置上下文后继续")
                }
                return Ok(());
            }
            match value {
                Value::Array(values) => {
                    for item in values {
                        self.resolve_images(item, workspace.clone(), cancel, connection)
                            .await?;
                    }
                }
                Value::Object(values) => {
                    for item in values.values_mut() {
                        self.resolve_images(item, workspace.clone(), cancel, connection)
                            .await?;
                    }
                }
                _ => {}
            }
            Ok(())
        })
    }
    fn tag_reasoning(&self, mut value: Value, identity: &str) -> Value {
        for item in value["output"].as_array_mut().into_iter().flatten() {
            if item["type"] == "reasoning" || item["chat_reasoning_content"].is_string() {
                item["_mf_provider"] = json!(identity);
            }
        }
        value
    }
    fn portable_responses(&self, payload: &mut Value, connection: &Connection) {
        let identity = connection.identity();
        let legacy = reqwest::Url::parse(&connection.base_url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_owned))
            .as_deref()
            == Some("api.deepseek.com");
        if let Some(input) = payload["input"].as_array_mut() {
            input.retain(|item| {
                item["type"] != "reasoning"
                    || item["_mf_provider"]
                        .as_str()
                        .map(|id| id == identity)
                        .unwrap_or(legacy)
            });
            input.retain(|item| {
                !(item["type"] == "message"
                    && item["content"].as_array().is_some_and(Vec::is_empty))
            });
            for item in input {
                if let Some(object) = item.as_object_mut() {
                    object.remove("_mf_provider");
                    object.remove("chat_reasoning_content");
                    if object.get("type").and_then(Value::as_str) != Some("reasoning") {
                        object.remove("id");
                        object.remove("status");
                        object.remove("phase");
                    }
                }
            }
        }
        if !connection.send_reasoning {
            payload.as_object_mut().unwrap().remove("reasoning");
        }
        payload["stream"] = json!(connection.stream);
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
            bail!("未配置Provider API Key；Windows请配置本机provider.env")
        };
        let connection = self.connection();
        let mut payload = payload;
        self.resolve_images(&mut payload, workspace.clone(), cancel, &connection)
            .await?;
        let payload = match connection.api_type {
            ApiType::Chat => compat::chat_request(&payload, &connection)?,
            ApiType::Responses => {
                self.portable_responses(&mut payload, &connection);
                payload
            }
        };
        let endpoint = match connection.api_type {
            ApiType::Chat => "chat/completions",
            ApiType::Responses => "responses",
        };
        let identity = connection.identity();
        let start = std::time::Instant::now();
        let request_id = trace
            .request_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        self.record_event(&workspace,
            "provider.request",
            Some(run),
            json!({"requestId":request_id,"time":now(),"apiType":connection.api_type,"baseUrl":connection.base_url,"body":self.log_value(&payload),"trace":trace.event_fields()}),
        )?;
        for attempt in 0..4u32 {
            if cancel.stopped() {
                self.record_event(
                    &workspace,
                    "provider.cancelled",
                    Some(run),
                    json!({"requestId":request_id,"epoch":trace.epoch,"phase":"before_send"}),
                )?;
                bail!("RUN_CANCELLED")
            }
            let request = Self::auth(
                &connection,
                self.client
                    .post(format!("{}/{endpoint}", connection.base_url)),
            )
            .json(&payload);
            let response = tokio::select! {
                biased;
                r=request.send()=>r?,
                _=cancel.wait()=>{
                    self.record_event(&workspace,
                        "provider.cancelled",
                        Some(run),
                        json!({"requestId":request_id,"epoch":trace.epoch,"phase":"sending"}),
                    )?;
                    bail!("RUN_CANCELLED")
                }
            };
            let status = response.status();
            if !status.is_success() {
                let value =
                    String::from_utf8_lossy(&bounded_body(response, cancel).await?).into_owned();
                let message = serde_json::from_str::<Value>(&value)
                    .ok()
                    .and_then(|v| v["error"]["message"].as_str().map(str::to_owned))
                    .unwrap_or_else(|| format!("HTTP {}", status.as_u16()));
                let message = self.redact(&message);
                self.record_event(&workspace,"provider.error",Some(run),json!({"requestId":request_id,"status":status.as_u16(),"attempt":attempt,"message":message}))?;
                if [429, 500, 502, 503].contains(&status.as_u16()) && attempt < 3 {
                    tokio::select! {_=tokio::time::sleep(std::time::Duration::from_secs(1<<attempt))=>{},_=cancel.wait()=>bail!("RUN_CANCELLED")};
                    continue;
                }
                bail!("Provider HTTP {}: {}", status.as_u16(), message)
            }
            let is_json = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|h| h.to_str().ok())
                .is_some_and(|h| h.contains("application/json"));
            if !payload["stream"].as_bool().unwrap_or(false) || is_json {
                let value: Value = serde_json::from_slice(&bounded_body(response, cancel).await?)?;
                let value = if connection.api_type == ApiType::Chat {
                    compat::chat_response(&value)?
                } else {
                    value
                };
                let value = self.log_value(&self.tag_reasoning(value, &identity));
                self.record_response(
                    &workspace,
                    run,
                    &request_id,
                    &value,
                    start.elapsed().as_secs_f64(),
                )?;
                if value["status"] == "incomplete" {
                    self.record_event(&workspace,
                        "provider.incomplete",
                        Some(run),
                        json!({"requestId":request_id,"trace":trace.event_fields(),"response":self.log_value(&value)}),
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
            let mut chat = compat::ChatStream::default();
            let mut bytes_total = 0usize;
            loop {
                let chunk = tokio::select! {
                    biased;
                    r=stream.next()=>r,
                    _=cancel.wait()=>{
                        self.record_event(&workspace,
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
                    self.record_event(&workspace,"provider.first_byte",Some(run),json!({"requestId":request_id,"latencySeconds":start.elapsed().as_secs_f64()}))?;
                }
                while let Some((pos, skip)) = frame_boundary(&buffer) {
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
                    if connection.api_type == ApiType::Chat && data.trim() == "[DONE]" {
                        let result = self.log_value(&self.tag_reasoning(chat.finish()?, &identity));
                        self.record_response(
                            &workspace,
                            run,
                            &request_id,
                            &result,
                            start.elapsed().as_secs_f64(),
                        )?;
                        return Ok(result);
                    }
                    if data.trim() == "[DONE]" {
                        continue;
                    }
                    let event: Value = serde_json::from_str(&data)?;
                    if connection.api_type == ApiType::Chat {
                        if let Some(delta) = chat
                            .push(&event)
                            .map_err(|e| anyhow::anyhow!(self.redact(&e.to_string())))?
                        {
                            self.record_event(
                                &workspace,
                                "provider.text_delta",
                                Some(run),
                                json!({"requestId":request_id,"text":self.redact(&delta)}),
                            )?;
                        }
                        stream_frames.push(self.log_value(&event));
                        continue;
                    }
                    let kind = event["type"].as_str().unwrap_or("");
                    if kind == "response.output_text.delta" {
                        self.record_event(
                            &workspace,
                            "provider.text_delta",
                            Some(run),
                            json!({"requestId":request_id,"text":self.log_value(&event["delta"])}),
                        )?;
                    }
                    if kind == "response.completed" {
                        let result = self
                            .log_value(&self.tag_reasoning(event["response"].clone(), &identity));
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
                        self.record_event(
                            &workspace,
                            "provider.incomplete",
                            Some(run),
                            json!({
                                "requestId":request_id,
                                "trace":trace.event_fields(),
                                "frames":stream_frames,
                                "terminal":self.log_value(&event),
                            }),
                        )?;
                        if kind == "response.incomplete" {
                            return Ok(self.log_value(&event["response"]));
                        }
                        bail!("Provider响应未完成，未执行部分工具：{}", kind)
                    }
                    stream_frames.push(self.log_value(&event));
                }
            }
            self.record_event(
                &workspace,
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
        self.record_event(
            workspace,
            "provider.response",
            Some(run),
            json!({"requestId":id,"latencySeconds":seconds,"body":self.log_value(value)}),
        )?;
        Ok(())
    }
}

fn frame_boundary(buffer: &[u8]) -> Option<(usize, usize)> {
    let lf = buffer.windows(2).position(|w| w == b"\n\n").map(|p| (p, 2));
    let crlf = buffer
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|p| (p, 4));
    [lf, crlf].into_iter().flatten().min_by_key(|(pos, _)| *pos)
}

async fn bounded_body(response: reqwest::Response, cancel: &Cancellation) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    loop {
        let chunk = tokio::select! { result=stream.next()=>result, _=cancel.wait()=>bail!("RUN_CANCELLED") };
        let Some(chunk) = chunk else {
            break;
        };
        let chunk = chunk?;
        if bytes.len().saturating_add(chunk.len()) > 16 * 1024 * 1024 {
            bail!("Provider响应超过16MiB本地保护值")
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mixed_sse_delimiters_choose_the_first_frame() {
        assert_eq!(
            frame_boundary(b"data: first\r\n\r\ndata: second\n\n"),
            Some((11, 4))
        );
        assert_eq!(
            frame_boundary(b"data: first\n\ndata: second\r\n\r\n"),
            Some((11, 2))
        );
        assert_eq!(frame_boundary(b"data: partial\r\n"), None);
    }
}
