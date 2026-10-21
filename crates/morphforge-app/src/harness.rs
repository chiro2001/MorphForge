use crate::{
    capture::CaptureBroker,
    fonts::FontCatalog,
    provider::{Cancellation, Provider, ProviderRequestTrace, ProviderSettings},
    recipe::{self, RecipeRequest},
    references::{self, ReferenceImage},
    store::{now, write_json, Artwork, Workspace},
    transport::Transport,
};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};
mod context;
mod context_usage;
pub use context_usage::ContextUsageAnchor;
mod goal;
pub(crate) mod lifecycle;
mod palette;
mod prompts;
mod runtime_materials;
mod visual_context;
pub use goal::GoalProgress;
pub const SYSTEM_PROMPT_VERSION: &str = "morphforge-system/5.2";
pub const TOOL_SCHEMA_VERSION: &str = "morphforge-tools/4.3";

#[derive(Clone, Debug)]
struct PendingPrompt {
    request_id: String,
    run_id: String,
    prompt: String,
    mode: String,
    accepted_at: String,
    accepted_epoch: u64,
    references: Vec<ReferenceImage>,
}

#[derive(Clone)]
struct MessageReceipt {
    ack: Value,
    prompt: String,
    mode: String,
    requested_run_id: Option<String>,
    reference_ids: Vec<String>,
}

#[derive(Clone)]
struct RunGuard {
    id: String,
    epoch: u64,
    deadline: tokio::time::Instant,
    cancel: Cancellation,
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub project_id: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub schema_tools: String,
    #[serde(default)]
    pub schema_prompt: String,
    pub input: Vec<Value>,
    pub goals: Vec<String>,
    pub summary: String,
    pub boundaries: Vec<usize>,
    pub compact_count: u32,
    #[serde(default)]
    pub goal: Option<GoalProgress>,
    #[serde(default)]
    pub reference_images: Vec<ReferenceImage>,
    #[serde(default)]
    pub reference_snapshot: Option<String>,
    #[serde(default)]
    pub runtime_context: Value,
    #[serde(default)]
    pub requested_input_length: usize,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "context_usage::decode_anchor"
    )]
    pub context_usage: Option<ContextUsageAnchor>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: String,
    #[serde(default)]
    pub epoch: u64,
    pub status: String,
    pub prompt: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub steps: u32,
    pub tokens: u64,
    #[serde(default)]
    pub active_request_id: Option<String>,
    #[serde(default)]
    pub prompt_version: String,
    #[serde(default)]
    pub tools_version: String,
    #[serde(default)]
    pub language: String,
    pub message: String,
    pub error: String,
}
pub struct State {
    pub settings: ProviderSettings,
    pub conversation: Conversation,
    pub run: Option<Run>,
    pub run_deadline: Option<tokio::time::Instant>,
    pub cancel: Cancellation,
    pub paused: bool,
    pub compact_requested: bool,
    pub phase: String,
    pub epoch: u64,
    active_request_id: Option<String>,
    pub worker_epoch: Option<u64>,
    active_budget_remaining: Option<Duration>,
    queue_turn_complete: bool,
    pending_steers: VecDeque<PendingPrompt>,
    pending_interrupts: VecDeque<PendingPrompt>,
    queued: VecDeque<PendingPrompt>,
    message_acks: HashMap<String, MessageReceipt>,
}
#[derive(Clone)]
pub struct Harness {
    pub inner: Arc<Mutex<State>>,
    pub workspace: Arc<Mutex<Workspace>>,
    pub provider: Provider,
    pub capture: CaptureBroker,
    pub transport: Transport,
    fonts: Option<Arc<FontCatalog>>,
}
impl Harness {
    pub fn new(
        workspace: Arc<Mutex<Workspace>>,
        provider: Provider,
        capture: CaptureBroker,
        transport: Transport,
    ) -> Self {
        Self::build(
            workspace,
            provider,
            capture,
            transport,
            crate::fonts::shared_default().ok(),
        )
    }

    pub fn new_with_fonts(
        workspace: Arc<Mutex<Workspace>>,
        provider: Provider,
        capture: CaptureBroker,
        transport: Transport,
        fonts: Arc<FontCatalog>,
    ) -> Self {
        Self::build(workspace, provider, capture, transport, Some(fonts))
    }

