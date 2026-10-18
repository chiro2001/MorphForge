use crate::{
    capture::CaptureBroker,
    fonts::FontCatalog,
    provider::{Cancellation, Provider, ProviderRequestTrace, ProviderSettings},
    recipe::{self, RecipeRequest},
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
mod prompts;
pub const SYSTEM_PROMPT_VERSION: &str = "morphforge-system/2.3";
pub const TOOL_SCHEMA_VERSION: &str = "morphforge-tools/2.3";

#[derive(Clone, Debug)]
struct PendingPrompt {
    request_id: String,
    run_id: String,
    prompt: String,
    mode: String,
    accepted_at: String,
    accepted_epoch: u64,
}

#[derive(Clone)]
struct MessageReceipt {
    ack: Value,
    prompt: String,
    mode: String,
    requested_run_id: Option<String>,
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
    pub input: Vec<Value>,
    pub goals: Vec<String>,
    pub summary: String,
    pub boundaries: Vec<usize>,
    pub compact_count: u32,
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
        let conversation = std::fs::read(root.join("runs/conversation.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let settings = std::fs::read(root.join("harness.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let (message_acks, restored_queue) = restore_message_state(&events);
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
        json!({
            "settings":s.settings,
            "provider":self.provider.status(),
            "run":s.run,
            "paused":s.paused,
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
                "estimatedTokens":estimate(&s.conversation.input),
                "messages":s.conversation.input.len(),
                "compactCount":s.conversation.compact_count,
                "summary":s.conversation.summary,
                "goals":s.conversation.goals
            }
        })
    }
    pub fn configure(&self, v: &Value) -> Result<Value> {
        let mut s = self.inner.lock().unwrap();
        let mut settings = serde_json::to_value(&s.settings)?;
        for (k, v) in v.as_object().context("设置须为对象")? {
            if settings.get(k).is_none() {
                bail!("未知Harness设置 {k}")
            }
            settings[k] = v.clone();
        }
        let next: ProviderSettings = serde_json::from_value(settings)?;
        if !["deepseek-flash", "deepseek-v4-pro"].contains(&next.model.as_str())
            || !["none", "low", "high", "max"].contains(&next.reasoning.as_str())
            || next.max_steps == 0
            || next.max_steps > 100
            || next.max_output_tokens < 512
            || next.max_output_tokens > 65536
            || next.compact_tokens < 1000
            || next.max_run_tokens < 1000
            || next.max_run_seconds == 0
            || next.max_run_seconds > 86400
            || !["zh-CN", "en"].contains(&next.language.as_str())
        {
            bail!("Harness预算配置越界")
        };
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
        write_json(&w.root.join("runs/conversation.json"), &conversation)?;
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
        let prompt = value
            .get("prompt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
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
        if !prompt.trim().is_empty() && !self.provider.configured() {
            bail!("请先配置DeepSeek API Key")
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
                if receipt.prompt != prompt || receipt.mode != mode || !same_run {
                    bail!("REQUEST_ID_CONFLICT：requestId已用于不同消息")
                }
                return Ok(receipt.ack.clone());
            }

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
            });
            state.message_acks.insert(
                request_id.clone(),
                MessageReceipt {
                    ack: ack.clone(),
                    prompt: prompt.clone(),
                    mode: mode.clone(),
                    requested_run_id: supplied_run_id.clone(),
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
        match action {
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
            "reset_context" => {
                let mut s = self.inner.lock().unwrap();
                if !["idle", "completed", "failed", "stopped"].contains(&s.phase.as_str()) {
                    bail!("请先停止当前运行")
                };
                s.conversation = Conversation::default();
                s.pending_steers.clear();
                s.pending_interrupts.clear();
                s.queued.clear();
            }
            _ => bail!("未知运行控制"),
        };
        self.persist()?;
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
                    state.phase = if timed_out {
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

            let previous_input_tokens =
                latest_provider_input_tokens(&self.workspace.lock().unwrap(), &guard.id);
            if force_compact
                || (settings.auto_compact
                    && previous_input_tokens
                        .is_some_and(|tokens| tokens > settings.compact_tokens as u64))
            {
                self.compact_with_guard(&guard, force_compact).await?;
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

            let (input, settings, steps_used, tokens_used) = {
                let state = self.inner.lock().unwrap();
                let run = state.run.as_ref().context("没有活动run")?;
                (
                    state.conversation.input.clone(),
                    state.settings.clone(),
                    run.steps,
                    run.tokens,
                )
            };
            let context = self.context();
            let system = prompts::system_prompt(&settings.language);
            let tool_schemas = tools_for_language(&settings.language);
            let remaining_steps = settings.max_steps.saturating_sub(steps_used);
            let remaining_tokens = settings.max_run_tokens.saturating_sub(tokens_used);
            // Request estimates guide early completion only. Provider-reported usage
            // is the source of truth for the cumulative run-token budget.
            let estimated_input_tokens = estimate(&input)
                .saturating_add(estimate(std::slice::from_ref(&context)))
                .saturating_add(estimate(std::slice::from_ref(&tool_schemas)))
                .saturating_add(system.len() / 3)
                .saturating_mul(2)
                .saturating_add(2048);
            let estimated_input_tokens_u64 =
                u64::try_from(estimated_input_tokens).unwrap_or(u64::MAX);
            if remaining_tokens == 0 {
                bail!("RUN_BUDGET：Provider实际用量已达到Token预算，停止发起新请求")
            }
            let effective_output_tokens = settings
                .max_output_tokens
                .min(remaining_tokens.min(u32::MAX as u64) as u32);
            let remaining_seconds = guard
                .deadline
                .saturating_duration_since(tokio::time::Instant::now())
                .as_secs_f64();
            let request_budget_estimate = estimated_input_tokens_u64
                .saturating_add(settings.max_output_tokens as u64)
                .saturating_mul(3);
            let near_budget = remaining_steps <= 3
                || remaining_tokens <= request_budget_estimate
                || remaining_seconds <= 120.0;
            let budget_note = if settings.language == "en" {
                format!(
                    "\nCurrent run budget: {remaining_steps} steps, {remaining_tokens} provider-reported total tokens, and {remaining_seconds:.0} active seconds remain. Effective output cap for this request: {effective_output_tokens}; it never exceeds the configured cap or remaining token budget. Local input estimates are advisory only. Provider input usage can exceed the remaining budget unexpectedly; at most one complete response may cross it, and no later model request will start.{}",
                    if near_budget {
                        " Budget is close: stop adding detail, inspect the current result, and use this response's final tool call for finish_artwork so commit and library save happen together. Do not spend the remaining budget on polish."
                    } else {
                        ""
                    }
                )
            } else {
                format!(
                    "\n当前run预算：剩余{remaining_steps}步、Provider实际统计的总Token余量{remaining_tokens}和{remaining_seconds:.0}秒活动时间。本请求有效输出上限{effective_output_tokens}，不会超过配置上限或实际剩余预算。输入估算仅作提前收尾提示。Provider实际输入用量可能不可预测地超过余量；最多允许一个完整响应越过预算，之后不再发起模型请求。{}",
                    if near_budget {
                        "预算已接近上限：停止增加细节并检查当前结果；本响应最后一个工具调用用finish_artwork一次完成commit和作品库保存，不要再把余量花在润色上。"
                    } else {
                        ""
                    }
                )
            };
            let instructions = format!(
                "{system}\n当前权威上下文摘要：{}\n{budget_note}{}",
                serde_json::to_string(&context)?,
                retry_note
            );
            let request_id = uuid::Uuid::new_v4().to_string();
            let payload = json!({
                "model":settings.model,
                "instructions":instructions,
                "input":input,
                "tools":tool_schemas,
                "reasoning":{"effort":settings.reasoning},
                "max_output_tokens":effective_output_tokens,
                "stream":true
            });
            {
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
                    }),
                )?;
            }
            let trace = ProviderRequestTrace {
                request_id: Some(request_id.clone()),
                prompt_version: Some(SYSTEM_PROMPT_VERSION.into()),
                tools_version: Some(TOOL_SCHEMA_VERSION.into()),
                language: Some(settings.language.clone()),
                epoch: Some(guard.epoch),
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
                    return Err(error);
                }
            };
            let usage = response["usage"]["total_tokens"].as_u64().unwrap_or(0);
            self.record_request_attempt(&guard, &request_id, usage);

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
                            if name == "run_recipe"
                                || (name == "voxel_text" && arguments["mode"] == "apply")
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
                let value = match result {
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
                    Err(error) => json!({
                        "ok":false,
                        "error":{"code":"TOOL_ERROR","message":self.provider.redact(&error.to_string())}
                    }),
                };
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
                if !cancelled && settings.model == "deepseek-flash" {
                    if let Some(images) = value["images"].as_array() {
                        for image in images {
                            if let Some(artifact) = image["artifact"].as_str() {
                                if !self.guard_is_current(&guard) {
                                    cancel_remaining = true;
                                    break;
                                }
                                match tokio::select! {
                                    biased;
                                    result = self.provider.image_part(
                                        self.workspace.clone(),
                                        artifact,
                                        settings.upload_images,
                                        &guard.cancel,
                                    ) => result,
                                    _ = guard.cancel.wait() => Err(anyhow::anyhow!("RUN_CANCELLED")),
                                    _ = tokio::time::sleep_until(guard.deadline) => {
                                        guard.cancel.stop();
                                        budget_expired = true;
                                        Err(anyhow::anyhow!("RUN_TIME_BUDGET"))
                                    }
                                } {
                                    Ok(part) => parts.push(part),
                                    Err(_) => {
                                        cancel_remaining = true;
                                        parts.push(json!({
                                            "type":"input_text",
                                            "text":"截图附件未能在中断前送达；后续可重新捕获。"
                                        }));
                                        break;
                                    }
                                }
                            }
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
            state.conversation.goals.push(message.prompt.clone());
            state
                .conversation
                .input
                .push(json!({"role":"user","content":message.prompt}));
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
            "stats":stats,
            "targetCompatibility":target_compatibility,
            "protectedGroups":workspace.project.protected_groups,
            "styles":styles,
            "catalog":catalog,
            "pinnedNotes":workspace.project.pinned_notes,
            "sync":self.transport.state(),
            "sdkVersion":recipe::SDK_VERSION,
            "sdk":"mf.box(min,[w,h,d],{style:'wood',tint:0,motion:0}); mf.erase(min,size); mf.paint(min,size,{style}) recolors existing voxels only and never creates them; use mf.box to add voxels. mf.clear(); mf.group(name,()=>{}); mf.repeat(n,(i)=>{}); mf.mirror('x',()=>{}); mf.translate([x,y,z],()=>{}); mf.line(a,b,{thickness:1,style}); mf.stairs(origin,n,{width:3,depth:1,rise:1,style}); mf.wall(origin,w,h,{thickness:1,style}); mf.roof(origin,w,d,h,{style}); Math.random()/mf.random() deterministic by seed. All coordinates integer signed grid. Y is height. VRC target requires X/Z[-128,128),Y[0,256). Styles are catalog IDs/UID names. mode apply preserves current geometry; replace starts from empty."
        })
    }
    pub async fn tool(&self, name: &str, args: &Value, run: Option<&str>) -> Result<Value> {
        self.tool_inner(name, args, run, None).await
    }

    async fn tool_with_guard(
        &self,
        name: &str,
        args: &Value,
        run: Option<&str>,
        guard: RunGuard,
    ) -> Result<Value> {
        self.tool_inner(name, args, run, Some(guard)).await
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
        match name {
            "get_context" => Ok(self.context_with_options(args)),
            "run_recipe" => {
                let request: RecipeRequest = serde_json::from_value(args.clone())?;
                self.run_recipe_with_guard(request, run, guard.as_ref()).await
            }
            "voxel_text" => self.voxel_text(args, run, guard.as_ref()).await,
            "capture_views" => {
                let request = self.capture.request(self.workspace.clone(), args, run);
                if let Some(guard) = guard {
                    tokio::select! {
                        biased;
                        result = request => result,
                        _ = guard.cancel.wait() => bail!("RUN_CANCELLED"),
                        _ = tokio::time::sleep_until(guard.deadline) => {
                            guard.cancel.stop();
                            bail!("RUN_TIME_BUDGET：截图超过运行时限")
                        }
                    }
                } else {
                    request.await
                }
            }
            "inspect_region" => {
                let workspace = self.workspace.lock().unwrap();
                let mut scene = workspace.project.scene.clone();
                if let (Some(min), Some(size)) = (args.get("min"), args.get("size")) {
                    let min = serde_json::from_value(min.clone())?;
                    let size = serde_json::from_value(size.clone())?;
                    let region = morphforge_core::Box3::new(min, size, 0);
                    region.validate(&workspace.project.limits)?;
                    scene.boxes.retain(|box3| box3.intersects(&region));
                }
                if let Some(group) = args["group"].as_str() {
                    scene.boxes.retain(|box3| box3.group == group)
                }
                Ok(json!({
                    "ok":true,
                    "revision":scene.revision,
                    "stats":scene.stats(&workspace.project.limits),
                    "sample":scene.boxes.iter().take(20).collect::<Vec<_>>(),
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
                let scene = workspace.scene_at(revision)?;
                let state = self
                    .transport
                    .publish(&scene, args["resync"].as_bool().unwrap_or(false))?;
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
                        Ok(json!({"ok":true,"artworks":workspace.library()?.into_iter().filter(|artwork|artwork.title.to_lowercase().contains(&query)||artwork.tags.iter().any(|tag|tag.to_lowercase().contains(&query))).collect::<Vec<_>>()}))
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
            let result = finish_artwork_in_workspace(&mut workspace, args, title, summary, run)?;
            let run_state = state
                .run
                .as_mut()
                .filter(|run_state| run_state.id == guard.id && run_state.epoch == guard.epoch)
                .context("RUN_CANCELLED")?;
            run_state.message = summary.to_owned();
            Ok(result)
        } else {
            finish_artwork_in_workspace(
                &mut self.workspace.lock().unwrap(),
                args,
                title,
                summary,
                run,
            )
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
                    "sdkVersion":recipe::SDK_VERSION,
                    "artifact":artifact,
                    "result":value,
                }),
            )?;
            if self.transport.config().enabled && self.transport.config().auto {
                let _ = workspace.commit(None, "自动发布", run)?;
                let state = self.transport.publish(&workspace.project.scene, false)?;
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
        if before < 500 && !force {
            return Ok(());
        }
        let cut = conversation
            .boundaries
            .iter()
            .rev()
            .nth(2)
            .copied()
            .unwrap_or(conversation.input.len());
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
            let payload = json!({
                "model":settings.model,
                "instructions":format!("{}\n{}",prompts::system_prompt(&settings.language),compact_prompt),
                "input":[{"role":"user","content":prompt}],
                "reasoning":{"effort":"none"},
                "max_output_tokens":4096,
                "stream":true
            });
            let trace = ProviderRequestTrace {
                request_id: Some(request_id.clone()),
                prompt_version: Some(SYSTEM_PROMPT_VERSION.into()),
                tools_version: Some(TOOL_SCHEMA_VERSION.into()),
                language: Some(settings.language.clone()),
                epoch: Some(guard.epoch),
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
                    Err(anyhow::anyhow!("RUN_TIME_BUDGET：compact超过运行时限"))
                }
            };
            let response = response_result?;
            self.clear_active_request(guard, &request_id);
            record_run_tokens(
                &mut self.inner.lock().unwrap(),
                &guard.id,
                response["usage"]["total_tokens"].as_u64().unwrap_or(0),
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
                "summary":summary,
                "stateAuthority":"调用get_context重新读取当前工程/发布状态"
            });
            let mut input = vec![json!({"role":"user","content":serde_json::to_string(&manifest)?})];
            input.extend(tail);
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

fn estimate(input: &[Value]) -> usize {
    serde_json::to_vec(input).map(|b| b.len() / 3).unwrap_or(0)
}

fn latest_capture_images(workspace: &Workspace) -> Option<Vec<(String, String)>> {
    let revision = workspace.project.scene.revision;
    let event = workspace.events.iter().rev().find(|event| {
        event.kind == "capture.completed"
            && event.data["revision"].as_u64() == Some(revision)
            && event.data["ok"] == true
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
) -> Result<Value> {
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
            "Capture fixed rendered views of a specific revision for visual review. The result images list maps each view to its real artifact filename and URL; reuse those values when saving an artwork.",
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
            "对指定 revision 捕获固定视角的真实渲染，用于检查作品。结果 images 会把每个视角映射到真实 artifact 文件名和 URL；保存作品时复用返回值。",
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
    json!([
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
            "inspect_region",
            descriptions[2],
            json!({
                "min":{"type":"array","items":{"type":"integer"},"minItems":3,"maxItems":3},
                "size":{"type":"array","items":{"type":"integer"},"minItems":3,"maxItems":3},
                "group":{"type":"string"}
            }),
            vec![],
        ),
        tool(
            "capture_views",
            descriptions[3],
            json!({
                "revision":{"type":"integer","minimum":0},
                "views":{"type":"array","items":{"type":"string","enum":["hero","front","top","side","detail"]}},
                "width":{"type":"integer","minimum":128,"maximum":2048},
                "height":{"type":"integer","minimum":128,"maximum":2048}
            }),
            vec!["revision", "views"],
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
            json!({"revision":{"type":"integer","minimum":0},"resync":{"type":"boolean"}}),
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
                "thumbnail":{"type":"string"}
            }),
            vec!["title", "summary"]
        ),
    ])
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
