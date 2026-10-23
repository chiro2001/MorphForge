//! One bounded worker for all caption targets; only the latest revision may apply.
use super::{Speech, SpeechContext, Target};
use crate::provider::Cancellation;
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

struct Job {
    token: u64,
    generation: u64,
    lane: String,
    session: Option<String>,
    target: Target,
    source: String,
    input: String,
    model: String,
    language: String,
    persist: bool,
    key: String,
    ready: Instant,
    context: SpeechContext,
    cache_key: String,
    provider_identity: String,
    panel_text: String,
}
#[derive(Default)]
pub(super) struct Queue {
    token: u64,
    generation: u64,
    latest: BTreeMap<String, u64>,
    seen: BTreeMap<String, String>,
    pending: BTreeMap<String, Job>,
    running: bool,
    cancel: Option<Cancellation>,
    cache: VecDeque<(String, String)>,
    last_translation: BTreeMap<String, String>,
}
impl Speech {
    pub(super) fn invalidate_translation(&self) {
        let mut queue = self.translation.lock().unwrap();
        queue.generation += 1;
        queue.pending.clear();
        queue.latest.clear();
        queue.seen.clear();
        queue.last_translation.clear();
        if let Some(cancel) = queue.cancel.take() {
            cancel.stop();
        }
        let mut state = self.state.lock().unwrap();
        state["translation"] = json!({"phase":"idle","text":"","error":null});
        self.translation_notify.notify_one();
    }
    pub(super) fn queue_translation(
        self: &Arc<Self>,
        context: &SpeechContext,
        target: &Target,
        text: &str,
        persist: bool,
        key: &str,
        session: Option<&str>,
    ) -> Result<()> {
        let settings = self.settings.lock().unwrap().clone();
        let lane = format!("{}:{}", target.project_id, target.panel_id);
        if text.trim().is_empty() {
            {
                let mut queue = self.translation.lock().unwrap();
                queue.token += 1;
                let token = queue.token;
                queue.latest.insert(lane.clone(), token);
                queue.pending.remove(&lane);
                queue.seen.remove(&lane);
            }
            self.state.lock().unwrap()["translation"] =
                json!({"phase":"idle","text":"","error":null,"panelId":target.panel_id});
            return if persist {
                self.deliver(context, target, text, false, key)
            } else {
                let config = context.transport.config();
                if config.enabled && config.auto {
                    context
                        .transport
                        .latest_text(&target.panel_id, text)
                        .map(|_| ())
                } else {
                    Ok(())
                }
            };
        }
        let model = if settings.translation_model.trim().is_empty() {
            context.harness.inner.lock().unwrap().settings.model.clone()
        } else {
            settings.translation_model.trim().into()
        };
        let runtime = tokio::runtime::Handle::try_current()
            .ok()
            .or_else(|| self.runtime.lock().unwrap().clone())
            .context("字幕翻译运行时不可用")?;
        // Only a recent caption window leaves the machine, not the modeling conversation.
        let input = text
            .chars()
            .rev()
            .take(1200)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        let provider_identity = context.harness.provider.connection().identity();
        let panel = {
            let w = context.workspace.lock().unwrap();
            w.project
                .scene
                .text_panels
                .iter()
                .find(|p| p.id == target.panel_id)
                .cloned()
                .context("目标文本框已删除")?
        };
        if crate::text_panels::caption_rows(&panel) < 2 {
            anyhow::bail!("双语字幕至少需要两行高度，请增高字幕框；字号保持不变")
        }
        let panel_text = panel.text.clone();
        let cache_key = crate::store::hash(&serde_json::to_vec(&json!([
            provider_identity,
            model,
            settings.translation_language,
            input
        ]))?);
        let (launch, previous_translation) = {
            let mut queue = self.translation.lock().unwrap();
            {
                let current = self.settings.lock().unwrap();
                if !current.translation_enabled
                    || current.translation_language != settings.translation_language
                    || current.translation_model != settings.translation_model
                {
                    return Ok(());
                }
            }
            if !persist && queue.seen.get(&lane) == Some(&cache_key) {
                return Ok(());
            }
            queue.seen.insert(lane.clone(), cache_key.clone());
            queue.token += 1;
            let token = queue.token;
            let generation = queue.generation;
            queue.latest.insert(lane.clone(), token);
            let job = Job {
                token,
                generation,
                lane: lane.clone(),
                session: session.map(str::to_owned),
                target: target.clone(),
                source: text.into(),
                input,
                model: model.clone(),
                language: settings.translation_language.clone(),
                persist,
                key: key.into(),
                ready: Instant::now()
                    + if persist {
                        Duration::ZERO
                    } else {
                        Duration::from_millis(600)
                    },
                context: context.clone(),
                cache_key,
                provider_identity,
                panel_text: panel_text.clone(),
            };
            queue.pending.insert(lane, job);
            self.state.lock().unwrap()["translation"] = json!({"phase":"pending","panelId":target.panel_id,"language":settings.translation_language,"model":model,"error":null});
            let launch = !queue.running;
            queue.running = true;
            (
                launch,
                queue
                    .last_translation
                    .get(&format!("{}:{}", target.project_id, target.panel_id))
                    .cloned(),
            )
        };
        // Keep source speech live while the next translation is pending.
        // The previous accepted translation remains in its own reserved lines.
        {
            let config = context.transport.config();
            if config.enabled && config.auto {
                let display = if let Some(previous) = previous_translation {
                    crate::text_panels::bilingual_caption(&panel, text, &previous)
                        .unwrap_or_else(|_| text.to_string())
                } else {
                    text.to_string()
                };
                let w = context.workspace.lock().unwrap();
                if w.project.id == target.project_id
                    && w.project
                        .scene
                        .text_panels
                        .iter()
                        .any(|p| p.id == target.panel_id && p.text == panel_text)
                {
                    if let Err(error) = context.transport.latest_text(&target.panel_id, &display) {
                        self.state.lock().unwrap()["error"] = json!(error.to_string());
                    }
                }
            }
        }
        self.translation_notify.notify_one();
        if launch {
            let service = self.clone();
            runtime.spawn(async move {
                service.translation_worker().await;
            });
        }
        Ok(())
    }
    async fn translation_worker(self: Arc<Self>) {
        let mut last_request = None::<Instant>;
        loop {
            let (job, wait) = {
                let mut queue = self.translation.lock().unwrap();
                let next = queue
                    .pending
                    .iter()
                    .min_by_key(|(_, job)| job.ready)
                    .map(|(lane, job)| (lane.clone(), job.ready));
                match next {
                    None => {
                        queue.running = false;
                        queue.cancel = None;
                        (None, None)
                    }
                    Some((lane, ready)) => {
                        let due = last_request
                            .map(|last| ready.max(last + Duration::from_millis(900)))
                            .unwrap_or(ready);
                        if Instant::now() < due {
                            (None, Some(due))
                        } else {
                            (queue.pending.remove(&lane), None)
                        }
                    }
                }
            };
            if let Some(due) = wait {
                tokio::select! {_=tokio::time::sleep_until(due.into())=>{},_=self.translation_notify.notified()=>{}}
                continue;
            }
            let Some(job) = job else {
                return;
            };
            if job.context.workspace.lock().unwrap().project.id != job.target.project_id {
                self.state.lock().unwrap()["translation"] = json!({"phase":"error","error":"目标项目已切换，译文未应用","panelId":job.target.panel_id});
                continue;
            }
            if let Some(id) = &job.session {
                let state = self.state.lock().unwrap();
                if state["sessionId"] != *id
                    || matches!(
                        state["phase"].as_str(),
                        Some("cancelled" | "cancelling" | "error")
                    )
                {
                    continue;
                }
            }
            if job.context.harness.provider.connection().identity() != job.provider_identity {
                self.state.lock().unwrap()["translation"] = json!({"phase":"error","error":"大模型连接已变更，请重新应用字幕","panelId":job.target.panel_id});
                continue;
            }
            let cancel = Cancellation::new();
            let cached = {
                let mut queue = self.translation.lock().unwrap();
                if queue.generation != job.generation
                    || queue.latest.get(&job.lane) != Some(&job.token)
                {
                    continue;
                }
                queue.cancel = Some(cancel.clone());
                self.state.lock().unwrap()["translation"] = json!({"phase":"translating","panelId":job.target.panel_id,"language":job.language,"model":job.model,"error":null});
                queue
                    .cache
                    .iter()
                    .find(|(key, _)| key == &job.cache_key)
                    .map(|(_, text)| text.clone())
            };
            let start = Instant::now();
            let cache_hit = cached.is_some();
            let result = if let Some(text) = cached {
                Ok((text, Value::Null))
            } else {
                last_request = Some(Instant::now());
                {
                    let mut state = self.state.lock().unwrap();
                    let count = state["translationRequests"].as_u64().unwrap_or(0) + 1;
                    state["translationRequests"] = json!(count);
                }
                job.context
                    .harness
                    .provider
                    .translate_caption(&job.model, &job.language, &job.input, &cancel)
                    .await
                    .map(|r| (r.text, r.usage))
            };
            let mut queue = self.translation.lock().unwrap();
            if !cache_hit && queue.generation == job.generation {
                if let Ok((text, _)) = &result {
                    queue.cache.push_back((job.cache_key.clone(), text.clone()));
                    while queue.cache.len() > 16 {
                        queue.cache.pop_front();
                    }
                }
            }
            if queue.generation != job.generation || queue.latest.get(&job.lane) != Some(&job.token)
            {
                continue;
            }
            if let Some(id) = &job.session {
                let state = self.state.lock().unwrap();
                if state["sessionId"] != *id
                    || matches!(
                        state["phase"].as_str(),
                        Some("cancelled" | "cancelling" | "error")
                    )
                {
                    continue;
                }
            }
            if job.context.workspace.lock().unwrap().project.id != job.target.project_id {
                self.state.lock().unwrap()["translation"] = json!({"phase":"error","error":"目标项目已切换，译文未应用","panelId":job.target.panel_id});
                continue;
            }
            let (translated, usage, mut error) = match result {
                Ok((text, usage)) => (text, usage, None),
                Err(error) => (
                    job.source.clone(),
                    Value::Null,
                    Some(job.context.harness.provider.redact(&error.to_string())),
                ),
            };
            let current_panel = job
                .context
                .workspace
                .lock()
                .unwrap()
                .project
                .scene
                .text_panels
                .iter()
                .find(|p| p.id == job.target.panel_id)
                .cloned();
            let text = if error.is_none() {
                match current_panel
                    .as_ref()
                    .context("目标文本框已删除")
                    .and_then(|p| {
                        crate::text_panels::bilingual_caption(p, &job.source, &translated)
                    }) {
                    Ok(display) => display,
                    Err(e) => {
                        error = Some(e.to_string());
                        job.source.clone()
                    }
                }
            } else {
                job.source.clone()
            };
            let delivery = if job.persist {
                let suffix = if error.is_some() {
                    "fallback"
                } else {
                    "translated"
                };
                self.deliver_checked(
                    &job.context,
                    &job.target,
                    &text,
                    false,
                    &format!(
                        "{}-{suffix}-{}-{}",
                        job.key,
                        job.language,
                        &job.cache_key[..16]
                    ),
                    Some(&job.panel_text),
                )
            } else {
                let config = job.context.transport.config();
                if config.enabled && config.auto {
                    let w = job.context.workspace.lock().unwrap();
                    if w.project.id != job.target.project_id
                        || !w
                            .project
                            .scene
                            .text_panels
                            .iter()
                            .any(|p| p.id == job.target.panel_id && p.text == job.panel_text)
                    {
                        Err(anyhow::anyhow!("字幕已变更，旧译文未应用"))
                    } else {
                        job.context
                            .transport
                            .latest_text(&job.target.panel_id, &text)
                            .map(|_| ())
                    }
                } else {
                    Ok(())
                }
            };
            let error = error.or_else(|| {
                delivery
                    .err()
                    .map(|e| job.context.harness.provider.redact(&e.to_string()))
            });
            if error.is_none() {
                queue
                    .last_translation
                    .insert(job.lane.clone(), translated.clone());
            }
            let mut state = self.state.lock().unwrap();
            state["translation"] = json!({"phase":if error.is_some(){"error"}else{"ready"},"panelId":job.target.panel_id,"language":job.language,"model":job.model,"text":translated,"source":job.source,"displayText":text,"bilingual":error.is_none(),"error":error,"cacheHit":cache_hit,"latencyMs":start.elapsed().as_millis(),"usage":usage});
        }
    }
    /// Shared path for manually typed live captions; no recording is started.
    pub fn latest_panel(
        self: &Arc<Self>,
        context: &SpeechContext,
        id: &str,
        project: &str,
        text: &str,
    ) -> Result<()> {
        let target = Self::target(
            context,
            &json!({"kind":"text_panel","panelId":id,"projectId":project}),
        )?;
        if self.settings.lock().unwrap().translation_enabled {
            self.queue_translation(context, &target, text, false, "live", None)
        } else {
            context.transport.latest_text(id, text).map(|_| ())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::App;
    use axum::{routing::post, Json, Router};
    #[tokio::test]
    async fn recognition_callbacks_translate_final_caption_without_replacing_source_draft() {
        let requests = Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
        let rows = requests.clone();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let router=Router::new().route("/responses",post(move|Json(v):Json<Value>|{let rows=rows.clone();async move{rows.lock().unwrap().push(v.clone());let text=format!("訳：{}",v["input"][0]["content"][0]["text"].as_str().unwrap());Json(json!({"status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":text}]}]}))}}));
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let dir = tempfile::tempdir().unwrap();
        let app = App::open(dir.path().into(), None).unwrap();
        app.harness
            .configure_provider(
                &json!({"baseUrl":base,"apiType":"responses","apiKey":"local-only"}),
            )
            .unwrap();
        app.harness
            .tool(
                "text_panels",
                &json!({"action":"create","id":"caption","baseRevision":0,"preset":"caption"}),
                None,
            )
            .await
            .unwrap();
        app.speech
            .configure(
                &json!({"autoCaption":true,"translationEnabled":true,"translationLanguage":"ja"}),
            )
            .unwrap();
        let receiver = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        {
            let mut s = app.transport.inner.lock().unwrap();
            let mut catalog = crate::target::AvatarCatalog::from_bytes(include_bytes!(
                "../../../../materials/mf6/catalog.json"
            ))
            .unwrap();
            catalog
                .capabilities
                .push(crate::text_wire::CAPABILITY.into());
            catalog.capabilities.push("mf6-text-latest14/1".into());
            s.avatar_catalog = Some(catalog);
            s.config.enabled = true;
            s.config.auto = true;
            s.config.port = receiver.local_addr().unwrap().port();
            s.config.interval = 0.05;
        }
        let target = Target {
            kind: "text_panel".into(),
            panel_id: "caption".into(),
            project_id: app.workspace.lock().unwrap().project.id.clone(),
        };
        let context = SpeechContext {
            workspace: app.workspace.clone(),
            harness: app.harness.clone(),
            transport: app.transport.clone(),
        };
        *app.speech.state.lock().unwrap() = json!({"sessionId":"recognition-fixture","active":true,"phase":"recording","draft":"","sequence":0,"sentences":[]});
        let stop = std::sync::atomic::AtomicU8::new(0);
        app.speech.partial(
            "recognition-fixture",
            "こんにちは".into(),
            &target,
            &context,
        );
        app.speech.publish(
            "recognition-fixture",
            "こんにちは".into(),
            1.,
            None,
            &target,
            &context,
            &stop,
        );
        async fn ready(app: &App) {
            tokio::time::timeout(Duration::from_secs(5), async {
                while app.speech.status()["state"]["translation"]["phase"] != "ready" {
                    let status = app.speech.status();
                    assert_ne!(status["state"]["translation"]["phase"], "error", "{status}");
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
        }
        ready(&app).await;
        app.speech
            .partial("recognition-fixture", "世界".into(), &target, &context);
        // The pending display must already contain fresh source and the previous
        // accepted translation, without waiting for or billing another response.
        fn queued_text(app: &App) -> String {
            let mut replay = app.transport.inner.lock().unwrap().text.clone();
            replay.replay();
            let mut glyphs = [0u16; 128];
            let mut count = 0;
            while let Some(message) = replay.next() {
                match message.0[0] {
                    3 => glyphs.fill(0),
                    2 => {
                        let page = message.0[2] as usize;
                        for (i, bytes) in message.0[3..27].chunks_exact(2).enumerate() {
                            if page * 12 + i < 128 {
                                glyphs[page * 12 + i] = u16::from_le_bytes([bytes[0], bytes[1]]);
                            }
                        }
                    }
                    1 => count = message.0[24] as usize,
                    _ => {}
                }
                replay.commit(&message);
            }
            if let Some(snapshot) = replay.next_latest() {
                count = (snapshot.data[28] - 128) as usize;
                for (i, bytes) in snapshot.data[..28].chunks_exact(2).enumerate() {
                    glyphs[i] = u16::from_le_bytes([bytes[0], bytes[1]]);
                }
            }
            String::from_utf16_lossy(&glyphs[..count])
        }
        assert_eq!(queued_text(&app), "こんにちは\n世界\n訳：こんにちは");
        assert_eq!(requests.lock().unwrap().len(), 1);
        app.speech.publish(
            "recognition-fixture",
            "世界".into(),
            1.,
            None,
            &target,
            &context,
            &stop,
        );
        {
            let mut state = app.speech.state.lock().unwrap();
            state["active"] = json!(false);
            state["phase"] = json!("idle");
        }
        ready(&app).await;
        assert_eq!(app.speech.status()["state"]["draft"], "こんにちは\n世界");
        assert_eq!(
            app.workspace.lock().unwrap().project.scene.text_panels[0].text,
            "こんにちは\n世界\n訳：こんにちは\n世界"
        );
        assert_eq!(queued_text(&app), "こんにちは\n世界\n訳：こんにちは\n世界");
        assert_eq!(requests.lock().unwrap().len(), 2);
        assert!(app.harness.state()["run"].is_null());
        app.transport.inner.lock().unwrap().stop = true;
        server.abort();
    }
}