    fn build(
        workspace: Arc<Mutex<Workspace>>,
        provider: Provider,
        capture: CaptureBroker,
        transport: Transport,
        fonts: Option<Arc<FontCatalog>>,
    ) -> Self {
        let (root, events) = {
            let workspace = workspace.lock().unwrap();
            (workspace.root.clone(), workspace.events.clone())
        };
        let mut conversation: Conversation = std::fs::read(root.join("runs/conversation.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        {
            let mut w = workspace.lock().unwrap();
            if conversation.id.is_empty() {
                conversation.id = w.session_id.clone();
            }
            if conversation.project_id.is_empty() {
                conversation.project_id = w.project.id.clone();
            }
            if conversation.created_at.is_empty() {
                conversation.created_at = now();
            }
            w.session_id = conversation.id.clone();
        }
        Self::hydrate_versions(&mut conversation, &events);
        let scoped_events: Vec<_> = events
            .into_iter()
            .filter(|e| {
                e.session_id
                    .as_deref()
                    .is_none_or(|id| id == conversation.id)
            })
            .collect();
        let (settings, upgraded) = std::fs::read(root.join("harness.json"))
            .ok()
            .and_then(|b| prompts::saved_settings(&b).ok())
            .unwrap_or_else(|| (ProviderSettings::default(), false));
        if upgraded {
            let _ = write_json(&root.join("harness.json"), &settings);
            let _=workspace.lock().unwrap().event("harness.defaults.upgraded",None,json!({
                "contextWindowTokens":settings.context_window_tokens,"maxOutputTokens":settings.max_output_tokens,
                "note":"仅升级仍匹配旧预设默认值的字段；自定义数值保留。"
            }));
        }
        let (message_acks, restored_queue) = restore_message_state(&scoped_events);
        Self {
            inner: Arc::new(Mutex::new(State {
                settings,
                conversation,
                run: None,
                run_deadline: None,
                cancel: Cancellation::new(),
                paused: false,
                compact_requested: false,
                phase: "idle".into(),
                epoch: 0,
                active_request_id: None,
                worker_epoch: None,
                active_budget_remaining: None,
                queue_turn_complete: true,
                pending_steers: VecDeque::new(),
                pending_interrupts: VecDeque::new(),
                queued: restored_queue,
                message_acks,
            })),
            workspace,
            provider,
            capture,
            transport,
            fonts,
        }
    }
    pub(super) fn hydrate_versions(c: &mut Conversation, events: &[crate::store::Event]) {
        let previous = events
            .iter()
            .rev()
            .filter(|e| e.session_id.as_deref().is_none_or(|id| id == c.id))
            .find_map(|e| {
                let value = if e.kind == "provider.request" {
                    &e.data["trace"]
                } else {
                    &e.data
                };
                Some((
                    value["toolsVersion"].as_str()?.to_owned(),
                    value["promptVersion"]
                        .as_str()
                        .unwrap_or(SYSTEM_PROMPT_VERSION)
                        .to_owned(),
                ))
            });
        if c.schema_tools.is_empty() {
            c.schema_tools = previous.as_ref().map(|p| p.0.clone()).unwrap_or_else(|| {
                if c.input.is_empty() {
                    TOOL_SCHEMA_VERSION.into()
                } else {
                    "legacy-unknown".into()
                }
            });
        }
        if c.schema_prompt.is_empty() {
            c.schema_prompt = previous.as_ref().map(|p| p.1.clone()).unwrap_or_else(|| {
                if c.input.is_empty() {
                    SYSTEM_PROMPT_VERSION.into()
                } else {
                    "legacy-unknown".into()
                }
            });
        }
        if c.input.is_empty() {
            c.schema_tools = TOOL_SCHEMA_VERSION.into();
            c.schema_prompt = SYSTEM_PROMPT_VERSION.into();
        }
    }
    fn schema_upgrade_required(c: &Conversation) -> bool {
        !c.input.is_empty()
            && ((!c.schema_tools.is_empty() && c.schema_tools != TOOL_SCHEMA_VERSION)
                || (!c.schema_prompt.is_empty() && c.schema_prompt != SYSTEM_PROMPT_VERSION))
    }
    pub fn state(&self) -> Value {
        let s = self.inner.lock().unwrap();
        let mut queued = s
            .queued
            .iter()
            .chain(s.pending_steers.iter())
            .chain(s.pending_interrupts.iter())
            .map(|message| {
                json!({
                    "requestId":message.request_id,
                    "runId":message.run_id,
                    "mode":message.mode,
                    "acceptedAt":message.accepted_at,
                    "epoch":message.accepted_epoch,
                })
            })
            .collect::<Vec<_>>();
        queued.sort_by(|a, b| a["acceptedAt"].as_str().cmp(&b["acceptedAt"].as_str()));
        let queued_prompts = s
            .queued
            .iter()
            .chain(s.pending_steers.iter())
            .chain(s.pending_interrupts.iter())
            .map(|message| message.prompt.clone())
            .collect::<Vec<_>>();
        let active_seconds_remaining = if s.paused {
            s.active_budget_remaining
        } else {
            s.run_deadline
                .map(|deadline| deadline.saturating_duration_since(tokio::time::Instant::now()))
        }
        .map(|remaining| remaining.as_secs_f64());
        let model_limits = self.provider.limits_status(&s.settings);
        let usage_meter = context_usage::meter(
            &s.conversation,
            &s.settings,
            &context_usage::provider_scope(&self.provider.connection()),
            model_limits["effectiveContextWindow"]
                .as_u64()
                .unwrap_or(s.settings.context_window_tokens as u64) as usize,
        );
        json!({
            "settings":s.settings,
            "provider":self.provider.status(),
            "modelLimits":model_limits,
            "usage":self.provider.usage(s.run.as_ref().map(|run|run.id.as_str())),
            "run":s.run,
            "paused":s.paused,
            "manualEditLocked":(is_active_phase(&s.phase) && s.phase != "paused") || s.worker_epoch.is_some(),
            "cacheMonitor":self.provider.cache_status(),
            "queued":queued_prompts,
            "interaction":{
                "phase":s.phase,
                "runId":s.run.as_ref().map(|run|run.id.as_str()),
                "epoch":s.run.as_ref().map(|run|run.epoch).unwrap_or(s.epoch),
                "activeRequestId":s.active_request_id,
                "paused":s.paused,
                "activeSecondsRemaining":active_seconds_remaining,
                "queue":queued,
            },
            "context":{
                "id":s.conversation.id,"title":s.conversation.title,"createdAt":s.conversation.created_at,"schemaUpgradeRequired":Self::schema_upgrade_required(&s.conversation),"schemaTools":s.conversation.schema_tools,"currentTools":TOOL_SCHEMA_VERSION,
                "estimatedTokens":usage_meter["historyTokensEstimate"],
                "usage":usage_meter,
                "messages":s.conversation.input.len(),
                "compactCount":s.conversation.compact_count,
                "summary":s.conversation.summary,
                "goals":s.conversation.goals,
                "goal":s.conversation.goal,
                "referenceImages":s.conversation.reference_images
            }
        })
    }
    pub fn ensure_manual_edit(&self) -> Result<()> {
        let state=self.inner.lock().unwrap();
        if (is_active_phase(&state.phase) && state.phase != "paused") || state.worker_epoch.is_some() {
            bail!("RUN_CONFLICT：AI正在修改，请暂停并等待当前执行结束后再手工编辑")
        }
        Ok(())
    }
    pub fn redact_display(&self, value: Value) -> Value {
        serde_json::from_str(&self.provider.redact(&value.to_string())).unwrap_or(Value::Null)
    }
    pub fn configure(&self, v: &Value) -> Result<Value> {
        let mut s = self.inner.lock().unwrap();
        let mut settings = serde_json::to_value(&s.settings)?;
        if let Some(name) = v["preset"].as_str() {
            let patch = prompts::preset_patch(name).context("未知创作预设")?;
            for (key, value) in patch.as_object().unwrap() {
                settings[key] = value.clone();
            }
        }
        for (k, v) in v.as_object().context("设置须为对象")? {
            if settings.get(k).is_none() {
                bail!("未知Harness设置 {k}")
            }
            settings[k] = v.clone();
        }
        let mut next: ProviderSettings = serde_json::from_value(settings)?;
        next.model = next.model.trim().into();
        if next.model.trim().is_empty()
            || next.model.len() > 256
            || next.model.chars().any(char::is_control)
            || !["none", "low", "high", "max"].contains(&next.reasoning.as_str())
            || !["auto", "strict", "observe"].contains(&next.cache_guard.as_str())
            || next.max_steps == 0
            || next.max_steps > 4096
            || next.max_output_tokens < 512
            || next.max_output_tokens > 4_194_304
            || next.compact_tokens < 1000
            || next.context_window_tokens < 8192
            || next.context_window_tokens > 4_194_304
            || next.compact_tokens > 4_194_304
            || next.detail_instructions.len() > 32000
            || next.goal_criteria.len() > 32000
            || next.goal_min_review_rounds == 0
            || next.goal_min_review_rounds > 16
            || next.goal_max_continuations == 0
            || next.goal_max_continuations > 128
            || prompts::preset_patch(&next.preset).is_none()
            || next.max_run_tokens < 1000
            || next.max_run_seconds == 0
            || next.max_run_seconds > 86400
            || !["zh-CN", "en"].contains(&next.language.as_str())
        {
            bail!("Harness预算配置越界")
        };
        if s.settings.model != next.model
            || s.settings.reasoning != next.reasoning
            || s.settings.language != next.language
            || s.settings.preset != next.preset
            || s.settings.detail_instructions != next.detail_instructions
            || s.settings.send_images != next.send_images
        {
            self.provider
                .authorize_prefix_reset("harness_configuration");
        }
        s.settings = next;
        let settings = s.settings.clone();
        drop(s);
        write_json(
            &self.workspace.lock().unwrap().root.join("harness.json"),
            &settings,
        )?;
        Ok(self.state())
    }
    fn persist(&self) -> Result<()> {
        let conversation = self.inner.lock().unwrap().conversation.clone();
        let w = self.workspace.lock().unwrap();
        lifecycle::save_conversation(&w, &conversation)?;
        Ok(())
    }
    pub fn start(&self, prompt: &str) -> Result<Value> {
        if prompt.trim().is_empty() || prompt.len() > 32000 {
            bail!("输入为空或超过32k字符")
        }
        self.message(&json!({"prompt":prompt,"mode":"queue"}))?;
        Ok(self.state())
    }

    /// Accept a durable user message into the active run's step or turn inbox.
    pub fn message(&self, value: &Value) -> Result<Value> {
        let mode = value
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("steer")
            .to_owned();
        if !["steer", "interrupt", "queue"].contains(&mode.as_str()) {
            bail!("未知消息模式；只支持steer/interrupt/queue")
        }
        let reference_ids = references::ids(value)?;
        let mut prompt = value
            .get("prompt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        if prompt.trim().is_empty() && !reference_ids.is_empty() {
            prompt = "请根据所选参考图片进行体素建模。".into();
        }
        if prompt.len() > 32000 || (prompt.trim().is_empty() && mode != "interrupt") {
            bail!("消息为空或超过32k字符")
        }
        let request_id = value
            .get("requestId")
            .and_then(Value::as_str)
            .filter(|id| !id.trim().is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        if request_id.len() > 128 {
            bail!("REQUEST_ID_CONFLICT：requestId超过128字符")
        }
        if !prompt.trim().is_empty()
            && Self::schema_upgrade_required(&self.inner.lock().unwrap().conversation)
        {
            bail!("HARNESS_UPGRADE_REQUIRED：工具/提示词已升级。可新开对话，或显式升级旧对话并重新预热缓存；未发送模型请求。");
        }
        if !prompt.trim().is_empty() && !self.provider.configured() {
            bail!("请先配置API Key")
        }

        let supplied_run_id = value
            .get("runId")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let mut spawn: Option<(String, u64)> = None;
        let mut was_started = false;
        let ack;
        let run_id;
        {
            let mut state = self.inner.lock().unwrap();
            if let Some(receipt) = state.message_acks.get(&request_id) {
                let same_run = match (&receipt.requested_run_id, &supplied_run_id) {
                    (Some(old), Some(new)) => old == new,
                    (Some(_), None) => false,
                    (None, None) => true,
                    (None, Some(new)) => receipt.ack["runId"].as_str() == Some(new),
                };
                if receipt.prompt != prompt
                    || receipt.mode != mode
                    || !same_run
                    || receipt.reference_ids != reference_ids
                {
                    bail!("REQUEST_ID_CONFLICT：requestId已用于不同消息")
                }
                return Ok(receipt.ack.clone());
            }

            if !reference_ids.is_empty() && !state.settings.send_images {
                bail!("当前已关闭向模型发送图片，请在配置页开启或取消参考图片");
            }
            let references =
                references::resolve(&mut self.workspace.lock().unwrap(), &reference_ids)?;
            let existing_run_id = state.run.as_ref().map(|run| run.id.clone());
            if let Some(requested) = supplied_run_id.as_deref() {
                if existing_run_id.as_deref() != Some(requested) || !is_active_phase(&state.phase) {
                    bail!("RUN_CONFLICT：runId已过期或当前没有活动run")
                }
            }

            if is_active_phase(&state.phase) {
                run_id = existing_run_id
                    .clone()
                    .context("RUN_CONFLICT：没有活动run")?;
                if mode == "interrupt" && prompt.trim().is_empty() {
                    self.pause_locked(&mut state)?;
                } else {
                    if mode == "interrupt" {
                        state.cancel.stop();
                        state.epoch = state.epoch.saturating_add(1);
                        let epoch = state.epoch;
                        state.phase = "interrupting".into();
                        state.paused = false;
                        if let Some(run) = state.run.as_mut() {
                            run.epoch = epoch;
                            run.status = "interrupting".into();
                        }
                    }
                    let epoch = state
                        .run
                        .as_ref()
                        .map(|run| run.epoch)
                        .unwrap_or(state.epoch);
                    let pending = PendingPrompt {
                        request_id: request_id.clone(),
                        run_id: run_id.clone(),
                        prompt: prompt.clone(),
                        mode: mode.clone(),
                        accepted_at: now(),
                        accepted_epoch: epoch,
                        references: references.clone(),
                    };
                    match mode.as_str() {
                        "steer" => state.pending_steers.push_back(pending),
                        "interrupt" => state.pending_interrupts.push_back(pending),
                        _ => state.queued.push_back(pending),
                    }
                    if mode == "interrupt" && state.worker_epoch.is_none() {
                        let new_epoch = state.run.as_ref().map(|run| run.epoch).unwrap_or(epoch);
                        self.prepare_worker_locked(&mut state, new_epoch)?;
                        spawn = Some((run_id.clone(), new_epoch));
                    }
                }
            } else {
                if supplied_run_id.is_some() {
                    bail!("RUN_CONFLICT：runId已过期")
                }
                if prompt.trim().is_empty() {
                    bail!("RUN_CONFLICT：没有可暂停的活动run")
                }
                if state.run.as_ref().is_some_and(|run| run.status == "paused") {
                    bail!("RUN_CONFLICT：run已暂停，请先resume或停止")
                }
                let (id, epoch) = self.begin_run_locked(&mut state, &prompt, "running");
                run_id = id;
                was_started = true;
                let pending = PendingPrompt {
                    request_id: request_id.clone(),
                    run_id: run_id.clone(),
                    prompt: prompt.clone(),
                    mode: mode.clone(),
                    accepted_at: now(),
                    accepted_epoch: epoch,
                    references: references.clone(),
                };
                match mode.as_str() {
                    "steer" => state.pending_steers.push_back(pending),
                    "interrupt" => state.pending_interrupts.push_back(pending),
                    _ => state.queued.push_back(pending),
                }
                spawn = Some((run_id.clone(), epoch));
            }

            let epoch = state
                .run
                .as_ref()
                .map(|run| run.epoch)
                .unwrap_or(state.epoch);
            let phase = state.phase.clone();
            let queue_position = match mode.as_str() {
                "steer" => state.pending_steers.len(),
                "interrupt" if !prompt.trim().is_empty() => state.pending_interrupts.len(),
                "queue" => state.queued.len(),
                _ => 0,
            };
            ack = json!({
                "ok":true,
                "accepted":true,
                "requestId":request_id,
                "runId":run_id,
                "mode":mode,
                "epoch":epoch,
                "phase":phase,
                "queuePosition":queue_position,
                "referenceCount":references.len(),
            });
            state.message_acks.insert(
                request_id.clone(),
                MessageReceipt {
                    ack: ack.clone(),
                    prompt: prompt.clone(),
                    mode: mode.clone(),
                    requested_run_id: supplied_run_id.clone(),
                    reference_ids: reference_ids.clone(),
                },
            );
            let mut workspace = self.workspace.lock().unwrap();
            if was_started {
                state.run.as_ref().context("run创建失败")?;
                workspace.event(
                    "harness.started",
                    Some(&run_id),
                    json!({
                        "prompt":prompt,
                        "requestId":request_id,
                        "epoch":epoch,
                        "promptVersion":SYSTEM_PROMPT_VERSION,
                        "toolsVersion":TOOL_SCHEMA_VERSION,
                        "language":state.settings.language,
                    }),
                )?;
            }
            workspace.event(
                "harness.message.accepted",
                Some(&run_id),
                json!({
                    "requestId":request_id,
                    "requestedRunId":supplied_run_id,
                    "runId":run_id,
                    "mode":mode,
                    "prompt":prompt,
                    "references":references,
                    "referenceIds":reference_ids,
                    "epoch":epoch,
                    "phase":phase,
                    "queuePosition":queue_position,
                    "ack":ack,
                }),
            )?;
        }
        if let Some((id, epoch)) = spawn {
            self.spawn_worker(id, epoch);
        }
        Ok(ack)
    }

    pub fn control(&self, action: &str) -> Result<Value> {
        if ["resume", "compact"].contains(&action)
            && Self::schema_upgrade_required(&self.inner.lock().unwrap().conversation)
        {
            bail!("HARNESS_UPGRADE_REQUIRED：请先升级旧对话或新开对话；未发送模型请求。");
        }
        match action {
            "upgrade_context" => {
                let mut state = self.inner.lock().unwrap();
                if is_active_phase(&state.phase)
                    || state.worker_epoch.is_some()
                    || !state.queued.is_empty()
                {
                    bail!("RUN_CONFLICT：请先停止AI运行和排队消息");
                }
                if !self.provider.cache_status()["alert"].is_null() {
                    bail!("CACHE_GUARD_PAUSED：已有缓存告警未确认，升级不会绕过告警。请先检查并确认告警。");
                }
                state.conversation.schema_tools = TOOL_SCHEMA_VERSION.into();
                state.conversation.schema_prompt = SYSTEM_PROMPT_VERSION.into();
                self.provider
                    .authorize_prefix_reset("explicit_schema_upgrade");
            }
            "stop" => self.stop_run()?,
            "pause" | "interrupt" => {
                self.message(&json!({"mode":"interrupt","prompt":""}))?;
            }
            "resume" => self.resume_run()?,
            "compact" => {
                let spawn = {
                    let mut state = self.inner.lock().unwrap();
                    if is_active_phase(&state.phase) {
                        state.compact_requested = true;
                        None
                    } else {
                        let (id, epoch) =
                            self.begin_run_locked(&mut state, "手动compact", "compacting");
                        state.compact_requested = true;
                        Some((id, epoch))
                    }
                };
                if let Some((id, epoch)) = spawn {
                    self.workspace.lock().unwrap().event(
                        "harness.started",
                        Some(&id),
                        json!({
                            "prompt":"手动compact",
                            "epoch":epoch,
                            "promptVersion":SYSTEM_PROMPT_VERSION,
                            "toolsVersion":TOOL_SCHEMA_VERSION,
                            "language":self.inner.lock().unwrap().settings.language,
                            "kind":"compact",
                        }),
                    )?;
                    self.spawn_worker(id, epoch);
                }
            }
            "ack_cache_warning" => {
                self.provider.acknowledge_cache_warning()?;
                let run = self
                    .inner
                    .lock()
                    .unwrap()
                    .run
                    .as_ref()
                    .map(|run| run.id.clone());
                self.workspace.lock().unwrap().event(
                    "cache.guard.acknowledged",
                    run.as_deref(),
                    json!({"action":"explicit_acknowledgement","nextResponseStillMonitored":true}),
                )?;
            }
            "reset_context" => {
                let current = self.workspace.lock().unwrap().project.id.clone();
                let session = self.inner.lock().unwrap().conversation.id.clone();
                return self.workspace_action(&json!({"action":"new_session","baseProjectId":current,"baseSessionId":session}));
            }
            _ => bail!("未知运行控制"),
        };
        self.persist()?;
        let run = self
            .inner
            .lock()
            .unwrap()
            .run
            .as_ref()
            .map(|run| run.id.clone());
        self.workspace.lock().unwrap().event("harness.control",run.as_deref(),json!({"action":action,"source":"explicit_control","changesModelHistory":action=="reset_context"}))?;
        Ok(self.state())
    }

    fn begin_run_locked(&self, state: &mut State, prompt: &str, status: &str) -> (String, u64) {
        let id = uuid::Uuid::new_v4().to_string();
        state.epoch = state.epoch.saturating_add(1);
        let epoch = state.epoch;
        let deadline =
            tokio::time::Instant::now() + Duration::from_secs(state.settings.max_run_seconds);
        state.cancel = Cancellation::new();
        state.paused = false;
        state.phase = "running".into();
        state.run_deadline = Some(deadline);
        state.active_budget_remaining = None;
        state.queue_turn_complete = true;
        state.active_request_id = None;
        state.worker_epoch = Some(epoch);
        state.run = Some(Run {
            id: id.clone(),
            epoch,
            status: status.into(),
            prompt: prompt.into(),
            started_at: now(),
            finished_at: None,
            steps: 0,
            tokens: 0,
            active_request_id: None,
            prompt_version: SYSTEM_PROMPT_VERSION.into(),
            tools_version: TOOL_SCHEMA_VERSION.into(),
            language: state.settings.language.clone(),
            message: String::new(),
            error: String::new(),
        });
        (id, epoch)
    }

    fn prepare_worker_locked(&self, state: &mut State, epoch: u64) -> Result<()> {
        let deadline = state.run_deadline.unwrap_or_else(|| {
            let remaining = state
                .active_budget_remaining
                .take()
                .unwrap_or_else(|| Duration::from_secs(state.settings.max_run_seconds));
            tokio::time::Instant::now() + remaining
        });
        state.cancel = Cancellation::new();
        state.paused = false;
        state.phase = "running".into();
        state.run_deadline = Some(deadline);
        state.worker_epoch = Some(epoch);
        let run = state.run.as_mut().context("RUN_CONFLICT：没有活动run")?;
        run.epoch = epoch;
        run.status = "running".into();
        run.finished_at = None;
        Ok(())
    }

    fn pause_locked(&self, state: &mut State) -> Result<()> {
        if state.phase == "paused" || state.phase == "pausing" {
            return Ok(());
        }
        if !is_active_phase(&state.phase) {
            bail!("RUN_CONFLICT：没有活动run可暂停")
        }
        let remaining = state
            .run_deadline
            .take()
            .map(|deadline| deadline.saturating_duration_since(tokio::time::Instant::now()))
            .or(state.active_budget_remaining)
            .unwrap_or_else(|| Duration::from_secs(state.settings.max_run_seconds));
        state.active_budget_remaining = Some(remaining);
        state.cancel.stop();
        state.epoch = state.epoch.saturating_add(1);
        let epoch = state.epoch;
        state.phase = "pausing".into();
        state.paused = true;
        if let Some(run) = state.run.as_mut() {
            run.epoch = epoch;
            run.status = "pausing".into();
        }
        if state.worker_epoch.is_none() {
            state.phase = "paused".into();
            if let Some(run) = state.run.as_mut() {
                run.status = "paused".into();
            }
        }
        Ok(())
    }

    fn spawn_worker(&self, id: String, epoch: u64) {
        let h = self.clone();
        tokio::spawn(async move {
            let result = h.run_loop_with_budget(&id, epoch).await;
            if let Some(error) = result
                .as_ref()
                .err()
                .filter(|error| error.to_string().starts_with("CACHE_GUARD_PAUSED"))
            {
                let mut state = h.inner.lock().unwrap();
                if state
                    .run
                    .as_ref()
                    .is_some_and(|run| run.id == id && run.epoch == epoch)
                {
                    if let Some(run) = state.run.as_mut() {
                        run.error = error.to_string();
                    }
                    let _ = h.pause_locked(&mut state);
                }
            }
            h.finish(&id, epoch, result);
        });
    }

    fn resume_run(&self) -> Result<()> {
        let spawn = {
            let mut state = self.inner.lock().unwrap();
            if state.phase != "paused" {
                return Ok(());
            }
            state.epoch = state.epoch.saturating_add(1);
            let epoch = state.epoch;
            self.prepare_worker_locked(&mut state, epoch)?;
            let id = state
                .run
                .as_ref()
                .context("RUN_CONFLICT：没有可恢复run")?
                .id
                .clone();
            let mut workspace = self.workspace.lock().unwrap();
            workspace.event(
                "harness.resumed",
                Some(&id),
                json!({"epoch":epoch,"remainingSeconds":state.run_deadline.map(|deadline|deadline.saturating_duration_since(tokio::time::Instant::now()).as_secs_f64())}),
            )?;
            (id, epoch)
        };
        self.spawn_worker(spawn.0, spawn.1);
        Ok(())
    }

    fn stop_run(&self) -> Result<()> {
        let mut cancelled = Vec::new();
        let finished = {
            let mut state = self.inner.lock().unwrap();
            if !is_active_phase(&state.phase) {
                return Ok(());
            }
            state.cancel.stop();
            state.epoch = state.epoch.saturating_add(1);
            let epoch = state.epoch;
            state.phase = "stopping".into();
            if let Some(goal) = state
                .conversation
                .goal
                .as_mut()
                .filter(|g| g.status == "active")
            {
                goal.status = "stopped".into();
            }
            state.paused = false;
            state.run_deadline = None;
            state.active_budget_remaining = None;
            cancelled.extend(state.pending_interrupts.drain(..));
            cancelled.extend(state.pending_steers.drain(..));
            cancelled.extend(state.queued.drain(..));
            let id = state
                .run
                .as_ref()
                .map(|run| run.id.clone())
                .unwrap_or_default();
            if let Some(run) = state.run.as_mut() {
                run.epoch = epoch;
                run.status = "stopping".into();
            }
            let immediately_finished = state.worker_epoch.is_none();
            if immediately_finished {
                state.phase = "stopped".into();
                if let Some(run) = state.run.as_mut() {
                    run.status = "stopped".into();
                    run.finished_at = Some(now());
                }
            }
            let mut workspace = self.workspace.lock().unwrap();
            workspace.event("harness.stopping", Some(&id), json!({"epoch":epoch}))?;
            for message in &cancelled {
                workspace.event(
                    "harness.message.cancelled",
                    Some(&id),
                    json!({"requestId":message.request_id,"runId":message.run_id,"epoch":epoch,"reason":"run_stopped"}),
                )?;
            }
            if immediately_finished {
                let run = state.run.clone();
                if let Some(run) = run {
                    workspace.event("harness.finished", Some(&id), json!(&run))?;
                    write_json(
                        &workspace.root.join("runs").join(format!("{id}.json")),
                        &run,
                    )?;
                }
            }
            immediately_finished
        };
        if finished {
            self.persist()?;
        }
        Ok(())
    }
    fn finish(&self, id: &str, epoch: u64, result: Result<()>) {
        let mut spawn = None;
        let mut terminal = None;
        let mut transition = None;
        {
            let mut state = self.inner.lock().unwrap();
            if !state.run.as_ref().is_some_and(|run| run.id == id) {
                return;
            }
            if state.worker_epoch == Some(epoch) {
                state.worker_epoch = None;
            }
            let current_epoch = state
                .run
                .as_ref()
                .map(|run| run.epoch)
                .unwrap_or(state.epoch);
            if current_epoch != epoch {
                match state.phase.as_str() {
                    "interrupting" => {
                        if self
                            .prepare_worker_locked(&mut state, current_epoch)
                            .is_ok()
                        {
                            spawn = Some((id.to_owned(), current_epoch));
                            transition = Some((
                                "harness.interrupted".to_owned(),
                                json!({"epoch":current_epoch,"continuedSameRun":true}),
                            ));
                        }
                    }
                    "pausing" => {
                        state.phase = "paused".into();
                        state.paused = true;
                        if let Some(run) = state.run.as_mut() {
                            run.status = "paused".into();
                            run.active_request_id = None;
                        }
                        state.active_request_id = None;
                        transition = Some((
                            "harness.paused".to_owned(),
                            json!({"epoch":current_epoch,"activeBudgetRemainingSeconds":state.active_budget_remaining.map(|v|v.as_secs_f64())}),
                        ));
                    }
                    "stopping" => {
                        state.phase = "stopped".into();
                        state.run_deadline = None;
                        state.active_request_id = None;
                        if let Some(run) = state.run.as_mut() {
                            run.status = "stopped".into();
                            run.finished_at = Some(now());
                            run.active_request_id = None;
                            terminal = Some(run.clone());
                        }
                    }
                    _ => return,
                }
            } else {
                let result_error = result.as_ref().err().map(ToString::to_string);
                let timed_out = result_error
                    .as_ref()
                    .is_some_and(|error| error.starts_with("RUN_TIME_BUDGET"));
                let should_continue = result.is_ok()
                    && !state.cancel.stopped()
                    && (state.compact_requested
                        || !state.pending_interrupts.is_empty()
                        || !state.pending_steers.is_empty()
                        || (state.queue_turn_complete && !state.queued.is_empty()));
                if should_continue {
                    state.worker_epoch = Some(epoch);
                    spawn = Some((id.to_owned(), epoch));
                } else {
                    let stopped =
                        state.cancel.stopped() || result_error.as_deref() == Some("RUN_CANCELLED");
                    if let Some(goal) = state
                        .conversation
                        .goal
                        .as_mut()
                        .filter(|g| g.status == "active")
                    {
                        goal.status = if result_error
                            .as_ref()
                            .is_some_and(|e| e.starts_with("GOAL_BLOCKED"))
                        {
                            "blocked"
                        } else if result_error.as_ref().is_some_and(|e| {
                            e.starts_with("RUN_BUDGET") || e.starts_with("RUN_TIME_BUDGET")
                        }) {
                            "budget_exhausted"
                        } else if stopped {
                            "stopped"
                        } else {
                            "active"
                        }
                        .into();
                    }
                    state.phase = if result_error
                        .as_ref()
                        .is_some_and(|e| e.starts_with("GOAL_BLOCKED"))
                    {
                        "blocked"
                    } else if timed_out {
                        "failed"
                    } else if stopped {
                        "stopped"
                    } else if result_error.is_none() {
                        "completed"
                    } else {
                        "failed"
                    }
                    .into();
                    state.run_deadline = None;
                    state.active_budget_remaining = None;
                    state.active_request_id = None;
                    state.paused = false;
                    let terminal_status = state.phase.clone();
                    if let Some(run) = state.run.as_mut() {
                        run.status = terminal_status;
                        run.finished_at = Some(now());
                        run.active_request_id = None;
                        if let Some(error) = &result_error {
                            run.error = error.clone();
                        }
                        terminal = Some(run.clone());
                    }
                }
            }
        }
        if let Some((kind, data)) = transition {
            if let Ok(mut workspace) = self.workspace.lock() {
                let _ = workspace.event(&kind, Some(id), data);
            }
        }
        if let Some((next_id, next_epoch)) = spawn {
            self.spawn_worker(next_id, next_epoch);
            return;
        }
        if let Some(run) = terminal {
            if let Ok(mut workspace) = self.workspace.lock() {
                let _ = workspace.event("harness.finished", Some(id), json!(&run));
                let _ = write_json(
                    &workspace.root.join("runs").join(format!("{id}.json")),
                    &run,
                );
            }
            let _ = self.persist();
        } else if self.inner.lock().unwrap().phase == "paused" {
            let _ = self.persist();
        }
    }
    fn run_guard(&self, id: &str, epoch: u64) -> Result<RunGuard> {
        let state = self.inner.lock().unwrap();
        let run = state.run.as_ref().context("没有活动run")?;
        if run.id != id
            || run.epoch != epoch
            || !["running", "compacting"].contains(&state.phase.as_str())
        {
            bail!("RUN_SUPERSEDED：run epoch已变化")
        }
        Ok(RunGuard {
            id: id.to_owned(),
            epoch,
            deadline: state.run_deadline.unwrap_or_else(tokio::time::Instant::now),
            cancel: state.cancel.clone(),
        })
    }

    fn guard_is_current(&self, guard: &RunGuard) -> bool {
        let state = self.inner.lock().unwrap();
        guard_matches_locked(&state, guard)
    }

    async fn run_loop_with_budget(&self, id: &str, epoch: u64) -> Result<()> {
        let guard = self.run_guard(id, epoch)?;
        self.run_loop(guard).await
    }
    async fn run_loop(&self, guard: RunGuard) -> Result<()> {
        let mut incomplete_retries = 0usize;
        let mut retry_note = String::new();
        loop {
            if guard.cancel.stopped() || !self.guard_is_current(&guard) {
                bail!("RUN_CANCELLED")
            }
            if tokio::time::Instant::now() >= guard.deadline {
                guard.cancel.stop();
                bail!("RUN_TIME_BUDGET：run超过配置时长，已保存已提交内容")
            }

            let (settings, step, tokens, force_compact, queue_turn_complete, manual_compact) = {
                let state = self.inner.lock().unwrap();
                let run = state.run.as_ref().context("没有活动run")?;
                if run.id != guard.id || run.epoch != guard.epoch {
                    bail!("RUN_SUPERSEDED：run epoch已变化")
                }
                (
                    state.settings.clone(),
                    run.steps,
                    run.tokens,
                    state.compact_requested,
                    state.queue_turn_complete,
                    run.prompt == "手动compact",
                )
            };
            if step >= settings.max_steps || tokens >= settings.max_run_tokens {
                bail!("RUN_BUDGET：达到步骤或Token预算，已提交版本仍保留")
            }

            let model_limits = tokio::select! {
                value=self.provider.discover_model_limits(&settings.model)=>value,
                _=guard.cancel.wait()=>bail!("RUN_CANCELLED"),
                _=tokio::time::sleep_until(guard.deadline)=>bail!("RUN_TIME_BUDGET：读取模型能力时超过时限"),
            };
            let effective_context_window = model_limits.context(settings.context_window_tokens);
            let compact_threshold = settings
                .compact_tokens
                .min(effective_context_window.saturating_sub(4096));
            let mut previous_input_tokens =
                latest_provider_input_tokens(&self.workspace.lock().unwrap(), &guard.id);
            let estimated_history = estimate(&self.inner.lock().unwrap().conversation.input);
            if force_compact
                || (settings.auto_compact
                    && (previous_input_tokens
                        .is_some_and(|tokens| tokens > compact_threshold as u64)
                        || estimated_history > compact_threshold))
            {
                self.compact_with_guard(&guard, force_compact).await?;
                previous_input_tokens = None;
                let mut state = self.inner.lock().unwrap();
                if state
                    .run
                    .as_ref()
                    .is_some_and(|run| run.id == guard.id && run.epoch == guard.epoch)
                {
                    state.compact_requested = false;
                }
                if manual_compact {
                    return Ok(());
                }
                let run = state.run.as_ref().context("没有活动run")?;
                if run.steps >= settings.max_steps || run.tokens >= settings.max_run_tokens {
                    bail!("RUN_BUDGET：Provider实际用量已达到步骤或Token预算，停止发起新请求")
                }
            }

            self.apply_pending_messages(&guard, queue_turn_complete)?;
            if !self.guard_is_current(&guard) {
                bail!("RUN_CANCELLED")
            }
            {
                let mut state = self.inner.lock().unwrap();
                if state
                    .run
                    .as_ref()
                    .is_none_or(|run| run.id != guard.id || run.epoch != guard.epoch)
                {
                    bail!("RUN_SUPERSEDED：run epoch已变化")
                }
                // A queue prompt is eligible only after a completed assistant turn.
                // Any request that follows, including an incomplete retry, reopens the turn.
                state.queue_turn_complete = false;
            }

            let (
                mut input,
                settings,
                steps_used,
                tokens_used,
                references,
                reference_snapshot,
                previous_context,
                context_epoch,
                requested_length,
            ) = {
                let state = self.inner.lock().unwrap();
                let run = state.run.as_ref().context("没有活动run")?;
                (
                    state.conversation.input.clone(),
                    state.settings.clone(),
                    run.steps,
                    run.tokens,
                    state.conversation.reference_images.clone(),
                    state.conversation.reference_snapshot.clone(),
                    state.conversation.runtime_context.clone(),
                    state.conversation.compact_count,
                    state.conversation.requested_input_length,
                )
            };
            let history_length = input.len();
            let additions = if settings.send_images {
                context::reference_update(&references, reference_snapshot.as_deref())
            } else {
                Vec::new()
            };
            input.extend(additions.iter().cloned());
            let context = context::authority_snapshot(&self.context());
            let context_changes = context::changes(&previous_context, &context);
            let system = format!(
                "{}\n{}\n{}",
                prompts::system_prompt(&settings.language),
                prompts::preset_prompt(&settings.preset, &settings.language),
                settings.detail_instructions
            );
            let tool_schemas = tools_for_language(&settings.language);
            let remaining_steps = settings.max_steps.saturating_sub(steps_used);
            let remaining_tokens = settings.max_run_tokens.saturating_sub(tokens_used);
            // Request estimates guide early completion only. Provider-reported usage
            // is the source of truth for the cumulative run-token budget.
            let mut estimate_input = input.clone();
            if !settings.send_images {
                omit_images(&mut estimate_input);
            }
            let estimated_input_tokens = estimate(&estimate_input)
                .saturating_add(estimate(std::slice::from_ref(&context_changes)))
                .saturating_add(estimate(std::slice::from_ref(&tool_schemas)))
                .saturating_add(system.len() / 3)
                .saturating_add(2048);
            let estimated_input_tokens_u64 =
                u64::try_from(estimated_input_tokens).unwrap_or(u64::MAX);
            if remaining_tokens == 0 {
                bail!("RUN_BUDGET：Provider实际用量已达到Token预算，停止发起新请求")
            }
            let input_for_headroom = estimated_input_tokens.max(
                previous_input_tokens
                    .unwrap_or_default()
                    .min(usize::MAX as u64) as usize,
            );
            let output_run_budget = if self.provider.cache_required(&settings.cache_guard) {
                remaining_tokens.saturating_sub(input_for_headroom as u64)
            } else {
                remaining_tokens
            };
            if output_run_budget < 512 {
                bail!("RUN_BUDGET：剩余Token不足以预留下一请求的估计输入和输出；已保留草稿")
            }
            let effective_output_tokens =
                model_limits.output_allowance(&settings, input_for_headroom, output_run_budget)?;
            let remaining_seconds = guard
                .deadline
                .saturating_duration_since(tokio::time::Instant::now())
                .as_secs_f64();
            // Leave room for a final edit, capture, and evidence-based finish.
            // Actual visual input can cost more than the local image estimate.
            let request_budget_estimate = estimated_input_tokens_u64
                .max(previous_input_tokens.unwrap_or_default())
                .saturating_add(effective_output_tokens as u64);
            let completion_reserve_tokens = request_budget_estimate.saturating_mul(3);
            let near_budget = remaining_steps <= 3
                || remaining_tokens <= completion_reserve_tokens
                || remaining_seconds <= 120.0;
            let budget_note = if settings.language == "en" {
                format!(
                    "\nCurrent run budget: {remaining_steps} steps, {remaining_tokens} provider-reported total tokens, and {remaining_seconds:.0} active seconds remain. Effective output cap for this request: {effective_output_tokens}; it never exceeds the configured cap or remaining token budget. Strict cache mode reserves estimated input tokens before calculating output allowance; estimates are not exact billing. Provider input usage can exceed the remaining budget unexpectedly; at most one complete response may cross it, and no later model request will start.{}",
                    if near_budget {
                        " Enter completion now: reserve the remaining requests for at most one necessary correction, capture_views of the resulting revision, then finish_artwork after inspecting that capture in a later response. If the current revision is already captured, review it and finish directly. Do not add optional detail or finish in the same response that first captures an unseen revision. Report unresolved visual issues honestly."
                    } else {
                        ""
                    }
                )
            } else {
                format!(
                    "\n当前run预算：剩余{remaining_steps}步、Provider实际统计的总Token余量{remaining_tokens}和{remaining_seconds:.0}秒活动时间。本请求有效输出上限{effective_output_tokens}，不会超过配置上限或实际剩余预算。输入估算仅作提前收尾提示。Provider实际输入用量可能不可预测地超过余量；最多允许一个完整响应越过预算，之后不再发起模型请求。{}",
                    if near_budget {
                        "现在进入收尾：预留后续请求用于至多一次必要修正、capture_views检查最终revision、在下一响应看过该截图后finish_artwork。若当前revision已有截图，检查后直接收尾。不再追加可选细节，不要在首次获取尚未看过的revision截图的同一响应中宣告完成。对仍存在的外观问题如实说明。"
                    } else {
                        ""
                    }
                )
            };
            let runtime_message =
                context::runtime_message(&context_changes, &budget_note, &retry_note);
            let runtime_placement =
                context::append_runtime(&mut input, requested_length, runtime_message);
            let requested_input_length = input.len();
            let mut delivery_input = input.clone();
            if !settings.send_images {
                omit_images(&mut delivery_input);
            }
            let request_id = uuid::Uuid::new_v4().to_string();
            let requested_history_tokens = estimate(&delivery_input);
            let payload = json!({
                "model":settings.model,
                "instructions":system,
                "input":delivery_input,
                "tools":tool_schemas,
                "reasoning":{"effort":settings.reasoning},
                "max_output_tokens":effective_output_tokens,
                "stream":true
            });
            let usage_scope = {
                let mut state = self.inner.lock().unwrap();
                if state
                    .run
                    .as_ref()
                    .is_none_or(|run| run.id != guard.id || run.epoch != guard.epoch)
                {
                    bail!("RUN_SUPERSEDED：run epoch已变化")
                }
                state.active_request_id = Some(request_id.clone());
                if let Some(run) = state.run.as_mut() {
                    run.active_request_id = Some(request_id.clone());
                }
                if state.conversation.input.len() != history_length {
                    bail!("CONTEXT_CONFLICT：准备请求时历史已变化；未覆盖会话")
                }
                let previous = state.conversation.clone();
                state.conversation.input = input;
                state.conversation.requested_input_length = requested_input_length;
                state.conversation.runtime_context = context;
                if settings.send_images {
                    state.conversation.reference_snapshot =
                        Some(context::reference_fingerprint(&references));
                }
                let root = self.workspace.lock().unwrap().root.clone();
                if let Err(error) =
                    write_json(&root.join("runs/conversation.json"), &state.conversation)
                {
                    state.conversation = previous;
                    return Err(error);
                }
                self.workspace.lock().unwrap().event(
                    "harness.request.prepared",
                    Some(&guard.id),
                    json!({
                        "requestId":request_id,
                        "epoch":guard.epoch,
                        "step":state.run.as_ref().map(|run|run.steps + 1),
                        "promptVersion":SYSTEM_PROMPT_VERSION,
                        "toolsVersion":TOOL_SCHEMA_VERSION,
                        "language":settings.language,
                        "incompleteRetry":incomplete_retries,
                        "remainingSteps":remaining_steps,
                        "remainingTokens":remaining_tokens,
                        "remainingSeconds":remaining_seconds,
                        "estimatedInputTokens":estimated_input_tokens,
                        "effectiveMaxOutputTokens":effective_output_tokens,
                        "completionPhase":near_budget,
                        "completionReserveTokens":completion_reserve_tokens,
                        "contextEpoch":context_epoch,
                        "contextPolicy":"append-only-until-compact",
                        "runtimePlacement":runtime_placement,
                        "authorityChangedFields":context_changes.as_object().map(|fields|fields.len()),
                        "preset":settings.preset,
                        "contextWindowTokens":settings.context_window_tokens,
                        "effectiveContextWindow":effective_context_window,
                        "upstreamModelLimits":model_limits,
                        "goalMode":settings.goal_mode,
                        "referenceImages":state.conversation.reference_images,
                    }),
                )?;
                context_usage::scope(
                    &state.conversation,
                    &settings,
                    &context_usage::provider_scope(&self.provider.connection()),
                )
            };
            let trace = ProviderRequestTrace {
                request_id: Some(request_id.clone()),
                prompt_version: Some(SYSTEM_PROMPT_VERSION.into()),
                tools_version: Some(TOOL_SCHEMA_VERSION.into()),
                language: Some(settings.language.clone()),
                epoch: Some(guard.epoch),
                context_epoch: Some(context_epoch),
                image_policy: Some(settings.send_images),
                compact: false,
                cache_guard: settings.cache_guard.clone(),
                strict_append_only: Some(settings.strict_append_only),
                estimated_input_tokens: estimated_input_tokens_u64,
            };
            let response_result = tokio::select! {
                biased;
                result = self.provider.response_with_trace(
                    payload,
                    self.workspace.clone(),
                    &guard.id,
                    &guard.cancel,
                    trace,
                ) => result,
                _ = tokio::time::sleep_until(guard.deadline) => {
                    guard.cancel.stop();
                    self.workspace.lock().unwrap().event(
                        "harness.request.cancelled",
                        Some(&guard.id),
                        json!({"requestId":request_id,"epoch":guard.epoch,"reason":"run_budget"}),
                    )?;
                    Err(anyhow::anyhow!("RUN_TIME_BUDGET：run超过配置时长，已停止并保留已提交几何"))
                }
            };
            let response = match response_result {
                Ok(response) => response,
                Err(error) => {
                    self.clear_active_request(&guard, &request_id);
                    if error.to_string().starts_with("CACHE_GUARD_PAUSED") {
                        let mut state = self.inner.lock().unwrap();
                        if let Some(run) = state.run.as_mut() {
                            run.error = error.to_string();
                        }
                        self.pause_locked(&mut state)?;
                    }
                    return Err(error);
                }
            };
            let usage = response["usage"]["total_tokens"].as_u64().unwrap_or_else(|| {
                // Some compatible servers omit usage. Count a conservative local estimate
                // rather than treating requests as free under a cumulative token budget.
                let estimate = estimated_input_tokens_u64.saturating_add(estimate(std::slice::from_ref(&response)) as u64);
                let _ = self.workspace.lock().unwrap().event("harness.usage.estimated",Some(&guard.id),json!({"requestId":request_id,"tokens":estimate,"reason":"Provider未报告usage；使用本地估算"}));
                estimate
            });
            self.record_request_attempt(&guard, &request_id, usage);
            if let Some(input_tokens) = response["usage"]["input_tokens"].as_u64() {
                let mut state = self.inner.lock().unwrap();
                if guard_matches_locked(&state, &guard) {
                    state.conversation.context_usage = Some(ContextUsageAnchor {
                        request_id: request_id.clone(),
                        scope: usage_scope,
                        input_tokens,
                        history_messages: requested_input_length,
                        history_tokens_estimate: requested_history_tokens,
                        measured_at: now(),
                    });
                }
            }

            if response["status"] == "incomplete" {
                let reason = response["incomplete_details"]["reason"]
                    .as_str()
                    .or_else(|| response["reason"].as_str())
                    .unwrap_or("unknown");
                if reason == "max_output_tokens" && incomplete_retries < 2 {
                    let (steps, total_tokens) = {
                        let state = self.inner.lock().unwrap();
                        let run = state.run.as_ref().context("没有活动run")?;
                        (run.steps, run.tokens)
                    };
                    if steps < settings.max_steps && total_tokens < settings.max_run_tokens {
                        incomplete_retries += 1;
                        retry_note = format!(
                            "\n上一请求因 max_output_tokens 达到上限而不完整；没有执行其中任何工具。请把分析压缩为短计划，只完成一个适量的小阶段，使用紧凑函数/循环与精简工具参数；先保存当前阶段，后续再细化。不要重复已完成几何，也不要输出长篇说明。最大输出 Token 配置保持不变。恢复次数：{incomplete_retries}/2。"
                        );
                        self.workspace.lock().unwrap().event(
                            "harness.incomplete.retry",
                            Some(&guard.id),
                            json!({
                                "requestId":request_id,
                                "epoch":guard.epoch,
                                "reason":reason,
                                "retry":incomplete_retries,
                                "maxRetries":2,
                                "maxOutputTokens":effective_output_tokens,
                                "promptVersion":SYSTEM_PROMPT_VERSION,
                                "toolsVersion":TOOL_SCHEMA_VERSION,
                            }),
                        )?;
                        continue;
                    }
                }
                bail!("RESPONSE_INCOMPLETE：模型响应不完整（{reason}），未执行部分工具")
            }
            if response["status"] != "completed" {
                bail!("响应未完整完成")
            }
            incomplete_retries = 0;
            retry_note.clear();
            let output = response["output"]
                .as_array()
                .context("模型响应缺少output数组")?
                .clone();
            validate_completed_output(&output)?;
            let called = output.iter().any(|item| item["type"] == "function_call");
            if let Some(text) = assistant_text(&output) {
                let mut state = self.inner.lock().unwrap();
                if let Some(run) = state.run.as_mut().filter(|run| run.id == guard.id) {
                    run.message = text;
                }
            }

            let mut tool_outputs = Vec::new();
            let mut budget_expired = false;
            let mut cancel_remaining = false;
            let mut finish_succeeded = false;
            let mut engine_fault = false;
            for item in &output {
                if item["type"] != "function_call" {
                    continue;
                }
                let call_id = item["call_id"]
                    .as_str()
                    .context("工具调用缺少call_id")?
                    .to_owned();
                let name = item["name"]
                    .as_str()
                    .context("工具调用缺少name")?
                    .to_owned();
                let arguments_raw = item["arguments"].as_str().unwrap_or("{}").to_owned();
                let started = std::time::Instant::now();
                self.workspace.lock().unwrap().event(
                    "tool.request",
                    Some(&guard.id),
                    json!({
                        "callId":call_id,
                        "name":name,
                        "arguments":arguments_raw,
                        "requestId":request_id,
                        "epoch":guard.epoch,
                        "toolVersion":TOOL_SCHEMA_VERSION,
                    }),
                )?;

                let mut cancelled = cancel_remaining || !self.guard_is_current(&guard);
                let result: Result<Value> = if cancelled {
                    Err(anyhow::anyhow!("RUN_CANCELLED"))
                } else {
                    match serde_json::from_str::<Value>(&arguments_raw) {
                        Ok(mut arguments) => {
                            if name == "recipe_sources" && arguments["action"] == "run"
                                || name == "run_recipe"
                                || (name == "voxel_text" && arguments["mode"] == "apply")
                                || (name == "components" && arguments["action"] == "place")
                            {
                                arguments["requestId"] = json!(call_id);
                            }
                            tokio::select! {
                                biased;
                                value = self.tool_with_guard(&name, &arguments, Some(&guard.id), guard.clone()) => value,
                                _ = guard.cancel.wait() => Err(anyhow::anyhow!("RUN_CANCELLED")),
                                _ = tokio::time::sleep_until(guard.deadline) => {
                                    guard.cancel.stop();
                                    budget_expired = true;
                                    Err(anyhow::anyhow!("RUN_TIME_BUDGET"))
                                }
                            }
                        }
                        Err(error) => Err(error.into()),
                    }
                };
                let mut value = match result {
                    Ok(value) => value,
                    Err(error)
                        if error.to_string().starts_with("RUN_CANCELLED")
                            || error.to_string().starts_with("RUN_SUPERSEDED")
                            || error.to_string().starts_with("RUN_TIME_BUDGET") =>
                    {
                        cancelled = true;
                        if error.to_string().starts_with("RUN_TIME_BUDGET") {
                            budget_expired = true;
                        }
                        json!({"ok":false,"error":{"code":"CANCELLED","message":"工具在提交前被取消；没有应用未完成的操作。"}})
                    }
                    Err(error) if error.to_string().starts_with("RECIPE_ENGINE_FAULT") => {
                        engine_fault = true;
                        cancel_remaining = true;
                        let message = self.provider.redact(&error.to_string());
                        {
                            let mut state = self.inner.lock().unwrap();
                            if let Some(run) = state.run.as_mut() {
                                run.error = message.clone();
                            }
                            self.pause_locked(&mut state)?;
                        }
                        json!({"ok":false,"error":{"code":"RECIPE_ENGINE_FAULT","message":message},"paused":true})
                    }
                    Err(error) => json!({
                        "ok":false,
                        "error":{"code":"TOOL_ERROR","message":self.provider.redact(&error.to_string())}
                    }),
                };
                if value["images"]
                    .as_array()
                    .is_some_and(|images| !images.is_empty())
                {
                    value["visualInput"] = json!({"attached":settings.send_images,"note":if settings.send_images{"图片会附在本工具结果之后；下一响应才可查看"}else{"当前设置不向模型发送图片，不能声称已经看过"}});
                }
                if name == "finish_artwork" && !cancelled && value["finished"] == true {
                    finish_succeeded = true;
                }
                if cancelled {
                    cancel_remaining = true;
                }
                self.workspace.lock().unwrap().event(
                    "tool.result",
                    Some(&guard.id),
                    json!({
                        "callId":call_id,
                        "name":name,
                        "epoch":guard.epoch,
                        "elapsedMs":started.elapsed().as_secs_f64()*1000.0,
                        "cancelled":cancelled,
                        "result":value,
                    }),
                )?;
                let mut parts = vec![json!({
                    "type":"input_text",
                    "text":serde_json::to_string(&value)?,
                })];
                if !cancelled && settings.send_images {
                    for image in value["images"].as_array().into_iter().flatten() {
                        if let Some(artifact) = image["artifact"].as_str() {
                            // Persist local references, resolve delivery only at the provider boundary.
                            parts.push(json!({"type":"input_text","text":format!("真实作品截图 / Scene capture: role={}，revision={}，view={}。{}",value["imageRole"].as_str().unwrap_or("scene_capture"),value["revision"],image["view"].as_str().unwrap_or("view"),if value["imageRole"]=="reinspection"{"重读历史/参考图片，不能作为当前场景完成证据。"}else{"真实场景截图，用于复核当前标注版本。"})}));
                            parts.push(
                                json!({"type":"input_image","artifact":artifact,"detail":"high"}),
                            );
                        }
                    }
                }
                tool_outputs.push(json!({
                    "type":"function_call_output",
                    "call_id":call_id,
                    "output":parts,
                }));
            }
            self.append_response_history(
                &guard,
                &output,
                tool_outputs,
                !called || finish_succeeded,
            )?;
            if engine_fault {
                bail!("RUN_CANCELLED")
            };
            if budget_expired {
                bail!("RUN_TIME_BUDGET：run超过配置时长，已保存已提交内容")
            }
            if !self.guard_is_current(&guard) {
                bail!("RUN_CANCELLED")
            }
            if finish_succeeded {
                return Ok(());
            }
            if !called {
                if self.continue_goal(&guard)? {
                    continue;
                }
                let state = self.inner.lock().unwrap();
                let has_next_work = !state.pending_interrupts.is_empty()
                    || !state.pending_steers.is_empty()
                    || (state.queue_turn_complete && !state.queued.is_empty())
                    || state.compact_requested;
                if !has_next_work {
                    return Ok(());
                }
            }
        }
    }

    fn clear_active_request(&self, guard: &RunGuard, request_id: &str) {
        let mut state = self.inner.lock().unwrap();
        if state.active_request_id.as_deref() == Some(request_id) {
            state.active_request_id = None;
        }
        if let Some(run) = state.run.as_mut().filter(|run| run.id == guard.id) {
            if run.active_request_id.as_deref() == Some(request_id) {
                run.active_request_id = None;
            }
        }
    }

    fn record_request_attempt(&self, guard: &RunGuard, request_id: &str, tokens: u64) {
        let mut state = self.inner.lock().unwrap();
        if let Some(run) = state.run.as_mut().filter(|run| run.id == guard.id) {
            run.steps = run.steps.saturating_add(1);
            run.tokens = run.tokens.saturating_add(tokens);
            if run.active_request_id.as_deref() == Some(request_id) {
                run.active_request_id = None;
            }
        }
        if state.active_request_id.as_deref() == Some(request_id) {
            state.active_request_id = None;
        }
    }

    fn apply_pending_messages(&self, guard: &RunGuard, allow_queue: bool) -> Result<()> {
        let mut state = self.inner.lock().unwrap();
        if state
            .run
            .as_ref()
            .is_none_or(|run| run.id != guard.id || run.epoch != guard.epoch)
            || !["running", "compacting"].contains(&state.phase.as_str())
        {
            bail!("RUN_CANCELLED")
        }
        let mut pending = Vec::new();
        pending.extend(state.pending_interrupts.drain(..));
        pending.extend(state.pending_steers.drain(..));
        if allow_queue {
            if let Some(message) = state.queued.pop_front() {
                pending.push(message);
            }
        }
        if pending.is_empty() {
            return Ok(());
        }
        for message in pending {
            let reference_changed =
                !state.conversation.reference_images.is_empty() || !message.references.is_empty();
            state.conversation.reference_images = message.references.clone();
            if state.settings.goal_mode && !message.prompt.trim().is_empty() {
                let workspace = self.workspace.lock().unwrap();
                let new_goal = state.conversation.goal.as_ref().is_none_or(|goal| {
                    goal.project_id != workspace.project.id
                        || goal.status == "achieved"
                        || message.mode == "queue"
                });
                if new_goal {
                    state.conversation.goal = Some(GoalProgress {
                        id: uuid::Uuid::new_v4().to_string(),
                        project_id: workspace.project.id.clone(),
                        objective: message.prompt.clone(),
                        criteria: state.settings.goal_criteria.clone(),
                        status: "active".into(),
                        min_review_rounds: state.settings.goal_min_review_rounds,
                        reviewed_geometry_hashes: Vec::new(),
                        continuations: 0,
                        no_progress_attempts: 0,
                        last_progress_signature: String::new(),
                        assessment: String::new(),
                    });
                } else {
                    let criteria = state.settings.goal_criteria.clone();
                    let rounds = state.settings.goal_min_review_rounds;
                    let goal = state.conversation.goal.as_mut().unwrap();
                    goal.objective.push_str(&format!("\n{}", message.prompt));
                    goal.criteria = criteria;
                    goal.min_review_rounds = rounds;
                    goal.status = "active".into();
                    goal.continuations = 0;
                    goal.no_progress_attempts = 0;
                }
                // Log objective and acceptance settings for iteration; no credentials are involved.
                drop(workspace);
                self.workspace.lock().unwrap().event(
                    "goal.updated",
                    Some(&guard.id),
                    json!(state.conversation.goal),
                )?;
            }
            state.conversation.goals.push(message.prompt.clone());
            state
                .conversation
                .input
                .push(json!({"role":"user","content":if !reference_changed {message.prompt.clone()} else {format!("{}\n当前参考图片：{}",message.prompt,if message.references.is_empty(){"无（取消之前的视觉参考约束，按本次要求续作）".into()}else{message.references.iter().map(|r|r.title.as_str()).collect::<Vec<_>>().join("、")})}}));
            let index = state.conversation.input.len() - 1;
            self.workspace.lock().unwrap().event(
                "harness.message.applied",
                Some(&guard.id),
                json!({
                    "requestId":message.request_id,
                    "runId":guard.id,
                    "mode":message.mode,
                    "epoch":guard.epoch,
                    "acceptedEpoch":message.accepted_epoch,
                    "messageIndex":index,
                    "references":message.references,
                }),
            )?;
        }
        drop(state);
        self.persist()
    }

    fn append_response_history(
        &self,
        guard: &RunGuard,
        output: &[Value],
        tool_outputs: Vec<Value>,
        turn_complete: bool,
    ) -> Result<()> {
        let mut state = self.inner.lock().unwrap();
        if state.run.as_ref().is_none_or(|run| run.id != guard.id) {
            bail!("RUN_SUPERSEDED：run已替换")
        }
        state.conversation.input.extend(output.iter().cloned());
        state.conversation.input.extend(tool_outputs);
        let boundary = state.conversation.input.len();
        state.conversation.boundaries.push(boundary);
        state.queue_turn_complete = turn_complete;
        drop(state);
        self.persist()
    }
    pub fn context(&self) -> Value {
        self.context_with_options(&json!({}))
    }
    fn context_with_options(&self, args: &Value) -> Value {
        let (active_goal, reference_images, enforce_target) = {
            let state = self.inner.lock().unwrap();
            (
                state.conversation.goal.clone(),
                state.conversation.reference_images.clone(),
                state.settings.enforce_target_limits,
            )
        };
        let workspace = self.workspace.lock().unwrap();
        let query = args["styleQuery"].as_str().unwrap_or("").to_lowercase();
        let offset = args["styleOffset"].as_u64().unwrap_or(0) as usize;
        let limit = args["styleLimit"].as_u64().unwrap_or(128).clamp(1, 128) as usize;
        let matching = workspace
            .project
            .materials
            .iter()
            .filter(|material| {
                query.is_empty()
                    || format!("{} {} {}", material.uid, material.name, material.family)
                        .to_lowercase()
                        .contains(&query)
            })
            .collect::<Vec<_>>();
        let styles = matching
            .iter()
            .skip(offset)
            .take(limit)
            .map(|material| {
                json!({
                    "id":material.id,
                    "uid":material.uid,
                    "name":material.name,
                    "family":material.family,
                    "color":material.color,
                    "renderMode":material.render_mode,
                    "surface":{"metallic":material.metallic,"roughness":material.roughness,"opacity":material.opacity,"emission":material.emission,"procedural":material.procedural},
                })
            })
            .collect::<Vec<_>>();
        let catalog = json!({
            "total":workspace.project.materials.len(),
            "matched":matching.len(),
            "offset":offset,
            "limit":limit,
            "nextOffset":if offset.saturating_add(limit)<matching.len(){Some(offset+limit)}else{None},
            "query":query,
            "hint":"get_context accepts styleQuery/styleOffset/styleLimit; choose styles by ID or UID"
        });
        let stats = workspace.project.scene.stats(&workspace.project.limits);
        let outside = workspace
            .project
            .scene
            .boxes
            .iter()
            .filter(|box3| {
                let end = box3.end();
                box3.min[0] < -128
                    || end[0] > 128
                    || box3.min[2] < -128
                    || end[2] > 128
                    || box3.min[1] < 0
                    || end[1] > 256
            })
            .collect::<Vec<_>>();
        let target_compatibility = json!({
            "target":"VRChat",
            "compatible":outside.is_empty(),
            "outOfBoundsBoxCount":outside.len(),
            "outOfBoundsExamples":outside.iter().take(8).map(|box3|json!({"min":box3.min,"size":box3.size,"group":box3.group})).collect::<Vec<_>>(),
            "bounds":stats.bounds,
            "requiredRange":{"x":[-128,128],"y":[0,256],"z":[-128,128]},
            "diagnosticOnly":true,
            "warning":if outside.is_empty(){Value::Null}else{json!("当前几何超出VRChat坐标范围；普通艺术工程允许超出，仅在目标为VRChat时收敛。")},
        });
        json!({
            "revision":workspace.project.scene.revision,
            "projectId":workspace.project.id,
            "projectName":workspace.project.name,
            "deliveryTarget":if enforce_target {"vrchat"}else{"art"},
            "modelingStyle":workspace.project.modeling_style,
            "modelingExtent":workspace.project.modeling_extent,
            "stats":stats,
            "targetCompatibility":target_compatibility,
            "placement":crate::scene_tools::target_report(&workspace.project.scene,self.transport.config().capacity),
            "materialCompatibility":self.transport.material_report(&workspace.project.scene,&workspace.project.materials),
            "builtinMaterialPacks":crate::builtin_pixels::list(),
            "autoCenterOnFinish":workspace.project.auto_center_on_finish,
            "protectedGroups":workspace.project.protected_groups,
            "styles":styles,
            "catalog":catalog,
            "pinnedNotes":workspace.project.pinned_notes,
            "sourceCredits":workspace.project.source_credits,
            "libraryHints":{"builtinExamples":7,"tool":"artworks action=search returns personal artworks and immutable examples; load/fork needs id and current baseRevision and creates an editable copy."},
            "goal":active_goal,
            "referenceImages":reference_images,
            "viewConventions":{"front":{"cameraFrom":"+Z","looksToward":"-Z"},"hero":{"cameraFrom":"+X,+Y,+Z"},"placementHint":"脸、眼睛、围裙、门窗、正面文字应位于主体的+Z外表面；更大的Z更靠近front相机。"},
            "sync":self.transport.state(),
            "runtimeSurfaceHints":{"mf6":"define_palette/define_materials可实时发送，不需重传头像；新图片仍需打包。查看工具返回的vrcEffective了解量化。","scale":"placement.scale保持实际倍数；新100%=.05，MF6范围.0005.. .5（1%..1000%）。","transparency":"MF6目录透明材质可剔内部面；实时透明表面暂不带六面遮罩。"},
            "recipeSources":crate::sources::list(&workspace).unwrap_or(Value::Null),
            "creative":workspace.project.creative.brief(),
            "sdkVersion":recipe::SDK_VERSION,
            "meshSdk":"mf.triangle(a,b,c,{color:'#RRGGBB',doubleSided:false,group}); mf.mesh(vertices,faces,{color,colors:[per-face RGB8 or hex],group,doubleSided}); faces are triples of vertex indices. mf.triangleStrip(vertices,{colors}); mf.slope(min,size,{style:styleId,direction:'+x'|'-x'|'+z'|'-z',group}); // textured voxel slope, one object. mf.wedge(min,size,{direction:'+x'|'-x'|'+z'|'-z',color}); integer coordinates, per-face RGB8, no UV. Same translate/mirror/group frame as mf.box. mesh_tools can inspect/edit stable face IDs. Region erase removes whole faces by AABB; it does not clip triangles. MF6 required for triangle OSC; art-only editing is independent of VRChat.",
            "sdk":"mf.box(min,[w,h,d],{style:'wood',tint:0,motion:0}); mf.erase(min,size); mf.paint(min,size,{style}) recolors existing voxels only and never creates them; use mf.box to add voxels. mf.clear(); mf.removeGroup(name); mf.replaceGroup(name,()=>{}); mf.ellipsoid(center,radii,options); mf.sphere(center,radius,options); mf.capsule(a,b,radius,options); mf.cylinder(base,height,radius,options) Y-axis; mf.tube(points,radius,options); mf.extrude(points2d,origin,depth,{plane:\"xy\",style}); mf.voxelize({min,max},(x,y,z)=>boolean_or_style,options) samples cell centers, max2M cells; mf.group(name,()=>{}); mf.repeat(n,(i)=>{}); mf.mirror('x',()=>{}); mf.translate([x,y,z],()=>{}); mf.line(a,b,{thickness:1,style}); mf.stairs(origin,n,{width:3,depth:1,rise:1,style}); mf.wall(origin,w,h,{thickness:1,style}); mf.roof(origin,w,d,h,{style}); Math.random()/mf.random() deterministic by seed. All coordinates integer signed grid. Y is height. VRC target requires X/Z[-128,128),Y[0,256). Styles are catalog IDs/UID names. mode apply preserves current geometry; replace starts from empty."
        })
    }
    pub async fn tool(&self, name: &str, args: &Value, run: Option<&str>) -> Result<Value> {
        let result=self.tool_inner(name,args,run,None).await?;
        Ok(self.with_resource_receipt(result))
    }

    async fn tool_with_guard(
        &self,
        name: &str,
        args: &Value,
        run: Option<&str>,
        guard: RunGuard,
    ) -> Result<Value> {
        let result=self.tool_inner(name,args,run,Some(guard)).await?;
        Ok(self.with_resource_receipt(result))
    }

    fn with_resource_receipt(&self,mut result:Value)->Value {
        if result.is_object() {
            let workspace=self.workspace.lock().unwrap();
            let capacity=self.transport.config().capacity;
            let stats=workspace.project.scene.stats(&workspace.project.limits);
            result["resources"]=json!({"revision":workspace.project.scene.revision,"boxCount":stats.box_count,"triangleCount":workspace.project.scene.triangles.len(),"voxelCount":stats.voxel_count,"voxelCountMeaning":"occupied Box-cell volume; triangle surfaces are counted separately","target":crate::scene_tools::target_report(&workspace.project.scene,capacity)});
        }
        result
    }
    async fn tool_inner(
        &self,
        name: &str,
        args: &Value,
        run: Option<&str>,
        guard: Option<RunGuard>,
    ) -> Result<Value> {
        if let Some(ref guard) = guard {
            if !self.guard_is_current(guard) {
                bail!("RUN_CANCELLED")
            }
        }
        if name == "artworks" && matches!(args["action"].as_str(), Some("load" | "fork")) {
            let mut state = self.inner.lock().unwrap();
            if let Some(ref guard) = guard {
                if !guard_matches_locked(&state, guard) {
                    bail!("RUN_CANCELLED")
                }
            } else if is_active_phase(&state.phase) || state.worker_epoch.is_some() {
                bail!("RUN_CONFLICT：先停止AI运行，再载入作品副本")
            }
            let mut workspace = self.workspace.lock().unwrap();
            let base = args["baseRevision"]
                .as_u64()
                .context("载入作品需要baseRevision")?;
            if base != workspace.project.scene.revision {
                bail!("REVISION_CONFLICT")
            }
            let id = args["id"].as_str().context("载入作品需要id")?;
            let result = workspace.load_artwork(id)?;
            if guard.is_none() {
                state.conversation.goal = None;
                state.conversation.input.push(json!({"role":"user","content":format!("用户已载入作品副本：{}（{}）。后续以当前工程几何为准，旧目标已结束。",workspace.project.name,id)}));
                write_json(
                    &workspace.root.join("runs/conversation.json"),
                    &state.conversation,
                )?;
            }
            return Ok(result);
        }
        if name == "read_artifact" {
            return self.with_workspace_guard(guard.as_ref(),|workspace|{
                let artifact=args["artifact"].as_str().context("需要artifact文件名")?;
                if artifact.len()>100||artifact.contains("..")||!artifact.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'.'|b'_'|b'-')){bail!("无效artifact")}
                let bytes=std::fs::read(workspace.root.join("artifacts").join(artifact))?;
                if bytes.len()>24*1024*1024{bail!("artifact超过24MiB")}
                let ext=artifact.rsplit('.').next().unwrap_or("");
                if ["png","webp","jpg","jpeg"].contains(&ext) {
                    let mut shown=artifact.to_owned();
                    if let Some(crop)=args.get("crop") {
                        let format=image::guess_format(&bytes)?;
                        let reader=||image::ImageReader::with_format(std::io::Cursor::new(&bytes),format);
                        let (image_width,image_height)=reader().into_dimensions()?;
                        if u64::from(image_width)*u64::from(image_height)>16_777_216{bail!("图片像素超过16M保护值")}
                        let image=reader().decode()?;
                        let number=|key:&str|->Result<u32>{Ok(u32::try_from(crop[key].as_u64().context("裁剪需要非负整数")?)?)};
                        let (x,y,width,height)=(number("x")?,number("y")?,number("width")?,number("height")?);
                        if width==0||height==0||x.checked_add(width).is_none_or(|n|n>image.width())||y.checked_add(height).is_none_or(|n|n>image.height()){bail!("裁剪超出图片范围")}
                        let mut output=std::io::Cursor::new(Vec::new());image.crop_imm(x,y,width,height).write_to(&mut output,image::ImageFormat::Png)?;shown=workspace.artifact(output.get_ref(),"png")?;
                    }
                    return Ok(json!({"ok":true,"imageRole":"reinspection","sourceArtifact":artifact,"crop":args["crop"],"images":[{"view":"historical-reinspection","artifact":shown}],"note":"重读历史/参考图片；不增加当前场景截图或Goal复核次数，不是当前revision完成证据。"}));
                }
                if !["js","json","txt","md"].contains(&ext){bail!("不支持的artifact类型")}
                let text=String::from_utf8(bytes)?;let content=text.chars().take(64_000).collect::<String>();
                Ok(json!({"ok":true,"artifact":artifact,"content":content,"truncated":text.chars().count()>64_000}))
            });
        }
        if name == "recipe_sources" {
            if args["action"] == "run" {
                Self::validate_recipe_capture(args)?;
                let (code, source_hash) = {
                    let w = self.workspace.lock().unwrap();
                    crate::sources::program(&w, args)?
                };
                let request = RecipeRequest {
                    code,
                    seed: match args.get("seed") {
                        None => 1,
                        Some(value) => {
                            u32::try_from(value.as_u64().context("seed需要0至4294967295整数")?)
                                .context("seed超过uint32")?
                        }
                    },
                    mode: args["mode"].as_str().unwrap_or("apply").into(),
                    base_revision: Some(
                        args["baseRevision"]
                            .as_u64()
                            .context("run需要baseRevision")?,
                    ),
                    request_id: args["requestId"].as_str().map(str::to_owned),
                    max_ops: 100_000,
                    dry_run: match args.get("dryRun") {
                        None => false,
                        Some(value) => value.as_bool().context("dryRun必须是布尔值")?,
                    },
                };
                let mut result = self
                    .run_recipe_with_guard(request, run, guard.as_ref())
                    .await?;
                result["sourceHash"] = json!(source_hash);
                return self.capture_after_recipe(result, args, run, guard).await;
            }
            return self.with_workspace_guard(guard.as_ref(), |workspace| {
                crate::sources::action(workspace, args, run)
            });
        }
        if name == "minecraft_import" {
            if guard.as_ref().is_some_and(|g| !self.guard_is_current(g)) {
                bail!("RUN_CANCELLED");
            }
            match args["action"].as_str().unwrap_or("list") {
                "preview" => {
                    let result = crate::minecraft::imports::preview(
                        self.workspace.clone(),
                        args.clone(),
                        run.map(str::to_owned),
                    )
                    .await?;
                    if guard.as_ref().is_some_and(|g| !self.guard_is_current(g)) {
                        bail!("RUN_CANCELLED");
                    }
                    return Ok(result);
                }
                "list" | "inspect" => {
                    return self.with_workspace_guard(guard.as_ref(), |w| {
                        if args["action"] == "inspect" {
                            crate::minecraft::imports::inspect(&w.root, args)
                        } else {
                            crate::minecraft::imports::list(&w.root)
                        }
                    })
                }
                "apply" => {
                    return self.with_workspace_guard(guard.as_ref(), |w| {
                        let mut result = crate::minecraft::imports::apply(w, args, run)?;
                        if result["replayed"] != true
                            && self.transport.config().enabled
                            && self.transport.config().auto
                        {
                            w.commit(None, "AI Minecraft建筑导入", run)?;
                            if let Err(e) = self.transport.publish_with_materials(
                                &w.project.scene,
                                &w.project.materials,
                                false,
                            ) {
                                result["publishWarning"] = json!(e.to_string());
                            }
                        }
                        Ok(result)
                    })
                }
                _ => bail!("未知Minecraft导入操作"),
            }
        }
        match name {
            "get_context" => Ok(self.context_with_options(args)),
            "components" | "reference_workbench" | "animation" => {
                if name == "reference_workbench" && args["action"] == "capture" {
                    return self.capture_views_tool(args,run,guard).await;
                }
                self.with_workspace_guard(guard.as_ref(), |w| {
                    let domain=match name {"reference_workbench"=>"references",other=>other};
                    let mut result=crate::creative::action(w,domain,args,run)?;
                    if name=="components" && args["action"]=="place" && args["dryRun"]!=true && result["replayed"]!=true && self.transport.config().enabled && self.transport.config().auto {
                        if let Err(e)=self.transport.publish_with_materials(&w.project.scene,&w.project.materials,false){result["publishWarning"]=json!(e.to_string())}
                    }
                    Ok(result)
                })
            },
            "image_tools" => self.with_workspace_guard(guard.as_ref(), |w| {
                let mut result=crate::image_tools::action(w,args,run)?;
                if args["action"]=="voxel_plane" && result["dryRun"]==false && self.transport.config().enabled && self.transport.config().auto {
                    match self.transport.publish_with_materials(&w.project.scene,&w.project.materials,false){Ok(v)=>result["sync"]=v,Err(e)=>result["syncWarning"]=json!(e.to_string())}
                }
                Ok(result)
            }),
            "transform_scene" => self.with_workspace_guard(guard.as_ref(),|workspace|{
                let mut result=crate::scene_tools::transform(workspace,args,run)?;
                if args["dryRun"]!=true && result["unchanged"]!=true && self.transport.config().enabled && self.transport.config().auto{
                    match self.transport.publish_with_materials(&workspace.project.scene,&workspace.project.materials,false){Ok(v)=>result["sync"]=v,Err(e)=>result["syncWarning"]=json!(e.to_string())}
                }
                Ok(result)
            }),
            "builtin_materials" => self.with_workspace_guard(guard.as_ref(),|workspace|{
                if args["action"].as_str().unwrap_or("list")=="list"{return Ok(json!({"packs":crate::builtin_pixels::list()}));}
                if self.transport.config().enabled{bail!("加入主题前请停止OSC发送，避免Avatar目录不一致");}
                crate::builtin_pixels::apply(workspace,args,run)
            }),
            "define_materials" => self.with_workspace_guard(guard.as_ref(),|workspace|runtime_materials::define(workspace,args,run)),
            "define_palette" => self.with_workspace_guard(guard.as_ref(), |workspace| {
                palette::define(workspace, args, run)
            }),
            "run_recipe" => {
                Self::validate_recipe_capture(args)?;
                let request: RecipeRequest = serde_json::from_value(args.clone())?;
                let result=self.run_recipe_with_guard(request, run, guard.as_ref()).await?;
                self.capture_after_recipe(result,args,run,guard).await
            }
            "mesh_tools" => self.with_workspace_guard(guard.as_ref(), |workspace| {
                let mut result = crate::mesh_tools::execute(workspace, args, run)?;
                if args["dryRun"] != true && args["action"] != "inspect" && self.transport.config().enabled && self.transport.config().auto {
                    workspace.commit(None, "AI网格增量", run)?;
                    if let Err(error) = self.transport.publish_with_materials(&workspace.project.scene, &workspace.project.materials, false) { result["publishWarning"] = json!(error.to_string()); }
                }
                Ok(result)
            }),
            "voxel_text" => self.voxel_text(args, run, guard.as_ref()).await,
            "capture_views" => self.capture_views_tool(args,run,guard).await,
            "inspect_region" => {
                let workspace = self.workspace.lock().unwrap();
                let mut scene = workspace.project.scene.clone();
                if let (Some(min), Some(size)) = (args.get("min"), args.get("size")) {
                    let min = serde_json::from_value(min.clone())?;
                    let size = serde_json::from_value(size.clone())?;
                    let region = morphforge_core::Box3::new(min, size, 0);
                    region.validate(&workspace.project.limits)?;
                    scene.boxes.retain(|box3| box3.intersects(&region));
                    scene.triangles.retain(|t| t.intersects(&region));
                }
                if let Some(group) = args["group"].as_str() {scene.boxes.retain(|box3|box3.group==group);scene.triangles.retain(|t|t.group==group);}
                if let Some(slice)=args.get("slice") {
                    let axis=match slice["axis"].as_str(){Some("x")=>0,Some("y")=>1,Some("z")=>2,_=>bail!("slice轴需要x/y/z")};
                    let index=i32::try_from(slice["index"].as_i64().context("slice需要index")?).context("slice坐标越界")?;
                    scene.boxes.retain_mut(|box3|{if index>=box3.min[axis]&&index<box3.end()[axis]{box3.min[axis]=index;box3.size[axis]=1;true}else{false}});
                    scene.triangles.retain(|t|{let b=t.bounds();b.min[axis]<=index&&b.max[axis]>=index});
                }
                let offset=args["offset"].as_u64().unwrap_or(0).min(usize::MAX as u64) as usize;
                let limit=args["limit"].as_u64().unwrap_or(20).clamp(1,128) as usize;
                Ok(json!({
                    "ok":true,
                    "revision":scene.revision,
                    "stats":scene.stats(&workspace.project.limits),
                    "sample":scene.boxes.iter().skip(offset).take(limit).collect::<Vec<_>>(),
                    "triangles":scene.triangles.iter().skip(offset).take(limit).collect::<Vec<_>>(),
                    "trianglesNextOffset":if offset.saturating_add(limit)<scene.triangles.len(){Some(offset+limit)}else{None},
                    "offset":offset,"limit":limit,"nextOffset":if offset.saturating_add(limit)<scene.boxes.len(){Some(offset+limit)}else{None},
                    "statsScope":if args.get("slice").is_some(){"one_voxel_slice"}else{"intersecting_boxes"},
                }))
            }
            "commit" => self.with_workspace_guard(guard.as_ref(), |workspace| {
                let commit = workspace.commit(
                    args["revision"].as_u64(),
                    args["title"].as_str().unwrap_or("AI提交"),
                    run,
                )?;
                Ok(json!({"ok":true,"commit":commit}))
            }),
            "publish" => self.with_workspace_guard(guard.as_ref(), |workspace| {
                let revision = args["revision"]
                    .as_u64()
                    .unwrap_or(workspace.project.scene.revision);
                if !workspace.project.commits.iter().any(|commit| commit.revision == revision) {
                    bail!("请先commit再publish")
                }
                if args["repair"]==true && args["resync"]==true{bail!("补播不可与清空重建同时使用");}
                if let Some(order)=args.get("sendOrder"){if !["volume","bottom_up","material"].iter().any(|s|order==*s){bail!("sendOrder无效");}}
                let scene = workspace.scene_at(revision)?;
                let materials=workspace.materials_at(revision)?;
                let mut state = if args["repair"]==true {
                    self.transport.publish_for_repair(&scene,&materials)?
                } else {
                    self.transport.publish_with_materials(&scene,&materials,args["resync"]==true)?
                };
                let mut options=serde_json::Map::new();for key in ["loopReplay","sendOrder"]{if let Some(value)=args.get(key){options.insert(key.into(),value.clone());}}
                if !options.is_empty(){state=self.transport.configure(&Value::Object(options),workspace)?;}
                workspace.event(
                    "revision.published",
                    run,
                    json!({"revision":revision,"state":state}),
                )?;
                Ok(state)
            }),
            "sync_status" => Ok(self.transport.state()),
            "undo_to" => self.with_workspace_guard(guard.as_ref(), |workspace| {
                workspace.undo(false, Some(args["revision"].as_u64().context("缺少revision")?))
            }),
            "artworks" => self.with_workspace_guard(guard.as_ref(), |workspace| {
                match args["action"].as_str().unwrap_or("search") {
                    "search" => {
                        let query = args["query"].as_str().unwrap_or("").to_lowercase();
                        let matches=|artwork:&&crate::store::Artwork|artwork.title.to_lowercase().contains(&query)||artwork.tags.iter().any(|tag|tag.to_lowercase().contains(&query));
                        Ok(json!({"ok":true,"artworks":workspace.library()?.iter().filter(matches).collect::<Vec<_>>(),"examples":crate::examples::list(workspace)?.iter().filter(matches).collect::<Vec<_>>()}))
                    }
                    "save" => {
                        let artwork = save_artwork_from_args(workspace, args, run)?;
                        Ok(json!({"ok":true,"artwork":artwork}))
                    }
                    _ => bail!("未知作品库操作"),
                }
            }),
            "finish_artwork" => self.finish_artwork(args, run, guard.as_ref()),
            "compact_context" => {
                let mut state = self.inner.lock().unwrap();
                if let Some(ref guard) = guard {
                    if !guard_matches_locked(&state, guard) {
                        bail!("RUN_CANCELLED")
                    }
                }
                state.compact_requested = true;
                Ok(json!({"ok":true,"scheduled":true,"note":"在完整工具批次结束后整理上下文，保留原始日志与作品"}))
            }
            _ => bail!("未知工具 {name}"),
        }
    }

