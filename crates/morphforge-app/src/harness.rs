use crate::{
    capture::CaptureBroker,
    provider::{Cancellation, Provider, ProviderSettings},
    recipe::{self, RecipeRequest},
    store::{now, write_json, Workspace},
    transport::Transport,
};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
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
    pub status: String,
    pub prompt: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub steps: u32,
    pub tokens: u64,
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
    pub queued: Vec<String>,
}
#[derive(Clone)]
pub struct Harness {
    pub inner: Arc<Mutex<State>>,
    pub workspace: Arc<Mutex<Workspace>>,
    pub provider: Provider,
    pub capture: CaptureBroker,
    pub transport: Transport,
}
const SYSTEM: &str = include_str!("system_prompt.txt");
impl Harness {
    pub fn new(
        workspace: Arc<Mutex<Workspace>>,
        provider: Provider,
        capture: CaptureBroker,
        transport: Transport,
    ) -> Self {
        let root = workspace.lock().unwrap().root.clone();
        let conversation = std::fs::read(root.join("runs/conversation.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let settings = std::fs::read(root.join("harness.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self {
            inner: Arc::new(Mutex::new(State {
                settings,
                conversation,
                run: None,
                run_deadline: None,
                cancel: Cancellation::new(),
                paused: false,
                compact_requested: false,
                queued: vec![],
            })),
            workspace,
            provider,
            capture,
            transport,
        }
    }
    pub fn state(&self) -> Value {
        let s = self.inner.lock().unwrap();
        json!({"settings":s.settings,"provider":self.provider.status(),"run":s.run,"paused":s.paused,"queued":s.queued,"context":{"estimatedTokens":estimate(&s.conversation.input),"messages":s.conversation.input.len(),"compactCount":s.conversation.compact_count,"summary":s.conversation.summary,"goals":s.conversation.goals}})
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
        };
        if !self.provider.configured() {
            bail!("请先配置DeepSeek API Key")
        }
        let mut s = self.inner.lock().unwrap();
        if s.run
            .as_ref()
            .is_some_and(|r| r.status == "running" || r.status == "compacting")
        {
            s.queued.push(prompt.into());
            drop(s);
            self.workspace.lock().unwrap().event(
                "harness.steered",
                None,
                json!({"prompt":prompt}),
            )?;
            return Ok(self.state());
        }
        let id = uuid::Uuid::new_v4().to_string();
        s.cancel = Cancellation::new();
        s.paused = false;
        let deadline = tokio::time::Instant::now()
            + std::time::Duration::from_secs(s.settings.max_run_seconds);
        s.run_deadline = Some(deadline);
        s.run = Some(Run {
            id: id.clone(),
            status: "running".into(),
            prompt: prompt.into(),
            started_at: now(),
            finished_at: None,
            steps: 0,
            tokens: 0,
            message: String::new(),
            error: String::new(),
        });
        s.conversation.goals.push(prompt.into());
        s.conversation
            .input
            .push(json!({"role":"user","content":prompt}));
        drop(s);
        self.persist()?;
        self.workspace.lock().unwrap().event(
            "harness.started",
            Some(&id),
            json!({"prompt":prompt,"toolVersion":"mf-tools/1"}),
        )?;
        let h = self.clone();
        tokio::spawn(async move {
            let result = h.run_loop_with_budget(&id).await;
            h.finish(&id, result);
        });
        Ok(self.state())
    }
    pub fn control(&self, action: &str) -> Result<Value> {
        let mut s = self.inner.lock().unwrap();
        match action {
            "stop" => s.cancel.stop(),
            "pause" => s.paused = true,
            "resume" => s.paused = false,
            "compact" => {
                s.compact_requested = true;
                if !s
                    .run
                    .as_ref()
                    .is_some_and(|r| r.status == "running" || r.status == "compacting")
                {
                    let id = uuid::Uuid::new_v4().to_string();
                    s.cancel = Cancellation::new();
                    let deadline = tokio::time::Instant::now()
                        + std::time::Duration::from_secs(s.settings.max_run_seconds);
                    s.run_deadline = Some(deadline);
                    s.run = Some(Run {
                        id: id.clone(),
                        status: "compacting".into(),
                        prompt: "手动compact".into(),
                        started_at: now(),
                        finished_at: None,
                        steps: 0,
                        tokens: 0,
                        message: String::new(),
                        error: String::new(),
                    });
                    drop(s);
                    let h = self.clone();
                    tokio::spawn(async move {
                        let result = h.compact_with_budget(&id, true).await;
                        h.finish(&id, result);
                    });
                    return Ok(self.state());
                }
            }
            "reset_context" => {
                if s.run
                    .as_ref()
                    .is_some_and(|r| r.status == "running" || r.status == "compacting")
                {
                    bail!("请先停止当前运行")
                };
                s.conversation = Conversation::default();
            }
            _ => bail!("未知运行控制"),
        };
        drop(s);
        self.persist()?;
        Ok(self.state())
    }
    fn finish(&self, id: &str, result: Result<()>) {
        let (continue_run, finished) = {
            let mut s = self.inner.lock().unwrap();
            let stopped = s.cancel.stopped();
            let result_error = result.as_ref().err().map(ToString::to_string);
            let timed_out = result_error
                .as_ref()
                .is_some_and(|error| error.starts_with("RUN_TIME_BUDGET"));
            let continue_run = continue_after_success(&mut s, id, stopped, result.is_ok());
            let finished = if continue_run {
                None
            } else {
                let mut finished = None;
                if let Some(run) = &mut s.run {
                    if run.id == id {
                        run.status = if timed_out {
                            "failed"
                        } else if stopped {
                            "stopped"
                        } else if result_error.is_none() {
                            "completed"
                        } else {
                            "failed"
                        }
                        .into();
                        run.finished_at = Some(now());
                        if let Some(error) = &result_error {
                            run.error = error.clone();
                        }
                        finished = Some(run.clone());
                    }
                }
                if finished.is_some() {
                    s.run_deadline = None;
                }
                finished
            };
            (continue_run, finished)
        };
        if continue_run {
            let h = self.clone();
            let id = id.to_owned();
            tokio::spawn(async move {
                let result = h.run_loop_with_budget(&id).await;
                h.finish(&id, result);
            });
            return;
        }
        if let Some(state) = finished {
            if let Ok(mut w) = self.workspace.lock() {
                let _ = w.event("harness.finished", Some(id), json!(&state));
                let _ = write_json(&w.root.join("runs").join(format!("{id}.json")), &state);
            }
            let _ = self.persist();
        }
    }
    fn run_deadline(&self, id: &str) -> Result<(tokio::time::Instant, Cancellation)> {
        let s = self.inner.lock().unwrap();
        if !s.run.as_ref().is_some_and(|run| run.id == id) {
            bail!("没有活动run")
        };
        Ok((
            s.run_deadline.unwrap_or_else(tokio::time::Instant::now),
            s.cancel.clone(),
        ))
    }
    async fn run_loop_with_budget(&self, id: &str) -> Result<()> {
        let (deadline, cancel) = self.run_deadline(id)?;
        match tokio::time::timeout_at(deadline, self.run_loop(id)).await {
            Ok(result) => result,
            Err(_) => {
                cancel.stop();
                bail!("RUN_TIME_BUDGET：run超过配置时长，已停止并保留已保存内容")
            }
        }
    }
    async fn compact_with_budget(&self, id: &str, force: bool) -> Result<()> {
        let (deadline, cancel) = self.run_deadline(id)?;
        match tokio::time::timeout_at(deadline, self.compact(id, force)).await {
            Ok(result) => result,
            Err(_) => {
                cancel.stop();
                bail!("RUN_TIME_BUDGET：run超过配置时长，compact已停止且旧上下文保留")
            }
        }
    }
    async fn run_loop(&self, id: &str) -> Result<()> {
        loop {
            let (settings, cancel, paused, force, step, tokens) = {
                let s = self.inner.lock().unwrap();
                let r = s.run.as_ref().context("没有运行")?;
                (
                    s.settings.clone(),
                    s.cancel.clone(),
                    s.paused,
                    s.compact_requested,
                    r.steps,
                    r.tokens,
                )
            };
            if cancel.stopped() {
                bail!("RUN_CANCELLED")
            };
            if paused {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                continue;
            }
            if step >= settings.max_steps || tokens >= settings.max_run_tokens {
                bail!("RUN_BUDGET：达到步骤或Token预算，已生成版本仍保留")
            }
            let estimated = { estimate(&self.inner.lock().unwrap().conversation.input) };
            if force || (settings.auto_compact && estimated > settings.compact_tokens) {
                self.compact(id, force).await?;
            }
            let (step, tokens) = {
                let s = self.inner.lock().unwrap();
                let r = s.run.as_ref().context("没有运行")?;
                (r.steps, r.tokens)
            };
            if step >= settings.max_steps || tokens >= settings.max_run_tokens {
                bail!("RUN_BUDGET：达到步骤或Token预算，已生成版本仍保留")
            }
            let appended = {
                let mut s = self.inner.lock().unwrap();
                let had_queued = !s.queued.is_empty();
                append_queued_prompts(&mut s);
                had_queued
            };
            if appended {
                self.persist()?;
            }
            let input = self.inner.lock().unwrap().conversation.input.clone();
            let context = self.context();
            let instructions = format!(
                "{SYSTEM}\n当前权威上下文摘要：{}",
                serde_json::to_string(&context)?
            );
            let payload = json!({"model":settings.model,"instructions":instructions,"input":input,"tools":tools(),"reasoning":{"effort":settings.reasoning},"max_output_tokens":settings.max_output_tokens,"stream":true});
            let response = self
                .provider
                .response(payload, self.workspace.clone(), id, &cancel)
                .await?;
            if response["status"] != "completed" {
                bail!("响应未完整完成")
            };
            let output = response["output"]
                .as_array()
                .context("响应没有output")?
                .clone();
            {
                let mut s = self.inner.lock().unwrap();
                if let Some(run) = s.run.as_mut().filter(|r| r.id == id) {
                    run.steps += 1;
                }
                record_run_tokens(
                    &mut s,
                    id,
                    response["usage"]["total_tokens"].as_u64().unwrap_or(0),
                );
            }
            let mut messages = output.clone();
            let mut called = false;
            for item in &output {
                if item["type"] == "message" {
                    let text = item["content"]
                        .as_array()
                        .map(|v| {
                            v.iter()
                                .filter_map(|p| p["text"].as_str())
                                .collect::<Vec<_>>()
                                .join("\n")
                        })
                        .unwrap_or_default();
                    self.inner.lock().unwrap().run.as_mut().unwrap().message = text;
                }
                if item["type"] != "function_call" {
                    continue;
                }
                called = true;
                let name = item["name"].as_str().context("工具缺少name")?;
                let call = item["call_id"].as_str().context("工具缺少call_id")?;
                while self.inner.lock().unwrap().paused {
                    if cancel.stopped() {
                        bail!("RUN_CANCELLED")
                    };
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
                if cancel.stopped() {
                    bail!("RUN_CANCELLED")
                }
                let args =
                    serde_json::from_str::<Value>(item["arguments"].as_str().unwrap_or("{}"));
                let start = std::time::Instant::now();
                self.workspace.lock().unwrap().event("tool.request",Some(id),json!({"callId":call,"name":name,"arguments":item["arguments"],"toolVersion":"mf-tools/1"}))?;
                let result = match args {
                    Ok(mut args) => {
                        if name == "run_recipe" {
                            args["requestId"] = json!(call)
                        }
                        tokio::select! {r=self.tool(name,&args,Some(id))=>r,_=cancel.wait()=>bail!("RUN_CANCELLED")}
                    }
                    Err(e) => Err(e.into()),
                };
                let value = match result {
                    Ok(v) => v,
                    Err(e) => {
                        json!({"ok":false,"error":{"code":"TOOL_ERROR","message":e.to_string()}})
                    }
                };
                self.workspace.lock().unwrap().event("tool.result",Some(id),json!({"callId":call,"name":name,"elapsedMs":start.elapsed().as_secs_f64()*1000.,"result":value}))?;
                let mut parts =
                    vec![json!({"type":"input_text","text":serde_json::to_string(&value)?})];
                if settings.model == "deepseek-flash" {
                    if let Some(images) = value["images"].as_array() {
                        for image in images {
                            if let Some(artifact) = image["artifact"].as_str() {
                                parts.push(
                                    self.provider
                                        .image_part(
                                            self.workspace.clone(),
                                            artifact,
                                            settings.upload_images,
                                            &cancel,
                                        )
                                        .await?,
                                );
                            }
                        }
                    }
                }
                messages.push(json!({"type":"function_call_output","call_id":call,"output":parts}));
            }
            {
                let mut s = self.inner.lock().unwrap();
                s.conversation.input.extend(messages);
                let boundary = s.conversation.input.len();
                s.conversation.boundaries.push(boundary);
            }
            self.persist()?;
            if !called && self.inner.lock().unwrap().queued.is_empty() {
                return Ok(());
            }
        }
    }
    pub fn context(&self) -> Value {
        self.context_with_options(&json!({}))
    }
    fn context_with_options(&self, args: &Value) -> Value {
        let w = self.workspace.lock().unwrap();
        let query = args["styleQuery"].as_str().unwrap_or("").to_lowercase();
        let offset = args["styleOffset"].as_u64().unwrap_or(0) as usize;
        let limit = args["styleLimit"].as_u64().unwrap_or(128).clamp(1, 128) as usize;
        let matching = w
            .project
            .materials
            .iter()
            .filter(|m| {
                query.is_empty()
                    || format!("{} {} {}", m.uid, m.name, m.family)
                        .to_lowercase()
                        .contains(&query)
            })
            .collect::<Vec<_>>();
        let styles=matching.iter().skip(offset).take(limit).map(|m|json!({"id":m.id,"uid":m.uid,"name":m.name,"family":m.family,"color":m.color,"renderMode":m.render_mode})).collect::<Vec<_>>();
        let catalog = json!({"total":w.project.materials.len(),"matched":matching.len(),"offset":offset,"limit":limit,"nextOffset":if offset.saturating_add(limit)<matching.len(){Some(offset+limit)}else{None},"query":query,"hint":"get_context accepts styleQuery/styleOffset/styleLimit; choose styles by ID or UID"});
        json!({"revision":w.project.scene.revision,"projectId":w.project.id,"projectName":w.project.name,"stats":w.project.scene.stats(&w.project.limits),"styles":styles,"catalog":catalog,"pinnedNotes":w.project.pinned_notes,"sync":self.transport.state(),"sdk":"mf.box(min,[w,h,d],{style:'wood',tint:0,motion:0}); mf.erase(min,size); mf.paint(min,size,{style}); mf.clear(); mf.group(name,()=>{}); mf.repeat(n,(i)=>{}); mf.mirror('x',()=>{}); mf.translate([x,y,z],()=>{}); mf.line(a,b,{thickness:1,style}); mf.stairs(origin,n,{width:3,depth:1,rise:1,style}); mf.wall(origin,w,h,{thickness:1,style}); mf.roof(origin,w,d,h,{style}); Math.random()/mf.random() deterministic by seed. All coordinates integer signed grid. Y is height. VRC target requires X/Z[-128,128),Y[0,256). Styles are catalog IDs/UID names. mode apply preserves current geometry; replace starts from empty."})
    }
    pub async fn tool(&self, name: &str, args: &Value, run: Option<&str>) -> Result<Value> {
        match name {
            "get_context" => Ok(self.context_with_options(args)),
            "run_recipe" => {
                let request: RecipeRequest = serde_json::from_value(args.clone())?;
                self.run_recipe(request, run).await
            }
            "capture_views" => {
                self.capture
                    .request(self.workspace.clone(), args, run)
                    .await
            }
            "inspect_region" => {
                let w = self.workspace.lock().unwrap();
                let mut scene = w.project.scene.clone();
                if let (Some(min), Some(size)) = (args.get("min"), args.get("size")) {
                    let min = serde_json::from_value(min.clone())?;
                    let size = serde_json::from_value(size.clone())?;
                    let region = morphforge_core::Box3::new(min, size, 0);
                    region.validate(&w.project.limits)?;
                    scene.boxes.retain(|b| b.intersects(&region));
                }
                if let Some(group) = args["group"].as_str() {
                    scene.boxes.retain(|b| b.group == group)
                }
                Ok(
                    json!({"ok":true,"revision":scene.revision,"stats":scene.stats(&w.project.limits),"sample":scene.boxes.iter().take(20).collect::<Vec<_>>()}),
                )
            }
            "commit" => {
                let commit = self.workspace.lock().unwrap().commit(
                    args["revision"].as_u64(),
                    args["title"].as_str().unwrap_or("AI提交"),
                    run,
                )?;
                Ok(json!({"ok":true,"commit":commit}))
            }
            "publish" => {
                let mut w = self.workspace.lock().unwrap();
                let rev = args["revision"]
                    .as_u64()
                    .unwrap_or(w.project.scene.revision);
                if !w.project.commits.iter().any(|c| c.revision == rev) {
                    bail!("请先commit再publish")
                };
                let scene = w.scene_at(rev)?;
                let state = self
                    .transport
                    .publish(&scene, args["resync"].as_bool().unwrap_or(false))?;
                w.event(
                    "revision.published",
                    run,
                    json!({"revision":rev,"state":state}),
                )?;
                Ok(state)
            }
            "sync_status" => Ok(self.transport.state()),
            "undo_to" => self.workspace.lock().unwrap().undo(
                false,
                Some(args["revision"].as_u64().context("缺少revision")?),
            ),
            "artworks" => {
                let mut w = self.workspace.lock().unwrap();
                match args["action"].as_str().unwrap_or("search") {
                    "search" => {
                        let query = args["query"].as_str().unwrap_or("").to_lowercase();
                        Ok(
                            json!({"ok":true,"artworks":w.library()?.into_iter().filter(|a|a.title.to_lowercase().contains(&query)||a.tags.iter().any(|t|t.to_lowercase().contains(&query))).collect::<Vec<_>>()}),
                        )
                    }
                    "save" => Ok(
                        json!({"ok":true,"artwork":w.save_artwork_views(args["title"].as_str().unwrap_or("新作品"),args["tags"].as_array().map(|a|a.iter().filter_map(|v|v.as_str().map(str::to_owned)).collect()).unwrap_or_default(),args["thumbnail"].as_str().map(str::to_owned),args["views"].as_array().map(|a|a.iter().filter_map(|v|v.as_str().map(str::to_owned)).collect()).unwrap_or_default(),run)?}),
                    ),
                    "load" | "fork" => w.load_artwork(args["id"].as_str().context("缺少作品ID")?),
                    _ => bail!("未知作品库操作"),
                }
            }
            "compact_context" => {
                self.inner.lock().unwrap().compact_requested = true;
                Ok(
                    json!({"ok":true,"scheduled":true,"note":"在当前完整工具链结束后执行，几何与原始日志不变"}),
                )
            }
            _ => bail!("未知工具 {name}"),
        }
    }
    pub async fn run_recipe(&self, request: RecipeRequest, run: Option<&str>) -> Result<Value> {
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
        let (base, styles) = {
            let w = self.workspace.lock().unwrap();
            let base = request.base_revision.unwrap_or(w.project.scene.revision);
            if base != w.project.scene.revision {
                bail!("REVISION_CONFLICT")
            };
            let mut styles = BTreeMap::new();
            for m in &w.project.materials {
                styles.insert(m.uid.clone(), m.id);
                styles.insert(m.uid.trim_start_matches("mf:").into(), m.id);
                styles.insert(m.name.clone(), m.id);
            }
            (base, styles)
        };
        let mut operations = recipe::execute(request.clone(), styles).await?;
        if request.mode == "replace" {
            operations.insert(0, morphforge_core::Operation::Clear)
        }
        let mut w = self.workspace.lock().unwrap();
        let value = if request.dry_run {
            let mut scene = w.project.scene.apply(&operations, &w.project.limits)?;
            scene.revision = base;
            json!({"ok":true,"dryRun":true,"revision":base,"stats":scene.stats(&w.project.limits)})
        } else {
            let value = w.apply(&operations, Some(base), request.request_id.as_deref(), run)?;
            let rev = w.project.scene.revision;
            w.recipes.insert(rev, request.code.clone());
            let artifact = w.artifact(request.code.as_bytes(), "js")?;
            w.event(
                "recipe.completed",
                run,
                json!({"seed":request.seed,"mode":request.mode,"artifact":artifact,"result":value}),
            )?;
            if self.transport.config().enabled && self.transport.config().auto {
                let _ = w.commit(None, "自动发布", run)?;
                let state = self.transport.publish(&w.project.scene, false)?;
                w.event("revision.published", run, state)?;
            }
            value
        };
        Ok(value)
    }
    async fn compact(&self, id: &str, force: bool) -> Result<()> {
        let (conversation, settings, cancel) = {
            let mut s = self.inner.lock().unwrap();
            s.compact_requested = false;
            (s.conversation.clone(), s.settings.clone(), s.cancel.clone())
        };
        let before = estimate(&conversation.input);
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
            let w = self.workspace.lock().unwrap();
            (
                w.project.pinned_notes.clone(),
                w.project.scene.revision,
                w.project.id.clone(),
            )
        };
        let result:Result<()> = async{
   self.workspace.lock().unwrap().event("compact.started",Some(id),json!({"beforeTokens":before,"messages":conversation.input.len(),"sourceMessages":cut,"revision":revision}))?;
   let payload=json!({"model":settings.model,"instructions":"你负责整理体素艺术会话的长期记忆。用中文保留用户要求、美术约束、关键决策、失败教训、未完成事项、作品引用。不要编造完成状态；几何以权威工程为准。只输出简洁但完整的交接摘要。","input":[{"role":"user","content":format!("固定要求:{}\n用户目标:{}\n此前摘要:{}\n历史:{}",pinned,serde_json::to_string(&conversation.goals)?,conversation.summary,serde_json::to_string(source)?)}],"reasoning":{"effort":"none"},"max_output_tokens":4096,"stream":true});
   let response=self.provider.response(payload,self.workspace.clone(),id,&cancel).await?;
   {let mut s=self.inner.lock().unwrap();record_run_tokens(&mut s,id,response["usage"]["total_tokens"].as_u64().unwrap_or(0));}
   if response["status"]!="completed"{bail!("compact响应未完整完成")}
   let summary=response["output"].as_array().context("compact没有output")?.iter().filter(|v|v["type"]=="message").flat_map(|v|v["content"].as_array().into_iter().flatten()).filter_map(|v|v["text"].as_str()).collect::<Vec<_>>().join("\n");if summary.trim().is_empty(){bail!("compact返回空摘要，旧上下文保留")}
   let manifest=json!({"type":"MorphForgeContextCompact/1","projectId":project_id,"revisionAtCompact":revision,"pinnedRequirements":pinned,"userGoals":conversation.goals,"summary":summary,"stateAuthority":"调用get_context重新读取当前工程/发布状态"});
   let mut input=vec![json!({"role":"user","content":serde_json::to_string(&manifest)?})];input.extend(tail);let after=estimate(&input);
   let (old,new)={let mut w=self.workspace.lock().unwrap();let old=w.artifact(serde_json::to_string(&conversation)?.as_bytes(),"json")?;let new=w.artifact(serde_json::to_string(&input)?.as_bytes(),"json")?;(old,new)};
   {
    let root=self.workspace.lock().unwrap().root.clone();let mut s=self.inner.lock().unwrap();
    if serde_json::to_value(&s.conversation)?!=serde_json::to_value(&conversation)?{bail!("COMPACT_CONFLICT：compact期间上下文已变化，旧上下文保留")}
    let previous=s.conversation.clone();s.conversation.input=input;s.conversation.summary=summary;s.conversation.compact_count+=1;s.conversation.boundaries=vec![s.conversation.input.len()];
    if let Err(error)=write_json(&root.join("runs/conversation.json"),&s.conversation){s.conversation=previous.clone();let _=write_json(&root.join("runs/conversation.json"),&previous);return Err(error)}
   }
   if let Ok(mut w)=self.workspace.lock(){let _=w.event("compact.completed",Some(id),json!({"beforeTokens":before,"afterTokens":after,"sourceMessages":cut,"beforeArtifact":old,"afterArtifact":new,"manifest":manifest}));}
   Ok(())
  }.await;
        if let Err(error) = &result {
            let message = self.provider.redact(&error.to_string());
            if let Ok(mut w) = self.workspace.lock() {
                let _=w.event("compact.failed",Some(id),json!({"error":message,"contextPreserved":true,"beforeTokens":before,"messages":conversation.input.len()}));
            }
        }
        result
    }
}
fn estimate(input: &[Value]) -> usize {
    serde_json::to_vec(input).map(|b| b.len() / 3).unwrap_or(0)
}
fn record_run_tokens(state: &mut State, id: &str, tokens: u64) {
    if let Some(run) = state.run.as_mut().filter(|r| r.id == id) {
        run.tokens = run.tokens.saturating_add(tokens);
    }
}
fn append_queued_prompts(state: &mut State) {
    let queued = std::mem::take(&mut state.queued);
    for prompt in queued {
        state.conversation.goals.push(prompt.clone());
        state
            .conversation
            .input
            .push(json!({"role":"user","content":prompt}));
    }
}
fn continue_after_success(state: &mut State, id: &str, stopped: bool, successful: bool) -> bool {
    if stopped || !successful || state.queued.is_empty() {
        return false;
    }
    if let Some(run) = state
        .run
        .as_mut()
        .filter(|r| r.id == id && ["running", "compacting"].contains(&r.status.as_str()))
    {
        run.status = "running".into();
        run.finished_at = None;
        return true;
    }
    false
}
pub fn tools() -> Value {
    fn tool(name: &str, description: &str, properties: Value, required: Vec<&str>) -> Value {
        json!({"type":"function","name":name,"description":description,"parameters":{"type":"object","properties":properties,"required":required,"additionalProperties":false}})
    }
    json!([
 tool("get_context","获取权威工程版本、范围、目标能力和SDK。材质默认最多128条，styleQuery搜索名称/UID/家族，styleOffset翻页。",json!({"styleQuery":{"type":"string"},"styleOffset":{"type":"integer","minimum":0},"styleLimit":{"type":"integer","minimum":1,"maximum":128}}),vec![]),
 tool("run_recipe","批量执行确定性JavaScript建模recipe。用函数/循环完成大形和局部修改，失败整次回滚。所有坐标整数；不要一盒一次工具调用。",json!({"code":{"type":"string"},"mode":{"type":"string","enum":["replace","apply"]},"baseRevision":{"type":"integer"},"seed":{"type":"integer"},"dryRun":{"type":"boolean"}}),vec!["code","mode","baseRevision","seed"]),
 tool("inspect_region","查询区域或分组摘要，不返回整个Box数组。",json!({"min":{"type":"array","items":{"type":"integer"}},"size":{"type":"array","items":{"type":"integer"}},"group":{"type":"string"}}),vec![]),
 tool("capture_views","对明确版本进行干净的固定多视角截图。修改大形后至少hero/front/top。",json!({"revision":{"type":"integer"},"views":{"type":"array","items":{"type":"string","enum":["hero","front","top","side","detail"]}},"width":{"type":"integer"},"height":{"type":"integer"}}),vec!["revision","views"]),
 tool("commit","固化已检查的草稿版本。",json!({"revision":{"type":"integer"},"title":{"type":"string"}}),vec!["revision","title"]),
 tool("publish","把已commit版本送往可选VRC队列。异步，不等待逐盒发完；没有开启发送时仅排目标。",json!({"revision":{"type":"integer"},"resync":{"type":"boolean"}}),vec!["revision"]),
 tool("sync_status","读取本机发送进度/ETA；ack=false不能声称远端已显示。",json!({}),vec![]),
 tool("undo_to","从历史revision恢复当前草稿，记录新的版本。",json!({"revision":{"type":"integer"}}),vec!["revision"]),
 tool("artworks","搜索、保存、加载或分支已生成作品；保存可用截图artifact作thumbnail。",json!({"action":{"type":"string","enum":["search","save","load","fork"]},"query":{"type":"string"},"title":{"type":"string"},"id":{"type":"string"},"tags":{"type":"array","items":{"type":"string"}},"thumbnail":{"type":"string"},"views":{"type":"array","items":{"type":"string"}}}),vec!["action"]),
 tool("compact_context","请求在完整工具链后整理模型上下文，保留全部原始日志、图片和作品。",json!({}),vec![])
 ])
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state_with_run(status: &str) -> State {
        State {
            settings: ProviderSettings::default(),
            conversation: Conversation::default(),
            run: Some(Run {
                id: "run-1".into(),
                status: status.into(),
                prompt: String::new(),
                started_at: now(),
                finished_at: None,
                steps: 0,
                tokens: 0,
                message: String::new(),
                error: String::new(),
            }),
            run_deadline: Some(tokio::time::Instant::now() + std::time::Duration::from_secs(900)),
            cancel: Cancellation::new(),
            paused: false,
            compact_requested: false,
            queued: vec![],
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
    fn successful_manual_compact_continues_when_prompt_was_queued() {
        let mut state = state_with_run("compacting");
        state.queued.push("继续搭建屋顶".into());
        assert!(continue_after_success(&mut state, "run-1", false, true));
        assert_eq!(state.run.as_ref().unwrap().status, "running");
        append_queued_prompts(&mut state);
        assert!(state.queued.is_empty());
        assert_eq!(state.conversation.goals, ["继续搭建屋顶"]);
        assert_eq!(state.conversation.input[0]["content"], "继续搭建屋顶");
    }

    #[test]
    fn failed_or_cancelled_compact_does_not_consume_queued_prompt() {
        let mut state = state_with_run("compacting");
        state.queued.push("继续".into());
        assert!(!continue_after_success(&mut state, "run-1", false, false));
        assert!(!continue_after_success(&mut state, "run-1", true, true));
        assert_eq!(state.queued, ["继续"]);
    }
}
