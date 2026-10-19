//! Explicit workspace transitions; never generate model requests or publish geometry.
use super::*;
use crate::store::{hash, Project};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Session {
    conversation: Conversation,
    run: Option<Run>,
    provider_state: Value,
}
fn session_path(root: &Path, id: &str) -> Result<PathBuf> {
    uuid::Uuid::parse_str(id).context("无效的对话ID")?;
    Ok(root.join("runs/sessions").join(format!("{id}.json")))
}
fn active_path(root: &Path, project_id: &str) -> PathBuf {
    root.join("projects").join(format!(
        "{}.active-session.json",
        hash(project_id.as_bytes())
    ))
}
pub(super) fn save_conversation(w: &Workspace, conversation: &Conversation) -> Result<()> {
    write_json(&w.root.join("runs/conversation.json"), conversation)?;
    write_json(
        &active_path(&w.root, &w.project.id),
        &json!({"id":conversation.id}),
    )
}
fn fresh(project_id: &str, title: &str) -> Conversation {
    Conversation {
        id: uuid::Uuid::new_v4().to_string(),
        project_id: project_id.into(),
        title: title.into(),
        schema_tools: TOOL_SCHEMA_VERSION.into(),
        schema_prompt: SYSTEM_PROMPT_VERSION.into(),
        created_at: now(),
        ..Default::default()
    }
}
fn session_summary(c: &Conversation) -> Value {
    let title = if c.title.is_empty() {
        c.goals.first().map(String::as_str).unwrap_or("新对话")
    } else {
        &c.title
    };
    json!({"id":c.id,"projectId":c.project_id,"title":title.chars().take(64).collect::<String>(),"createdAt":c.created_at,"messages":c.input.len(),"compactCount":c.compact_count})
}