    async fn capture_views_tool(
        &self,
        args: &Value,
        run: Option<&str>,
        guard: Option<RunGuard>,
    ) -> Result<Value> {
        let resolved = {
            let w = self.workspace.lock().unwrap();
            crate::creative::resolve_capture(&w, args)?
        };
        let args = &resolved;
        let (project_id, signature) = {
            let workspace = self.workspace.lock().unwrap();
            (
                workspace.project.id.clone(),
                goal::geometry_hash(&workspace),
            )
        };
        let request = self.capture.request(self.workspace.clone(), args, run);
        let result = if let Some(guard) = guard {
            tokio::select! {
                biased;
                result = request => result,
                _ = guard.cancel.wait() => bail!("RUN_CANCELLED"),
                _ = tokio::time::sleep_until(guard.deadline) => {
                    guard.cancel.stop(); bail!("RUN_TIME_BUDGET：截图超过运行时限")
                }
            }
        } else {
            request.await
        }?;
        let matches = {
            let workspace = self.workspace.lock().unwrap();
            workspace.project.id == project_id
                && goal::geometry_hash(&workspace) == signature
                && result["revision"].as_u64() == Some(workspace.project.scene.revision)
        };
        if matches && args.get("animation").is_none() && result["inspectionOnly"]!=true {
            self.record_goal_capture(&project_id, signature, &result)?;
        }
        Ok(result)
    }
    async fn capture_after_recipe(
        &self,
        mut result: Value,
        args: &Value,
        run: Option<&str>,
        guard: Option<RunGuard>,
    ) -> Result<Value> {
        if let Some(options) = args.get("capture") {
            if args["dryRun"] == true {
                result["captureSkipped"] = json!("dryRun");
                return Ok(result);
            }
            let mut options = options.clone();
            options["revision"] = result["revision"].clone();
            match self.capture_views_tool(&options, run, guard).await {
                Ok(capture) => {
                    result["images"] = capture["images"].clone();
                    result["captureId"] = capture["id"].clone();
                }
                Err(error) if error.to_string().starts_with("RUN_") => return Err(error),
                Err(error) => {
                    result["geometryCommitted"] = json!(true);
                    result["captureError"] = json!(self.provider.redact(&error.to_string()));
                    result["captureRecovery"] = json!("绘制已经提交；只重试截图，不要重复绘制");
                }
            }
        }
        Ok(result)
    }
    fn validate_recipe_capture(args: &Value) -> Result<()> {
        if let Some(capture) = args.get("capture") {
            if !capture.is_object() {
                bail!("capture必须是对象")
            };
            crate::capture::view_options(capture)?;
            crate::capture::render_options(capture)?;
        }
        Ok(())
    }
    fn with_workspace_guard<T>(
        &self,
        guard: Option<&RunGuard>,
        operation: impl FnOnce(&mut Workspace) -> Result<T>,
    ) -> Result<T> {
        if let Some(guard) = guard {
            let state = self.inner.lock().unwrap();
            if !guard_matches_locked(&state, guard) {
                bail!("RUN_CANCELLED")
            }
            let mut workspace = self.workspace.lock().unwrap();
            operation(&mut workspace)
        } else {
            operation(&mut self.workspace.lock().unwrap())
        }
    }

    fn finish_artwork(
        &self,
        args: &Value,
        run: Option<&str>,
        guard: Option<&RunGuard>,
    ) -> Result<Value> {
        let title = args["title"]
            .as_str()
            .filter(|value| !value.trim().is_empty() && value.len() <= 1024)
            .context("finish_artwork需要不超过1024字节的title")?;
        let summary = args["summary"]
            .as_str()
            .filter(|value| !value.trim().is_empty() && value.len() <= 16_000)
            .context("finish_artwork需要不超过16000字节的summary")?;

        if let Some(guard) = guard {
            let mut state = self.inner.lock().unwrap();
            if !guard_matches_locked(&state, guard) {
                bail!("RUN_CANCELLED")
            }
            let mut workspace = self.workspace.lock().unwrap();
            if workspace.project.auto_center_on_finish {
                let base = workspace.project.scene.revision;
                let centered = crate::scene_tools::transform(
                    &mut workspace,
                    &json!({"action":"center_on_ground","baseRevision":base}),
                    run,
                )?;
                if centered["unchanged"] != true {
                    bail!("CENTERED_NEEDS_REVIEW：完成前已落地居中到revision {}，请截图并在下一响应看图后再完成；源码未改写。",workspace.project.scene.revision);
                }
            }
            goal::check_goal_finish(&state, &workspace, args)?;
            if state.settings.send_images {
                if let Some(images) = latest_capture_images(&workspace) {
                    let artifacts = images
                        .iter()
                        .map(|(_, artifact)| artifact.as_str())
                        .collect::<HashSet<_>>();
                    fn contains_image(value: &Value, artifacts: &HashSet<&str>) -> bool {
                        if value["type"] == "input_image"
                            && value["artifact"]
                                .as_str()
                                .is_some_and(|a| artifacts.contains(a))
                        {
                            return true;
                        }
                        match value {
                            Value::Array(a) => a.iter().any(|v| contains_image(v, artifacts)),
                            Value::Object(o) => o.values().any(|v| contains_image(v, artifacts)),
                            _ => false,
                        }
                    }
                    let sent = state
                        .conversation
                        .input
                        .iter()
                        .take(state.conversation.requested_input_length)
                        .any(|item| contains_image(item, &artifacts));
                    if !sent {
                        bail!("FINISH_REVIEW_REQUIRED：当前截图尚未发送给模型；先在下一响应看图，再finish_artwork")
                    }
                }
            }

            workspace.event(
                "artwork.reference_sources",
                run,
                json!({"references":state.conversation.reference_images}),
            )?;
            let result = finish_artwork_in_workspace(&mut workspace, args, title, summary, run, state.settings.enforce_target_limits, self.transport.config().capacity)?;
            if state.settings.goal_mode {
                if let Some(goal) = state.conversation.goal.as_mut() {
                    goal.status = "achieved".into();
                    goal.assessment = args["goalAssessment"].as_str().unwrap_or("").into();
                    workspace.event("goal.achieved", run, json!(&goal))?;
                }
            }
            let run_state = state
                .run
                .as_mut()
                .filter(|run_state| run_state.id == guard.id && run_state.epoch == guard.epoch)
                .context("RUN_CANCELLED")?;
            run_state.message = summary.to_owned();
            Ok(result)
        } else {
            let mut state = self.inner.lock().unwrap();
            let mut workspace = self.workspace.lock().unwrap();
            if workspace.project.auto_center_on_finish {
                let base = workspace.project.scene.revision;
                let centered = crate::scene_tools::transform(
                    &mut workspace,
                    &json!({"action":"center_on_ground","baseRevision":base}),
                    run,
                )?;
                if centered["unchanged"] != true {
                    bail!("CENTERED_NEEDS_REVIEW：完成前已落地居中到revision {}，请截图并在下一响应看图后再完成；源码未改写。",workspace.project.scene.revision);
                }
            }
            goal::check_goal_finish(&state, &workspace, args)?;
            workspace.event(
                "artwork.reference_sources",
                run,
                json!({"references":state.conversation.reference_images}),
            )?;
            let result = finish_artwork_in_workspace(&mut workspace, args, title, summary, run, state.settings.enforce_target_limits, self.transport.config().capacity)?;
            if state.settings.goal_mode {
                if let Some(goal) = state.conversation.goal.as_mut() {
                    goal.status = "achieved".into();
                    goal.assessment = args["goalAssessment"].as_str().unwrap_or("").into();
                    workspace.event("goal.achieved", run, json!(&goal))?;
                }
            }
            drop(workspace);
            drop(state);
            self.persist()?;
            Ok(result)
        }
    }