// A journal makes project + conversation + cache state recoverable across process failure.
pub(crate) fn recover(root: &Path) -> Result<()> {
    let path = root.join("workspace-transition.json");
    if !path.exists() {
        return Ok(());
    }
    let value: Value = serde_json::from_slice(&fs::read(&path)?)?;
    let project: Project = serde_json::from_value(value["project"].clone())?;
    let session: Session = serde_json::from_value(value["session"].clone())?;
    project.scene.validate(&project.limits)?;
    if project.id != session.conversation.project_id {
        bail!("工作区恢复记录不匹配");
    }
    write_json(&root.join("project.json"), &project)?;
    write_json(&root.join("runs/conversation.json"), &session.conversation)?;
    write_json(&session_path(root, &session.conversation.id)?, &session)?;
    write_json(
        &active_path(root, &project.id),
        &json!({"id":session.conversation.id}),
    )?;
    write_json(
        &root.join("provider-prefix.json"),
        &session.provider_state["prefix"],
    )?;
    write_json(
        &root.join("cache-monitor.json"),
        &session.provider_state["guard"],
    )?;
    fs::remove_file(path)?;
    Ok(())
}
impl Harness {
    fn archive_session(&self, w: &Workspace, state: &State) -> Result<()> {
        let session = Session {
            conversation: state.conversation.clone(),
            run: state.run.clone(),
            provider_state: self.provider.context_guard_snapshot()?,
        };
        write_json(&session_path(&w.root, &state.conversation.id)?, &session)?;
        save_conversation(w, &state.conversation)
    }
    pub fn initialize_workspace(&self) -> Result<()> {
        let state = self.inner.lock().unwrap();
        let w = self.workspace.lock().unwrap();
        if w.project.id != state.conversation.project_id {
            bail!("工程与对话不匹配，请检查本地数据");
        }
        self.archive_session(&w, &state)
    }
    pub fn workspaces(&self) -> Result<Value> {
        let state = self.inner.lock().unwrap();
        let w = self.workspace.lock().unwrap();
        let mut projects = vec![];
        for entry in fs::read_dir(w.root.join("projects"))? {
            let path = entry?.path();
            if !path.to_string_lossy().ends_with(".project.json") {
                continue;
            }
            let project: Project = serde_json::from_slice(&fs::read(path)?)?;
            projects.push(json!({"id":project.id,"name":project.name,"revision":project.scene.revision,"boxCount":project.scene.boxes.len()}));
        }
        let mut sessions = vec![];
        for entry in fs::read_dir(w.root.join("runs/sessions"))? {
            let path = entry?.path();
            if path.extension().is_none_or(|x| x != "json") {
                continue;
            }
            let session: Session = serde_json::from_slice(&fs::read(path)?)?;
            let c = &session.conversation;
            if c.project_id == w.project.id && c.id != state.conversation.id {
                sessions.push(session_summary(c));
            }
        }
        sessions.push(session_summary(&state.conversation));
        sessions.sort_by(|a, b| b["createdAt"].as_str().cmp(&a["createdAt"].as_str()));
        projects.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        Ok(
            json!({"projectId":w.project.id,"sessionId":state.conversation.id,"projects":projects,"sessions":sessions}),
        )
    }
    pub fn workspace_action(&self, request: &Value) -> Result<Value> {
        let action = request["action"].as_str().context("缺少工作区操作")?;
        if !["new_project", "open_project", "new_session", "open_session"].contains(&action) {
            bail!("未知工作区操作");
        }
        let mut state = self.inner.lock().unwrap();
        if is_active_phase(&state.phase)
            || state.worker_epoch.is_some()
            || !state.queued.is_empty()
            || !state.pending_steers.is_empty()
            || !state.pending_interrupts.is_empty()
        {
            bail!("RUN_CONFLICT：请先停止AI运行并清空排队消息，再切换工程或对话");
        }
        let mut w = self.workspace.lock().unwrap();
        if request["baseProjectId"].as_str() != Some(&w.project.id)
            || request["baseSessionId"].as_str() != Some(&state.conversation.id)
        {
            bail!("WORKSPACE_CONFLICT：工作区已切换，请刷新后重试");
        }
        let name = request["name"].as_str().unwrap_or("").trim();
        if name.len() > 256 {
            bail!("名称不能超过256字节");
        }
        let project = if action == "new_project" {
            Project {
                name: if name.is_empty() {
                    "未命名作品".into()
                } else {
                    name.into()
                },
                ..Default::default()
            }
        } else if action == "open_project" {
            let id = request["id"].as_str().context("缺少工程ID")?;
            if id == w.project.id {
                return Ok(json!({"ok":true,"unchanged":true}));
            }
            let p: Project = serde_json::from_slice(
                &fs::read(w.project_archive_path(id)).context("找不到指定工程")?,
            )?;
            if p.id != id {
                bail!("工程存档ID不匹配");
            }
            p
        } else {
            w.project.clone()
        };
        let target_session = if action == "open_session" {
            Some(request["id"].as_str().context("缺少对话ID")?.to_owned())
        } else if action == "open_project" {
            let p = active_path(&w.root, &project.id);
            if p.exists() {
                let v: Value = serde_json::from_slice(&fs::read(p)?)?;
                Some(v["id"].as_str().context("工程对话索引损坏")?.into())
            } else {
                None
            }
        } else {
            None
        };
        if target_session.as_deref() == Some(&state.conversation.id) && project.id == w.project.id {
            return Ok(json!({"ok":true,"unchanged":true}));
        }
        let mut next = if let Some(id) = target_session {
            let next: Session = serde_json::from_slice(
                &fs::read(session_path(&w.root, &id)?).context("找不到指定对话")?,
            )?;
            if next.conversation.project_id != project.id || next.conversation.id != id {
                bail!("对话不属于当前工程");
            }
            next
        } else {
            Session {
                conversation: fresh(&project.id, if action == "new_session" { name } else { "" }),
                run: None,
                provider_state: json!({"prefix":{},"guard":{}}),
            }
        };
        if let Some(ref run) = next.run {
            if next.conversation.schema_tools.is_empty() {
                next.conversation.schema_tools = run.tools_version.clone();
            }
            if next.conversation.schema_prompt.is_empty() {
                next.conversation.schema_prompt = run.prompt_version.clone();
            }
        }
        Self::hydrate_versions(&mut next.conversation, &w.events);
        // Validate restored state before changing the durable workspace pointer.
        self.provider.validate_context_guard(&next.provider_state)?;
        self.archive_session(&w, &state)?;
        w.persist()?;
        write_json(
            &w.root.join("workspace-transition.json"),
            &json!({"project":project,"session":next}),
        )?;
        self.transport
            .configure(&json!({"enabled":false,"auto":false}), &mut w)?;
        if project.id != w.project.id {
            w.activate_project(project)?;
        }
        save_conversation(&w, &next.conversation)?;
        self.provider
            .restore_context_guard(Some(&next.provider_state))?;
        state.cancel.stop();
        state.epoch = state.epoch.saturating_add(1);
        state.conversation = next.conversation;
        state.run = next.run;
        state.phase = if state.run.is_some() {
            "stopped".into()
        } else {
            "idle".into()
        };
        state.paused = false;
        state.run_deadline = None;
        state.active_budget_remaining = None;
        state.active_request_id = None;
        state.compact_requested = false;
        state.queue_turn_complete = true;
        state.message_acks.clear();
        w.session_id = state.conversation.id.clone();
        self.archive_session(&w, &state)?;
        let changed = json!({"action":action,"projectId":w.project.id,"sessionId":state.conversation.id,"oscDisconnected":true});
        w.event("workspace.changed", None, changed)?;
        fs::remove_file(w.root.join("workspace-transition.json"))?;
        Ok(
            json!({"ok":true,"projectId":w.project.id,"sessionId":state.conversation.id,"eventSeq":w.next_seq-1}),
        )
    }
}