    pub async fn run_recipe(&self, request: RecipeRequest, run: Option<&str>) -> Result<Value> {
        self.run_recipe_with_guard(request, run, None).await
    }

    async fn run_recipe_with_guard(
        &self,
        request: RecipeRequest,
        run: Option<&str>,
        guard: Option<&RunGuard>,
    ) -> Result<Value> {
        if let Some(request_id) = request.request_id.as_deref() {
            if let Some(value) = self
                .workspace
                .lock()
                .unwrap()
                .requests
                .get(request_id)
                .cloned()
            {
                return Ok(value);
            }
        }
        if !["apply", "replace"].contains(&request.mode.as_str()) {
            bail!("mode只支持apply/replace")
        };
        if let Some(guard) = guard {
            if !self.guard_is_current(guard) {
                bail!("RUN_CANCELLED")
            }
        }
        let (base, styles) = {
            let workspace = self.workspace.lock().unwrap();
            let base = request
                .base_revision
                .unwrap_or(workspace.project.scene.revision);
            if base != workspace.project.scene.revision {
                bail!("REVISION_CONFLICT")
            };
            let mut styles = BTreeMap::new();
            for material in &workspace.project.materials {
                styles.insert(material.uid.clone(), material.id);
                styles.insert(material.uid.trim_start_matches("mf:").into(), material.id);
                styles.insert(material.name.clone(), material.id);
            }
            (base, styles)
        };
        let mut operations = recipe::execute(request.clone(), styles).await?;
        if request.mode == "replace" {
            operations.insert(0, morphforge_core::Operation::Clear)
        }

        let apply = |workspace: &mut Workspace| -> Result<Value> {
            if request.dry_run {
                let mut scene = workspace
                    .project
                    .scene
                    .apply(&operations, &workspace.project.limits)?;
                scene.revision = base;
                return Ok(
                    json!({"ok":true,"dryRun":true,"revision":base,"stats":scene.stats(&workspace.project.limits)}),
                );
            }
            let value =
                workspace.apply(&operations, Some(base), request.request_id.as_deref(), run)?;
            let revision = workspace.project.scene.revision;
            workspace.recipes.insert(revision, request.code.clone());
            let artifact = workspace.artifact(request.code.as_bytes(), "js")?;
            workspace.event(
                "recipe.completed",
                run,
                json!({
                        "seed":request.seed,
                        "mode":request.mode,
                        "creative":workspace.project.creative.brief(),
                "sdkVersion":recipe::SDK_VERSION,
                        "artifact":artifact,
                        "result":value,
                    }),
            )?;
            if self.transport.config().enabled && self.transport.config().auto {
                let _ = workspace.commit(None, "自动发布", run)?;
                let state = self.transport.publish_with_materials(
                    &workspace.project.scene,
                    &workspace.project.materials,
                    false,
                )?;
                workspace.event("revision.published", run, state)?;
            }
            Ok(value)
        };

        if let Some(guard) = guard {
            // Hold the run-state lock through the synchronous workspace commit. An
            // interrupt/stop therefore linearizes either before the commit (blocked)
            // or after its durable result (committed), never between the guard and write.
            let state = self.inner.lock().unwrap();
            if !guard_matches_locked(&state, guard) {
                bail!("RUN_CANCELLED")
            }
            let mut workspace = self.workspace.lock().unwrap();
            apply(&mut workspace)
        } else {
            apply(&mut self.workspace.lock().unwrap())
        }
    }

    async fn voxel_text(
        &self,
        args: &Value,
        run: Option<&str>,
        guard: Option<&RunGuard>,
    ) -> Result<Value> {
        let mode = args["mode"].as_str().unwrap_or("preview");
        let fonts = self
            .fonts
            .clone()
            .map(Ok)
            .unwrap_or_else(crate::fonts::shared_default)?;
        if mode == "fonts" {
            let query = args["query"].as_str().unwrap_or("").to_lowercase();
            let offset = args["offset"].as_u64().unwrap_or(0) as usize;
            let limit = args["limit"].as_u64().unwrap_or(32).clamp(1, 64) as usize;
            let matching = fonts
                .list()
                .into_iter()
                .filter(|font| {
                    query.is_empty()
                        || format!("{} {} {}", font.family, font.style, font.id)
                            .to_lowercase()
                            .contains(&query)
                })
                .collect::<Vec<_>>();
            let values = matching
                .iter()
                .skip(offset)
                .take(limit)
                .map(|font| {
                    json!({
                        "id":font.id,
                        "family":font.family,
                        "style":font.style,
                        "license":font.license,
                    })
                })
                .collect::<Vec<_>>();
            return Ok(json!({
                "ok":true,
                "mode":"fonts",
                "total":matching.len(),
                "offset":offset,
                "limit":limit,
                "nextOffset":if offset.saturating_add(limit)<matching.len(){Some(offset+limit)}else{None},
                "fonts":values,
            }));
        }
        if !["preview", "apply"].contains(&mode) {
            bail!("voxel_text mode只支持fonts/preview/apply")
        }
        let mut request_value = args.clone();
        if let Some(object) = request_value.as_object_mut() {
            for key in [
                "mode",
                "baseRevision",
                "requestId",
                "previewFingerprint",
                "query",
                "offset",
                "limit",
            ] {
                object.remove(key);
            }
        }
        let request: morphforge_core::text::VoxelTextRequest =
            serde_json::from_value(request_value)?;
        let fingerprint = crate::fonts::request_fingerprint(&request)?;
        if mode == "apply" {
            let _base_revision = args["baseRevision"]
                .as_u64()
                .context("apply缺少baseRevision")?;
            let request_id = args["requestId"]
                .as_str()
                .filter(|value| !value.trim().is_empty() && value.len() <= 256)
                .context("apply缺少不超过256字节的requestId")?;
            if args["previewFingerprint"].as_str() != Some(fingerprint.as_str()) {
                bail!("PREVIEW_CONFLICT：文字设置已改变，请重新预览")
            }
            if let Some(guard) = guard {
                if !self.guard_is_current(guard) {
                    bail!("RUN_CANCELLED")
                }
            }
            if let Some(receipt) = self.with_workspace_guard(guard, |workspace| {
                workspace.text_receipt(request_id, &fingerprint)
            })? {
                return Ok(json!({
                    "ok":true,
                    "mode":"apply",
                    "replayed":true,
                    "result":receipt,
                }));
            }
        }

        if !self
            .workspace
            .lock()
            .unwrap()
            .project
            .materials
            .iter()
            .any(|material| material.id == request.style_id)
        {
            bail!(
                "未知材质编号{}，请从get_context材质目录选择",
                request.style_id
            )
        }
        if let Some(guard) = guard {
            if !self.guard_is_current(guard) {
                bail!("RUN_CANCELLED")
            }
        }
        let catalog = fonts.clone();
        let preview_request = request.clone();
        let preview = tokio::task::spawn_blocking(move || catalog.preview(&preview_request))
            .await
            .context("文字排版任务异常")??;
        let preview_summary = json!({
            "requestFingerprint":preview.request_fingerprint,
            "font":{
                "id":preview.font.id,
                "family":preview.font.family,
                "style":preview.font.style,
                "source":preview.font.source,
                "sha256":preview.font.sha256,
                "faceIndex":preview.font.face_index,
                "license":preview.font.license,
            },
            "layout":preview.layout,
            "warnings":preview.warnings,
        });
        if mode == "preview" {
            let revision = self.workspace.lock().unwrap().project.scene.revision;
            let mut apply_template = serde_json::to_value(&request)?;
            apply_template["mode"] = json!("apply");
            apply_template["baseRevision"] = json!(revision);
            apply_template["previewFingerprint"] = json!(fingerprint);
            return Ok(json!({
                "ok":true,
                "mode":"preview",
                "baseRevision":revision,
                "preview":preview_summary,
                "previewFingerprint":fingerprint,
                "applyTemplate":apply_template,
                "stats":preview.layout,
            }));
        }

        let base_revision = args["baseRevision"]
            .as_u64()
            .context("apply缺少baseRevision")?;
        let request_id = args["requestId"].as_str().context("apply缺少requestId")?;
        let font = serde_json::to_value(&preview.font)?;
        let layout = serde_json::to_value(&preview.layout)?;
        let result = self.with_workspace_guard(guard, |workspace| {
            workspace.apply_text(
                &preview.operations,
                base_revision,
                request_id,
                &fingerprint,
                &font,
                &layout,
                run,
            )
        })?;
        Ok(json!({
            "ok":true,
            "mode":"apply",
            "preview":preview_summary,
            "previewFingerprint":fingerprint,
            "result":result,
        }))
    }

    async fn compact_with_guard(&self, guard: &RunGuard, force: bool) -> Result<()> {
        if !self.guard_is_current(guard) {
            bail!("RUN_CANCELLED")
        }
        let (conversation, settings) = {
            let mut state = self.inner.lock().unwrap();
            if !guard_matches_locked(&state, guard) {
                bail!("RUN_CANCELLED")
            }
            state.compact_requested = false;
            (state.conversation.clone(), state.settings.clone())
        };
        let before = latest_provider_input_tokens(&self.workspace.lock().unwrap(), &guard.id)
            .unwrap_or_default() as usize;
        if before.max(estimate(&conversation.input)) < 500 && !force {
            return Ok(());
        }
        let cut = context::compact_cut(&conversation.boundaries, conversation.input.len());
        let source = &conversation.input[..cut];
        let tail = conversation.input[cut..].to_vec();
        let (pinned, revision, project_id) = {
            let workspace = self.workspace.lock().unwrap();
            (
                workspace.project.pinned_notes.clone(),
                workspace.project.scene.revision,
                workspace.project.id.clone(),
            )
        };
        let result: Result<()> = async {
            self.workspace.lock().unwrap().event(
                "compact.started",
                Some(&guard.id),
                json!({
                    "beforeTokens":before,
                    "messages":conversation.input.len(),
                    "sourceMessages":cut,
                    "revision":revision,
                    "epoch":guard.epoch,
                    "promptVersion":SYSTEM_PROMPT_VERSION,
                    "language":settings.language,
                }),
            )?;
            let compact_prompt = if settings.language == "en" {
                "Summarize this voxel-art session for continuity. Keep the user's requirements, visual constraints, decisions, failed approaches, open tasks, and artifact references. Include pinned requirements verbatim. Do not claim work is complete unless the history proves it. Current geometry and sync state must be re-read from tools. Return a concise but complete handoff."
            } else {
                "整理这段体素艺术会话的连续工作记忆。保留用户要求、美术约束、关键决策、失败教训、未完成事项和作品引用，并逐字保留固定要求。不要声称未完成的工作已经完成；当前几何与同步状态必须重新调用工具读取。返回简洁但完整的交接摘要。"
            };
            let prompt = format!(
                "{compact_prompt}\n固定要求：{}\n用户目标：{}\n此前摘要：{}\n历史：{}",
                pinned,
                serde_json::to_string(&conversation.goals)?,
                conversation.summary,
                serde_json::to_string(source)?
            );
            let request_id = uuid::Uuid::new_v4().to_string();
            {
                let mut state = self.inner.lock().unwrap();
                if !guard_matches_locked(&state, guard) {
                    bail!("RUN_CANCELLED")
                }
                state.active_request_id = Some(request_id.clone());
                if let Some(run) = state.run.as_mut().filter(|run| run.id == guard.id) {
                    run.active_request_id = Some(request_id.clone());
                }
            }
            let remaining_tokens = {
                let state = self.inner.lock().unwrap();
                settings.max_run_tokens.saturating_sub(state.run.as_ref().map(|r|r.tokens).unwrap_or(0))
            };
            if remaining_tokens == 0 { bail!("RUN_BUDGET：compact前已无Token预算") }
            let model_limits=self.provider.model_limits(&settings.model);
            let compact_input_estimate=estimate(&[json!({"role":"user","content":prompt})]).saturating_add(prompts::system_prompt(&settings.language).len()/3).saturating_add(2048);
            let compact_run_budget=if self.provider.cache_required(&settings.cache_guard){remaining_tokens.saturating_sub(compact_input_estimate as u64)}else{remaining_tokens};
            if compact_run_budget<512 {bail!("RUN_BUDGET：剩余Token不足以预留compact输入，未发起摘要请求")}
            let compact_output_cap = model_limits.output(settings.max_output_tokens).min(4096).min(compact_run_budget.min(u32::MAX as u64) as u32);
            let payload = json!({
                "model":settings.model,
                "instructions":format!("{}\n{}",prompts::system_prompt(&settings.language),compact_prompt),
                "input":[{"role":"user","content":prompt}],
                "reasoning":{"effort":"none"},
                "max_output_tokens":compact_output_cap,
                "stream":true
            });
            let trace = ProviderRequestTrace {
                request_id: Some(request_id.clone()),
                prompt_version: Some(SYSTEM_PROMPT_VERSION.into()),
                tools_version: Some(TOOL_SCHEMA_VERSION.into()),
                language: Some(settings.language.clone()),
                epoch: Some(guard.epoch),
                context_epoch: Some(conversation.compact_count),
                image_policy: Some(settings.send_images),
                compact: true,
                cache_guard: settings.cache_guard.clone(),
                strict_append_only: Some(settings.strict_append_only),
                estimated_input_tokens: compact_input_estimate as u64,
            };
            let compact_estimate = estimate(std::slice::from_ref(&payload));
            if compact_estimate.saturating_add(compact_output_cap as usize) > model_limits.context(settings.context_window_tokens) {
                bail!("CONTEXT_BUDGET：compact输入超过配置窗口，请提高窗口或重置上下文；原始历史仍保留")
            }
            let response_result = tokio::select! {
                biased;
                result = self.provider.response_with_trace(
                    payload,
                    self.workspace.clone(),
                    &guard.id,
                    &guard.cancel,
                    trace,
                ) => result,
                _ = tokio::time::sleep_until(guard.deadline) => {
                    guard.cancel.stop();
                    Err(anyhow::anyhow!("RUN_TIME_BUDGET：compact超过运行时限"))
                }
            };
            let response = response_result?;
            self.clear_active_request(guard, &request_id);
            record_run_tokens(
                &mut self.inner.lock().unwrap(),
                &guard.id,
                response["usage"]["total_tokens"].as_u64().unwrap_or_else(||compact_estimate.saturating_add(estimate(std::slice::from_ref(&response))) as u64),
            );
            if response["status"] != "completed" {
                bail!("RESPONSE_INCOMPLETE：compact响应未完整完成，旧上下文保留")
            }
            let summary = response["output"]
                .as_array()
                .context("compact没有output")?
                .iter()
                .filter(|item| item["type"] == "message")
                .flat_map(|item| item["content"].as_array().into_iter().flatten())
                .filter(|part| part["type"] == "output_text")
                .filter_map(|part| part["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n");
            if summary.trim().is_empty() {
                bail!("compact返回空摘要，旧上下文保留")
            }
            let manifest = json!({
                "type":"MorphForgeContextCompact/1",
                "projectId":project_id,
                "revisionAtCompact":revision,
                "pinnedRequirements":pinned,
                "userGoals":conversation.goals,
                "creationGoal":conversation.goal,
                "referenceImages":conversation.reference_images,
                "summary":summary,
                "stateAuthority":"调用get_context重新读取当前工程/发布状态"
            });
            let mut input = vec![json!({"role":"user","content":serde_json::to_string(&manifest)?})];
            input.extend(tail);
            let visual_context = visual_context::retain_latest_views(&mut input);
            context::compact_references(&mut input);
            let after = estimate(&input);
            let (old, new) = {
                let mut workspace = self.workspace.lock().unwrap();
                let old = workspace.artifact(serde_json::to_string(&conversation)?.as_bytes(), "json")?;
                let new = workspace.artifact(serde_json::to_string(&input)?.as_bytes(), "json")?;
                (old, new)
            };
            {
                let mut state = self.inner.lock().unwrap();
                if !guard_matches_locked(&state, guard) {
                    bail!("RUN_CANCELLED")
                }
                if serde_json::to_value(&state.conversation)? != serde_json::to_value(&conversation)? {
                    bail!("COMPACT_CONFLICT：compact期间上下文已变化，旧上下文保留")
                }
                let previous = state.conversation.clone();
                state.conversation.input = input;
                state.conversation.summary = summary;
                state.conversation.reference_snapshot = None;
                state.conversation.runtime_context = Value::Null;
                state.conversation.requested_input_length = 0;
                state.conversation.context_usage = None;
                state.conversation.compact_count += 1;
                state.conversation.boundaries = vec![state.conversation.input.len()];
                let mut workspace = self.workspace.lock().unwrap();
                if let Err(error) = write_json(
                    &workspace.root.join("runs/conversation.json"),
                    &state.conversation,
                ) {
                    state.conversation = previous;
                    return Err(error)
                }
                workspace.event(
                    "compact.completed",
                    Some(&guard.id),
                    json!({
                        "beforeTokens":before,
                        "afterTokens":after,
                        "sourceMessages":cut,
                        "beforeArtifact":old,
                        "afterArtifact":new,
                        "manifest":manifest,
                        "epoch":guard.epoch,
                        "contextEpoch":state.conversation.compact_count,
                        "visualContext":visual_context,
                    }),
                )?;
            }
            Ok(())
        }
        .await;
        if let Err(error) = &result {
            let message = self.provider.redact(&error.to_string());
            if let Ok(mut workspace) = self.workspace.lock() {
                let _ = workspace.event(
                    "compact.failed",
                    Some(&guard.id),
                    json!({
                        "error":message,
                        "contextPreserved":true,
                        "beforeTokens":before,
                        "messages":conversation.input.len(),
                        "epoch":guard.epoch,
                    }),
                );
            }
        }
        result
    }
}

fn is_active_phase(phase: &str) -> bool {
    [
        "running",
        "compacting",
        "interrupting",
        "pausing",
        "paused",
        "stopping",
    ]
    .contains(&phase)
}

fn restore_message_state(
    events: &[crate::store::Event],
) -> (HashMap<String, MessageReceipt>, VecDeque<PendingPrompt>) {
    let mut receipts = HashMap::new();
    let mut pending = Vec::<PendingPrompt>::new();
    for event in events {
        if event.kind == "harness.message.accepted" {
            let Some(request_id) = event.data["requestId"].as_str() else {
                continue;
            };
            let run_id = event.data["runId"]
                .as_str()
                .or(event.run_id.as_deref())
                .unwrap_or("")
                .to_owned();
            let prompt = event.data["prompt"].as_str().unwrap_or("").to_owned();
            let mode = event.data["mode"].as_str().unwrap_or("queue").to_owned();
            let ack = event.data.get("ack").cloned().unwrap_or_else(|| {
                json!({
                    "ok":true,
                    "accepted":true,
                    "requestId":request_id,
                    "runId":run_id,
                    "mode":mode,
                    "epoch":event.data["epoch"],
                    "phase":event.data["phase"],
                    "queuePosition":event.data["queuePosition"],
                })
            });
            receipts.insert(
                request_id.to_owned(),
                MessageReceipt {
                    ack,
                    prompt: prompt.clone(),
                    mode: mode.clone(),
                    requested_run_id: event.data["requestedRunId"].as_str().map(str::to_owned),
                    reference_ids: serde_json::from_value(event.data["referenceIds"].clone())
                        .unwrap_or_default(),
                },
            );
            pending.retain(|message| message.request_id != request_id);
            pending.push(PendingPrompt {
                request_id: request_id.to_owned(),
                run_id,
                prompt,
                mode,
                accepted_at: event.data["ack"]
                    .get("acceptedAt")
                    .and_then(Value::as_str)
                    .unwrap_or(&event.time)
                    .to_owned(),
                accepted_epoch: event.data["epoch"].as_u64().unwrap_or(0),
                references: serde_json::from_value(event.data["references"].clone())
                    .unwrap_or_default(),
            });
        } else if [
            "harness.message.applied",
            "harness.message.cancelled",
            "harness.message.failed",
        ]
        .contains(&event.kind.as_str())
        {
            if let Some(request_id) = event.data["requestId"].as_str() {
                pending.retain(|message| message.request_id != request_id);
            }
        }
    }
    // A no-text interrupt is a pause action, not a user prompt to replay.
    pending.retain(|message| !(message.mode == "interrupt" && message.prompt.trim().is_empty()));
    (receipts, pending.into())
}

fn guard_matches_locked(state: &State, guard: &RunGuard) -> bool {
    !guard.cancel.stopped()
        && state
            .run
            .as_ref()
            .is_some_and(|run| run.id == guard.id && run.epoch == guard.epoch)
        && ["running", "compacting"].contains(&state.phase.as_str())
}

fn assistant_text(output: &[Value]) -> Option<String> {
    let text = output
        .iter()
        .filter(|item| item["type"] == "message")
        .flat_map(|item| item["content"].as_array().into_iter().flatten())
        .filter(|part| part["type"] == "output_text")
        .filter_map(|part| part["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    (!text.is_empty()).then_some(text)
}

fn validate_completed_output(output: &[Value]) -> Result<()> {
    let mut ids = HashSet::new();
    let calls = output
        .iter()
        .filter(|item| item["type"] == "function_call")
        .collect::<Vec<_>>();
    for item in &calls {
        let call_id = item["call_id"]
            .as_str()
            .filter(|id| !id.trim().is_empty())
            .context("完整响应中的工具调用缺少call_id；未执行任何工具")?;
        if item["name"].as_str().is_none_or(str::is_empty) {
            bail!("完整响应中的工具调用缺少name；未执行任何工具")
        }
        if !ids.insert(call_id.to_owned()) {
            bail!("完整响应包含重复call_id；未执行任何工具")
        }
    }
    let finish_calls = calls
        .iter()
        .enumerate()
        .filter(|(_, item)| item["name"] == "finish_artwork")
        .collect::<Vec<_>>();
    if finish_calls.len() > 1 {
        bail!("完整响应包含多个finish_artwork；未执行任何工具")
    }
    if let Some((index, _)) = finish_calls.first() {
        if *index + 1 != calls.len() {
            bail!("finish_artwork必须是完整工具批次最后一个工具调用；未执行任何工具")
        }
    }
    Ok(())
}

fn omit_images(input: &mut Vec<Value>) {
    fn visit(value: &mut Value) {
        match value {
            Value::Array(values) => {
                values.retain(|v| v["type"] != "input_image");
                for value in values {
                    visit(value);
                }
            }
            Value::Object(values) => {
                for value in values.values_mut() {
                    visit(value);
                }
            }
            _ => {}
        }
    }
    for value in input {
        visit(value);
    }
}

fn estimate(input: &[Value]) -> usize {
    fn cost(value: &Value) -> usize {
        if value["type"] == "input_image" {
            return 2048;
        }
        match value {
            Value::String(text) => text.len().div_ceil(3),
            Value::Array(items) => items.iter().map(cost).sum(),
            Value::Object(items) => items
                .iter()
                .map(|(key, value)| key.len().div_ceil(3) + cost(value) + 4)
                .sum(),
            _ => 1,
        }
    }
    input.iter().map(cost).sum()
}

fn latest_capture_images(workspace: &Workspace) -> Option<Vec<(String, String)>> {
    let revision = workspace.project.scene.revision;
    let event = workspace.events.iter().rev().find(|event| {
        event.kind == "capture.completed"
            && event.data["revision"].as_u64() == Some(revision)
            && event.data["ok"] == true
            && event.data["inspectionOnly"] != true
    })?;
    let images = event.data["images"].as_array()?;
    Some(
        images
            .iter()
            .filter_map(|image| {
                let view = image["view"].as_str()?.to_owned();
                let artifact = image["artifact"].as_str()?.to_owned();
                workspace
                    .root
                    .join("artifacts")
                    .join(&artifact)
                    .is_file()
                    .then_some((view, artifact))
            })
            .collect(),
    )
}

fn latest_provider_input_tokens(workspace: &Workspace, run_id: &str) -> Option<u64> {
    workspace
        .events
        .iter()
        .rev()
        .find(|event| event.kind == "provider.response" && event.run_id.as_deref() == Some(run_id))
        .and_then(|event| event.data["body"]["usage"]["input_tokens"].as_u64())
}

fn resolve_artwork_image(
    value: &str,
    capture_images: Option<&[(String, String)]>,
) -> Result<String> {
    if ["hero", "front", "top", "side", "detail"].contains(&value) {
        return capture_images
            .and_then(|images| {
                images
                    .iter()
                    .find(|(view, _)| view == value)
                    .map(|(_, artifact)| artifact.clone())
            })
            .with_context(|| {
                format!("ARTWORK_CAPTURE_MISSING：当前revision最新截图中没有{value}视角")
            });
    }
    Ok(value.to_owned())
}

fn save_artwork_from_args(
    workspace: &mut Workspace,
    args: &Value,
    run: Option<&str>,
) -> Result<Artwork> {
    let capture_images = latest_capture_images(workspace);
    let requested_views = match args.get("views") {
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .context("作品视角必须是字符串")
            })
            .collect::<Result<Vec<_>>>()?,
        None | Some(Value::Null) => Vec::new(),
        _ => bail!("作品views必须是字符串数组"),
    };
    let views = if requested_views.is_empty() {
        capture_images
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|(_, artifact)| artifact.clone())
            .collect()
    } else {
        requested_views
            .iter()
            .map(|view| resolve_artwork_image(view, capture_images.as_deref()))
            .collect::<Result<Vec<_>>>()?
    };
    let thumbnail = match args.get("thumbnail") {
        Some(Value::String(value)) => {
            Some(resolve_artwork_image(value, capture_images.as_deref())?)
        }
        None | Some(Value::Null) => capture_images.as_deref().and_then(|images| {
            images
                .iter()
                .find(|(view, _)| view == "hero")
                .or_else(|| images.first())
                .map(|(_, artifact)| artifact.clone())
        }),
        _ => bail!("作品thumbnail必须是字符串"),
    };
    let tags = match args.get("tags") {
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .context("作品tag必须是字符串")
            })
            .collect::<Result<Vec<_>>>()?,
        None | Some(Value::Null) => Vec::new(),
        _ => bail!("作品tags必须是字符串数组"),
    };
    workspace.save_artwork_views(
        args["title"].as_str().unwrap_or("新作品"),
        tags,
        thumbnail,
        views,
        run,
    )
}

fn finish_artwork_in_workspace(
    workspace: &mut Workspace,
    args: &Value,
    title: &str,
    summary: &str,
    run: Option<&str>,
    enforce_target: bool,
    capacity: usize,
) -> Result<Value> {
    if enforce_target {
        let target=crate::scene_tools::target_report(&workspace.project.scene,capacity);
        if target["fits"]!=true {bail!("TARGET_LIMIT：作品尚不满足VRChat空间/容量约束：{}；修正后重新截图再交付，或由用户在设置中明确选择不限制目标。",target);}
    }
    let revision = workspace.project.scene.revision;
    if latest_capture_images(workspace).is_none_or(|images| images.is_empty()) {
        bail!("FINISH_CAPTURE_REQUIRED：请先为当前revision完成至少一个真实截图")
    }
    let commit = workspace.commit(Some(revision), title, run)?;
    let artwork = save_artwork_from_args(workspace, args, run)?;
    Ok(json!({
        "ok":true,
        "finished":true,
        "revision":revision,
        "commit":commit,
        "artwork":artwork,
        "summary":summary,
    }))
}

fn record_run_tokens(state: &mut State, id: &str, tokens: u64) {
    if let Some(run) = state.run.as_mut().filter(|r| r.id == id) {
        run.tokens = run.tokens.saturating_add(tokens);
    }
}
pub fn tools() -> Value {
    tools_for_language("zh-CN")
}

pub fn tools_for_language(language: &str) -> Value {
    fn tool(name: &str, description: &str, properties: Value, required: Vec<&str>) -> Value {
        json!({
            "type":"function",
            "name":name,
            "description":description,
            "parameters":{
                "type":"object",
                "properties":properties,
                "required":required,
                "additionalProperties":false
            }
        })
    }
    let en = language == "en";
    let descriptions = if en {
        [
            "Read the authoritative revision, project limits, target capabilities, pinned requirements, and a page of available materials.",
            "Run deterministic JavaScript voxel geometry as one transaction. Use compact functions and loops; apply to continue, replace only for a new scene. Include the exact baseRevision.",
            "Read a bounded region or group summary without returning the full box array.",
            "Capture real views of a revision. Combine views presets with shots for arbitrary angles, group/region framing and orthographic projection; at most five distinct names, custom shots override same-name presets. The result images list maps each view to its real artifact filename and URL; reuse those values when saving an artwork.",
            "Commit a reviewed draft revision.",
            "Queue an already committed revision for optional asynchronous VRChat publishing.",
            "Read local send progress; a local send is not a remote acknowledgement.",
            "Restore a historical revision as a new draft revision.",
            "Search, save, load, or fork an artwork. Save defaults to the latest capture of the current revision; views accepts captured view names such as hero/front/top or real artifact filenames, and thumbnail defaults to the captured hero or first image.",
            "Compact model context after a complete tool batch; keep raw logs, source artifacts, and artwork.",
            "List fonts, preview voxel text, or apply the exact preview at a guarded baseRevision. Preview returns a complete applyTemplate; reuse it without changing text layout fields. requestId is optional because the tool call ID is injected automatically. Use a material ID from get_context.",
            "Finish a reviewed artwork by committing the current revision and saving it with the latest real capture. Call this as the last function_call in the batch. Its summary becomes the user-facing completion message, so no follow-up response is needed.",
        ]
    } else {
        [
            "读取权威 revision、工程限制、目标能力、固定要求和一页可用材质。",
            "把确定性 JavaScript 体素几何作为一个事务执行。用紧凑函数和循环；apply 用于续作，replace 仅用于新场景，并传准确 baseRevision。",
            "读取局部区域或分组摘要，不返回完整 Box 数组。",
            "对指定revision捕获真实渲染。views用预设视角，shots用自由/局部/正交取景；可组合，最多5个不同名字，同名shots覆盖预设。结果 images 会把每个视角映射到真实 artifact 文件名和 URL；保存作品时复用返回值。",
            "提交已检查的草稿 revision。",
            "把已提交 revision 放入可选的异步 VRChat 发布队列。",
            "读取本机发送进度；本机发送不等于远端 ACK。",
            "把历史 revision 恢复为新的草稿 revision。",
            "搜索、保存、载入或分支作品。save 默认使用当前 revision 最新截图的真实图片；views 可传 hero/front/top 等截图视角名或真实 artifact 文件名，thumbnail 默认选 hero 或第一张截图。",
            "在完整工具批次后整理模型上下文；保留原始日志、源 artifact 和作品。",
            "列字体、预览或应用体素文字。preview 会返回完整 applyTemplate；apply 时原样复用，不要改文字布局字段。requestId 可省略，工具会自动注入 tool_call_id。材质使用 get_context 返回的 ID。",
            "完成已检查的作品：提交当前 revision 并用最新真实截图保存到作品库。必须作为本批次最后一个 function_call；summary 会直接展示给用户，不需要再调用模型生成结束语。",
        ]
    };
    let text_properties = json!({
        "mode":{"type":"string","enum":["fonts","preview","apply"]},
        "query":{"type":"string"},
        "offset":{"type":"integer","minimum":0},
        "limit":{"type":"integer","minimum":1,"maximum":64},
        "text":{"type":"string"},
        "fontId":{"type":"string"},
        "fontSize":{"type":"integer","minimum":1,"maximum":512},
        "depth":{"type":"integer","minimum":1,"maximum":256},
        "origin":{"type":"array","items":{"type":"integer"},"minItems":3,"maxItems":3},
        "plane":{"type":"string","enum":["xy","xz","yz"]},
        "facing":{"type":"string","enum":["positive","negative"]},
        "align":{"type":"string","enum":["left","center","right"]},
        "letterSpacing":{"type":"integer"},
        "lineSpacing":{"type":"integer"},
        "maxWidth":{"type":"integer","minimum":1},
        "styleId":{"type":"integer","minimum":0},
        "tintId":{"type":"integer","minimum":0,"maximum":255},
        "motionPhase":{"type":"integer","minimum":0,"maximum":255},
        "group":{"type":"string"},
        "previewFingerprint":{"type":"string"},
        "baseRevision":{"type":"integer","minimum":0},
        "requestId":{"type":"string","description":"Optional; model tool calls inject their call_id automatically."}
    });
    let mut schemas = json!([
        tool(
            "get_context",
            descriptions[0],
            json!({
                "styleQuery":{"type":"string"},
                "styleOffset":{"type":"integer","minimum":0},
                "styleLimit":{"type":"integer","minimum":1,"maximum":128}
            }),
            vec![],
        ),
        tool(
            "minecraft_import",
            if en {
                "Import a locally uploaded Minecraft building: list sources, inspect paginated palette, preview compilation without changing the draft, then apply using previewId + baseRevision. Source files remain preserved. Formats: litematic, Sponge schem, legacy schematic, Java structure NBT. options.blockScale=0/1/2/4/8/16; default 0 automatically selects the smallest exact shape grid (1 for full cubes), capped to fit the 256-cell target when possible; any resulting thin-detail quantization is reported. Explicit higher values preserve thin structures. center uses bottom center; resourcePack=builtin or imported pack ID. Preview returns unknown/quantized states and capacity report; inspect this before applying. Apply is undoable and idempotent with requestId. Unsupported block shapes are approximated and reported. No Minecraft game logic."
            } else {
                "导入用户已上传的Minecraft建筑：list查看源库；inspect分页查看方块状态；preview编译并返回报告但不修改草稿；确认报告后用previewId和baseRevision调用apply，可撤销且requestId幂等。支持litematic、Sponge schem、旧schematic、Java结构NBT。options.blockScale=0/1/2/4/8/16，默认0按形状选最小精度（完整方块1:1），优先适配256格空间并报告被量化的薄细节；显式提高可保留薄结构；center地面居中；resourcePack=builtin或已导入资源包ID。先检查未知形状、量化、材质近似和容量报告，超限时调整精度或只保留外壳。保留原建筑，不模拟游戏逻辑。"
            },
            json!({
              "action":{"type":"string","enum":["list","inspect","preview","apply"]},
              "sourceId":{"type":"string"},"previewId":{"type":"string"},"requestId":{"type":"string"},"projectId":{"type":"string"},"baseRevision":{"type":"integer","minimum":0},
              "offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":128},
              "options":{"type":"object","properties":{
                "blockScale":{"type":"integer","enum":[0,1,2,4,8,16]},"center":{"type":"boolean"},"surfaceOnly":{"type":"boolean"},"resourcePack":{"type":"string"},"mode":{"type":"string","enum":["replace","overlay"]},"group":{"type":"string"},"offset":{"type":"array","items":{"type":"integer"},"minItems":3,"maxItems":3}
              },"additionalProperties":false}
            }),
            vec!["action"]
        ),
        tool(
            "mesh_tools",
            if en {
                "Inspect or edit RGB8 triangle faces using stable IDs; apply appends or updates faces, wedge creates a closed polygon slope; primitive=voxel with styleId creates one textured voxel slope. Geometry uses integer XYZ, per-face color and optional doubleSided, no UV. baseRevision required for edits; dryRun previews without changes. Region erase selects whole faces by AABB. For large indexed meshes prefer mf.mesh in run_recipe. Requires MF6 only when publishing to VRChat."
            } else {
                "查看或编辑逐面RGB8三角网格：inspect分页返回稳定ID；apply新增(id=0)或更新已有面；wedge默认生成封闭多边形斜面，primitive=voxel配合styleId生成单对象带材质体素斜面；remove/paint按ID删除或改色。整数XYZ、每面颜色、可选doubleSided，无UV。修改需baseRevision，dryRun只预览。区域擦除按AABB删整面。大型索引网格优先在run_recipe调用mf.mesh；发布到VRChat才要求MF6。"
            },
            json!({
                "action":{"type":"string","enum":["inspect","apply","wedge","remove","paint"]},
                "baseRevision":{"type":"integer","minimum":0},"dryRun":{"type":"boolean"},
                "group":{"type":"string","maxLength":128},"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":128},
                "mode":{"type":"string","enum":["append","replace_group","replace_scene"]},
                "ids":{"type":"array","maxItems":4096,"items":{"type":"integer","minimum":1}},
                "min":{"type":"array","items":{"type":"integer"},"minItems":3,"maxItems":3},
                "size":{"type":"array","items":{"type":"integer","minimum":1},"minItems":3,"maxItems":3},
                "color":{"type":"array","items":{"type":"integer","minimum":0,"maximum":255},"minItems":3,"maxItems":3},
                "primitive":{"type":"string","enum":["polygon","voxel"]},"styleId":{"type":"integer","minimum":0},
                "direction":{"type":"string","enum":["+x","-x","+z","-z"]},"doubleSided":{"type":"boolean"},
                "triangles":{"type":"array","maxItems":512,"items":{"type":"object","properties":{
                    "id":{"type":"integer","minimum":0},"vertices":{"type":"array","minItems":3,"maxItems":3,"items":{"type":"array","items":{"type":"integer"},"minItems":3,"maxItems":3}},
                    "color":{"type":"array","items":{"type":"integer","minimum":0,"maximum":255},"minItems":3,"maxItems":3},"group":{"type":"string","maxLength":128},"doubleSided":{"type":"boolean"}
                },"required":["vertices","color"],"additionalProperties":false}}
            }),
            vec!["action"]
        ),
        tool(
            "define_palette",
            if en {
                "Define 1–32 exact opaque sRGB colors (#RRGGBB) for reference matching. Reuse returned style IDs/UIDs and revision in geometry calls. Adds missing colors without changing existing materials or geometry. MF6 sends new colors directly as RGB24 without rebuilding the Avatar."
            } else {
                "按#RRGGBB（sRGB）定义1至32个不透明纯色，匹配参考图肤色、藏蓝等。返回材质ID/UID和最新revision，后续几何使用这些返回值；同色复用，不改已有材质或几何。MF6可直接实时发送新颜色，无需更新Avatar目录。"
            },
            json!({
                "baseRevision":{"type":"integer","minimum":0},
                "colors":{"type":"array","minItems":1,"maxItems":32,"items":{
                    "type":"object","properties":{
                        "name":{"type":"string","minLength":1,"maxLength":128},
                        "color":{"type":"string","pattern":"^#[0-9a-fA-F]{6}$"}
                    },"required":["name","color"],"additionalProperties":false
                }}
            }),
            vec!["baseRevision", "colors"],
        ),
        tool(
            "image_tools",
            if en {
                "Local reference-image tools: list; inspect exact RGBA8 pixels/compact sample grid; resize/crop to a new reference while keeping the original and attribution; voxel_plane previews by default or applies with dryRun=false + baseRevision. width/height select pixel resolution, one side preserves aspect. Image top-left maps to minimum u/maximum v (xy: x/y, xz: x/z, yz: z/y). Default canvasMode=overlay replaces opaque pixels only; replace erases the whole rectangle including transparent holes. maxColors median-cut quantizes colors; materialIds instead maps to existing materials. New RGB goes directly to MF6. This creates a pixel-art plane, not 3D reconstruction."
            } else {
                "本机参考图工具：list列图库；inspect读取精确RGBA8像素和紧凑采样网格；resize裁剪/缩放为新参考图，保留原图与署名；voxel_plane默认只预览，dryRun=false并提供baseRevision才应用。width/height控制像素分辨率，只给一边等比缩放。顶行映射较大v；xy的u/v=x/y，xz=x/z，yz=z/y。默认canvasMode=overlay仅覆盖不透明像素；replace替换整个矩形画布（透明孔也擦除）；alphaThreshold后按不透明方块显示；maxColors中位切分量化颜色，或materialIds映射已有材质。新RGB可直接发MF6。这是像素画平面，不是自动三维重建。"
            },
            json!({"action":{"type":"string","enum":["list","inspect","resize","voxel_plane"]},"imageId":{"type":"string"},"width":{"type":"integer","minimum":1,"maximum":1024},"height":{"type":"integer","minimum":1,"maximum":1024},"crop":{"type":"object","properties":{"x":{"type":"integer","minimum":0},"y":{"type":"integer","minimum":0},"width":{"type":"integer","minimum":1},"height":{"type":"integer","minimum":1}},"required":["x","y","width","height"],"additionalProperties":false},"filter":{"type":"string","enum":["nearest","area","lanczos"]},"points":{"type":"array","maxItems":256,"items":{"type":"array","items":{"type":"integer","minimum":0},"minItems":2,"maxItems":2}},"grid":{"type":"array","items":{"type":"integer","minimum":1,"maximum":64},"minItems":2,"maxItems":2},"baseRevision":{"type":"integer","minimum":0},"dryRun":{"type":"boolean"},"plane":{"type":"string","enum":["xy","xz","yz"]},"origin":{"type":"array","items":{"type":"integer"},"minItems":3,"maxItems":3},"thickness":{"type":"integer","minimum":1,"maximum":64},"alphaThreshold":{"type":"integer","minimum":1,"maximum":255},"maxColors":{"type":"integer","minimum":1,"maximum":4096},"materialIds":{"type":"array","minItems":1,"maxItems":256,"items":{"type":"integer","minimum":1,"maximum":65535}},"group":{"type":"string"},"title":{"type":"string"},"canvasMode":{"type":"string","enum":["overlay","replace"]}}),
            vec!["action"]
        ),
        tool(
            "define_materials",
            if en {
                "Define runtime color/PBR/water/cloud/wood/stone materials; returns IDs/UIDs and revision plus quantized vrcEffective. MF6 sends them inline without Avatar rebuild; arbitrary new image textures still need packaging. Parameters outside supported wire ranges are rejected."
            } else {
                "定义实时纯色/PBR/水/云/木纹/石纹材质。返回材质ID/UID、revision和VRC量化后的vrcEffective；MF6无需重传头像。新图片贴图仍需打包。kind=color为精确RGB哑光；pbr可指定表面参数；程序材质支持noiseScale/seed。"
            },
            json!({"baseRevision":{"type":"integer","minimum":0},"materials":{"type":"array","minItems":1,"maxItems":32,"items":{"type":"object","properties":{"name":{"type":"string","minLength":1,"maxLength":128},"color":{"type":"string","pattern":"^#[0-9a-fA-F]{6}$"},"kind":{"type":"string","enum":["color","pbr","water","cloud","wood","stone"]},"metallic":{"type":"number","minimum":0,"maximum":1},"roughness":{"type":"number","minimum":0,"maximum":1},"opacity":{"type":"number","minimum":0,"maximum":1},"emission":{"type":"number","minimum":0,"maximum":3},"noiseScale":{"type":"number","minimum":0.25,"maximum":8},"seed":{"type":"integer","minimum":0,"maximum":4294967295u64}},"required":["name","color","kind"],"additionalProperties":false}}}),
            vec!["baseRevision", "materials"]
        ),
        tool(
            "run_recipe",
            descriptions[1],
            json!({
                "code":{"type":"string"},
                "mode":{"type":"string","enum":["replace","apply"]},
                "baseRevision":{"type":"integer","minimum":0},
                "seed":{"type":"integer"},
                "dryRun":{"type":"boolean"}
            }),
            vec!["code", "mode", "baseRevision", "seed"],
        ),
        tool(
            "recipe_sources",
            if en {
                "List/read/write/patch/delete virtual JS files, or run an entry with params. CommonJS require resolves only saved project files. Existing writes/deletes need expectedHash from read; writes and runs need baseRevision. Source files survive compact and restart. Exports: module.exports=(mf,params)=>{...}. No host filesystem access."
            } else {
                "列出/读取/写入/精确patch/删除虚拟JS文件，或带params运行入口。支持CommonJS require，仅访问已保存的工程文件；修改/删除已有文件需read/write返回的file.hash作为expectedHash；patch.edits用唯一find/replace，只传变更片段，写入和run需baseRevision。源码跨compact/重启保留。导出示例：module.exports=(mf,params)=>{...}。不提供主机文件系统。"
            },
            json!({
                "action":{"type":"string","enum":["list","read","write","patch","delete","run"]},"path":{"type":"string"},"content":{"type":"string"},"expectedHash":{"type":"string","description":"read/write返回的file.hash，不是工作区sourceHash。"},"edits":{"type":"array","minItems":1,"maxItems":32,"items":{"type":"object","properties":{"find":{"type":"string","minLength":1},"replace":{"type":"string"},"all":{"type":"boolean"}},"required":["find","replace"],"additionalProperties":false}},"baseRevision":{"type":"integer","minimum":0},"params":{"type":"object"},"mode":{"type":"string","enum":["apply","replace"]},"seed":{"type":"integer","minimum":0},"dryRun":{"type":"boolean"},"requestId":{"type":"string"}
            }),
            vec!["action"]
        ),
        tool(
            "transform_scene",
            if en {
                "Translate the existing whole scene or center it on the ground; dryRun previews offset and bounds. Does not rewrite recipe source. Uses signed integer cells, preserves materials/groups and supports Undo. Auto-centering is not performed after every draw."
            } else {
                "整体平移现有作品或落地居中。dryRun预览偏移和边界，保留材质/分组、可撤销；不改写Recipe源码，重新replace源码仍采用原坐标。不要每步自动居中。"
            },
            json!({"action":{"type":"string","enum":["translate","center_on_ground"]},"offset":{"type":"array","items":{"type":"integer"},"minItems":3,"maxItems":3},"baseRevision":{"type":"integer","minimum":0},"dryRun":{"type":"boolean"}}),
            vec!["action", "baseRevision"]
        ),
        tool(
            "builtin_materials",
            if en {
                "List offline original pixel material themes or add one to the current project. apply requires baseRevision and id. replaceBaseMaterials optionally repaints built-in base styles. Bundled MF6 pixel textures are already in the matching Avatar and resolve by identity. Arbitrary new textures require an Avatar rebuild; stop OSC before applying."
            } else {
                "查看离线内置原创像素材质主题，或加入当前工程。apply需id和baseRevision；replaceBaseMaterials可将基础材质映射为像素风。配套MF6头像已内置此主题，作品编号不同也能映射；额外新贴图仍需打包，应用前停止OSC。"
            },
            json!({"action":{"type":"string","enum":["list","apply"]},"id":{"type":"string","enum":["pixel-workshop","block-workshop"]},"baseRevision":{"type":"integer","minimum":0},"replaceBaseMaterials":{"type":"boolean"}}),
            vec!["action"]
        ),
        tool(
            "inspect_region",
            descriptions[2],
            json!({
                "min":{"type":"array","items":{"type":"integer"},"minItems":3,"maxItems":3},
                "size":{"type":"array","items":{"type":"integer"},"minItems":3,"maxItems":3},
                "group":{"type":"string"},"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":128},
                "slice":{"type":"object","properties":{"axis":{"type":"string","enum":["x","y","z"]},"index":{"type":"integer"}},"required":["axis","index"],"additionalProperties":false}
            }),
            vec![],
        ),
        tool(
            "read_artifact",
            if en {
                "Reread a saved image or JS/JSON/text artifact. Optional pixel crop zooms an image without changing the scene. Historical/reference rereads do not count as current-revision captures or Goal review."
            } else {
                "重新查看已保存图片或JS/JSON/文本artifact，可用像素crop放大局部，不改变场景。历史/参考图片重读不算当前revision截图或Goal复核。"
            },
            json!({"artifact":{"type":"string"},"crop":{"type":"object","properties":{"x":{"type":"integer","minimum":0},"y":{"type":"integer","minimum":0},"width":{"type":"integer","minimum":1},"height":{"type":"integer","minimum":1}},"required":["x","y","width","height"],"additionalProperties":false}}),
            vec!["artifact"]
        ),
        tool(
            "capture_views",
            descriptions[3],
            json!({
                "revision":{"type":"integer","minimum":0},
                "source":{"type":"string","enum":["draft","committed"],"description":"draft当前草稿/指定revision；committed最后提交快照，不代表远端接收。"},
                "inspection":{"type":"object","description":"临时剖视/移开副本，不修改作品，也不能作为最终完整截图。hideBounds隐藏相交整个对象。","properties":{"includeGroups":{"type":"array","items":{"type":"string"}},"hideGroups":{"type":"array","items":{"type":"string"}},"hideBounds":{"type":"object","properties":{"min":{"type":"array","items":{"type":"integer"},"minItems":3,"maxItems":3},"max":{"type":"array","items":{"type":"integer"},"minItems":3,"maxItems":3}},"required":["min","max"],"additionalProperties":false},"offsets":{"type":"array","items":{"type":"object","properties":{"group":{"type":"string"},"offset":{"type":"array","items":{"type":"integer"},"minItems":3,"maxItems":3}},"required":["group","offset"],"additionalProperties":false}}},"additionalProperties":false},
                "mode":{"type":"string","enum":["art","vrc"]},"quality":{"type":"string","enum":["standard","high"]},"supersampling":{"type":"integer","minimum":1,"maximum":3},"voxelGrid":{"type":"boolean"},"surfaceCulling":{"type":"boolean"},
                "shots":{"type":"array","minItems":1,"maxItems":5,"description":"自由/局部取景；可与views组合，同名覆盖预设，合计最多5个视角。坐标为体素格。yaw=0从+Z看正面；pitch=0为平视。","items":{"type":"object","properties":{
                    "id":{"type":"string","pattern":"^[A-Za-z0-9_-]{1,64}$"},"projection":{"type":"string","enum":["perspective","orthographic"]},
                    "yaw":{"type":"number","minimum":-360,"maximum":360},"pitch":{"type":"number","minimum":-90,"maximum":90},"fov":{"type":"number","minimum":10,"maximum":120},"margin":{"type":"number","minimum":0,"maximum":1},
                    "position":{"type":"array","items":{"type":"number"},"minItems":3,"maxItems":3},"target":{"type":"array","items":{"type":"number"},"minItems":3,"maxItems":3},
                    "fit":{"type":"object","properties":{"group":{"type":"string"},"bounds":{"type":"object","properties":{"min":{"type":"array","items":{"type":"number"},"minItems":3,"maxItems":3},"max":{"type":"array","items":{"type":"number"},"minItems":3,"maxItems":3}},"required":["min","max"],"additionalProperties":false}},"additionalProperties":false}
                },"required":["id"],"additionalProperties":false}},
                "views":{"type":"array","items":{"type":"string","enum":["hero","front","top","side","detail"]}},
                "width":{"type":"integer","minimum":128,"maximum":2048},
                "height":{"type":"integer","minimum":128,"maximum":2048}
            }),
            vec!["revision"],
        ),
        tool(
            "commit",
            descriptions[4],
            json!({"revision":{"type":"integer","minimum":0},"title":{"type":"string"}}),
            vec!["revision", "title"],
        ),
        tool(
            "publish",
            descriptions[5],
            json!({"revision":{"type":"integer","minimum":0},"resync":{"type":"boolean"},"repair":{"type":"boolean","description":"MF6保留显示补播，不得与resync同时开启"},"loopReplay":{"type":"boolean","description":"MF6发送完成后持续循环补播；不会自动开启OSC"},"sendOrder":{"type":"string","enum":["volume","bottom_up","material"]}}),
            vec!["revision"],
        ),
        tool("sync_status", descriptions[6], json!({}), vec![]),
        tool(
            "undo_to",
            descriptions[7],
            json!({"revision":{"type":"integer","minimum":0}}),
            vec!["revision"],
        ),
        tool(
            "artworks",
            descriptions[8],
            json!({
                "action":{"type":"string","enum":["search","save","load","fork"]},
                "query":{"type":"string"},
                "title":{"type":"string"},
                "id":{"type":"string"},
                "baseRevision":{"type":"integer","minimum":0,"description":"load/fork必填，使用get_context返回的当前revision。Required for load/fork."},
                "tags":{"type":"array","items":{"type":"string"}},
                "thumbnail":{"type":"string","description":"Optional captured view name or existing artifact filename; defaults to hero/first image from the latest current-revision capture."},
                "views":{"type":"array","items":{"type":"string"},"description":"Captured view aliases (hero/front/top/side/detail) or real artifact filenames. Omit to reuse every image in the latest current-revision capture."}
            }),
            vec!["action"],
        ),
        tool("compact_context", descriptions[9], json!({}), vec![]),
        tool(
            "voxel_text",
            descriptions[10],
            text_properties,
            vec!["mode"]
        ),
        tool(
            "finish_artwork",
            descriptions[11],
            json!({
                "title":{"type":"string","minLength":1,"maxLength":1024},
                "summary":{"type":"string","minLength":1,"maxLength":16000},
                "tags":{"type":"array","items":{"type":"string","maxLength":128},"maxItems":64},
                "views":{"type":"array","items":{"type":"string"},"maxItems":5},
                "thumbnail":{"type":"string"},
                "goalAssessment":{"type":"string","maxLength":16000,"description":"Goal模式必填；逐项对照目标说明结果及限制。Required in Goal mode: assess each criterion and disclose limitations."}
            }),
            vec!["title", "summary"]
        ),
    ]);
    let mut capture = schemas
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "capture_views")
        .unwrap()["parameters"]
        .clone();
    capture["properties"]
        .as_object_mut()
        .unwrap()
        .remove("revision");
    capture["required"] = json!([]);
    for tool in schemas.as_array_mut().unwrap() {
        if matches!(tool["name"].as_str(), Some("run_recipe" | "recipe_sources")) {
            tool["parameters"]["properties"]["capture"] = capture.clone();
            let suffix = if en {
                " Optional capture auto-renders the committed revision in the same tool result, saving a model round trip. Inspect images in the next response before finishing. If rendering fails, geometryCommitted/captureError says to retry only capture."
            } else {
                " 可选capture在成功绘制后自动截图并返回图片，省去一次模型请求；下一响应看图后才能完成。截图失败返回geometryCommitted/captureError，只重试截图，勿重复绘制。"
            };
            tool["description"] = json!(format!(
                "{}{suffix}",
                tool["description"].as_str().unwrap_or("")
            ));
        }
    }
    schemas
        .as_array_mut()
        .unwrap()
        .extend(crate::creative_schema::tools(en));
    let capture = schemas
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|t| t["name"] == "capture_views")
        .unwrap();
    let properties = capture["parameters"]["properties"].as_object_mut().unwrap();
    properties.insert("comparison".into(),json!({"type":"object","properties":{"imageId":{"type":"string"},"mode":{"type":"string","enum":["split","overlay"]},"opacity":{"type":"number"},"zoom":{"type":"number"},"offset":{"type":"array","items":{"type":"number"},"minItems":2,"maxItems":2},"crop":{"type":"array","items":{"type":"number"},"minItems":4,"maxItems":4}},"required":["imageId","mode","opacity","zoom","offset","crop"],"additionalProperties":false}));
    properties.insert("bookmarkId".into(), json!({"type":"string"}));
    properties.insert("compare".into(), json!({"type":"boolean"}));
    properties.insert("animation".into(),json!({"type":"object","properties":{"clipId":{"type":"string"},"time":{"type":"number","minimum":0}},"required":["clipId","time"],"additionalProperties":false}));
    schemas
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with_run(status: &str) -> State {
        let settings = ProviderSettings::default();
        State {
            settings: settings.clone(),
            conversation: Conversation::default(),
            run: Some(Run {
                id: "run-1".into(),
                epoch: 1,
                status: status.into(),
                prompt: String::new(),
                started_at: now(),
                finished_at: None,
                steps: 0,
                tokens: 0,
                active_request_id: None,
                prompt_version: SYSTEM_PROMPT_VERSION.into(),
                tools_version: TOOL_SCHEMA_VERSION.into(),
                language: settings.language,
                message: String::new(),
                error: String::new(),
            }),
            run_deadline: Some(tokio::time::Instant::now() + Duration::from_secs(900)),
            cancel: Cancellation::new(),
            paused: false,
            compact_requested: false,
            phase: "running".into(),
            epoch: 1,
            active_request_id: None,
            worker_epoch: Some(1),
            active_budget_remaining: None,
            queue_turn_complete: true,
            pending_steers: VecDeque::new(),
            pending_interrupts: VecDeque::new(),
            queued: VecDeque::new(),
            message_acks: HashMap::new(),
        }
    }

    #[test]
    fn compact_usage_counts_toward_the_active_run_budget() {
        let mut state = state_with_run("compacting");
        record_run_tokens(&mut state, "run-1", 73);
        assert_eq!(state.run.as_ref().unwrap().tokens, 73);
        record_run_tokens(&mut state, "other-run", 11);
        assert_eq!(state.run.as_ref().unwrap().tokens, 73);
    }

    #[test]
    fn localized_tool_schemas_keep_the_same_domain_surface() {
        let chinese = tools_for_language("zh-CN");
        assert_eq!(tools(), chinese);
        let english = tools_for_language("en");
        let names = chinese
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"run_recipe"));
        assert!(names.contains(&"voxel_text"));
        assert!(names.contains(&"finish_artwork"));
        assert!(!names
            .iter()
            .any(|name| ["shell", "bash", "browser", "edit_file"].contains(name)));
        for schema in [chinese, english] {
            for tool in schema.as_array().unwrap() {
                assert_eq!(tool["type"], "function");
                assert_eq!(tool["parameters"]["additionalProperties"], false);
            }
        }
        let context_description = |tools: &Value| {
            tools
                .as_array()
                .unwrap()
                .iter()
                .find(|tool| tool["name"] == "get_context")
                .unwrap()["description"]
                .clone()
        };
        assert_ne!(
            context_description(&tools_for_language("zh-CN")),
            context_description(&tools_for_language("en"))
        );
    }

    #[test]
    fn completed_output_validation_rejects_duplicate_call_ids_before_dispatch() {
        let valid = vec![
            json!({"type":"reasoning","summary":[{"type":"summary_text","text":"hidden"}]}),
            json!({"type":"message","content":[{"type":"output_text","text":"done"}]}),
            json!({"type":"function_call","call_id":"call-1","name":"run_recipe","arguments":"{}"}),
        ];
        assert!(validate_completed_output(&valid).is_ok());
        let duplicate = vec![
            json!({"type":"function_call","call_id":"call-1","name":"run_recipe","arguments":"{}"}),
            json!({"type":"function_call","call_id":"call-1","name":"commit","arguments":"{}"}),
        ];
        assert!(validate_completed_output(&duplicate).is_err());
    }

    #[test]
    fn finish_artwork_must_be_the_last_function_call_in_its_batch() {
        let finish_only = vec![function_call_for_test("finish-1", "finish_artwork")];
        assert!(validate_completed_output(&finish_only).is_ok());

        let finish_before_other_call = vec![
            function_call_for_test("finish-1", "finish_artwork"),
            function_call_for_test("commit-1", "commit"),
        ];
        assert!(validate_completed_output(&finish_before_other_call).is_err());

        let duplicate_finish = vec![
            function_call_for_test("finish-1", "finish_artwork"),
            function_call_for_test("finish-2", "finish_artwork"),
        ];
        assert!(validate_completed_output(&duplicate_finish).is_err());
    }

    fn function_call_for_test(call_id: &str, name: &str) -> Value {
        json!({"type":"function_call","call_id":call_id,"name":name,"arguments":"{}"})
    }

    #[test]
    fn reasoning_is_replayed_but_never_used_as_user_visible_run_text() {
        let output = vec![
            json!({"type":"reasoning","summary":[{"type":"summary_text","text":"private reasoning"}]}),
            json!({"type":"message","content":[{"type":"output_text","text":"visible answer"}]}),
        ];
        assert_eq!(assistant_text(&output).as_deref(), Some("visible answer"));
    }
}
