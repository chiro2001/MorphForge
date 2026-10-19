use super::*;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalProgress {
    pub id: String,
    pub project_id: String,
    pub objective: String,
    pub criteria: String,
    pub status: String,
    pub min_review_rounds: usize,
    pub reviewed_geometry_hashes: Vec<String>,
    pub continuations: u32,
    pub no_progress_attempts: u32,
    pub last_progress_signature: String,
    pub assessment: String,
}

pub(super) fn geometry_hash(workspace: &Workspace) -> String {
    crate::store::hash(&serde_json::to_vec(&workspace.project.scene.boxes).unwrap_or_default())
}

impl Harness {
    pub fn configure_provider(&self, value: &Value) -> Result<Value> {
        let state = self.inner.lock().unwrap();
        if is_active_phase(&state.phase) || state.worker_epoch.is_some() {
            bail!("请先停止当前运行，再更换Provider连接（暂停时也不能更换）")
        }
        self.provider.configure(value)
    }

    pub(super) fn record_goal_capture(
        &self,
        project_id: &str,
        signature: String,
        result: &Value,
    ) -> Result<()> {
        if result["ok"] != true || result["images"].as_array().is_none_or(Vec::is_empty) {
            return Ok(());
        }
        let mut state = self.inner.lock().unwrap();
        if !state.settings.goal_mode {
            return Ok(());
        }
        if let Some(goal) = state
            .conversation
            .goal
            .as_mut()
            .filter(|g| g.project_id == project_id && g.status == "active")
        {
            if !goal.reviewed_geometry_hashes.contains(&signature) {
                goal.reviewed_geometry_hashes.push(signature);
                self.workspace.lock().unwrap().event("goal.reviewed", None, json!({"goalId":goal.id,"revision":result["revision"],"reviewRounds":goal.reviewed_geometry_hashes.len(),"required":goal.min_review_rounds}))?;
            }
        }
        drop(state);
        self.persist()
    }

    pub(super) fn continue_goal(&self, guard: &RunGuard) -> Result<bool> {
        let mut state = self.inner.lock().unwrap();
        if !guard_matches_locked(&state, guard) {
            bail!("RUN_CANCELLED")
        }
        if !state.settings.goal_mode {
            return Ok(false);
        }
        let max_continuations = state.settings.goal_max_continuations;
        let language = state.settings.language.clone();
        let mut workspace = self.workspace.lock().unwrap();
        let Some(goal) = state
            .conversation
            .goal
            .as_mut()
            .filter(|g| g.status == "active")
        else {
            return Ok(false);
        };
        if goal.project_id != workspace.project.id {
            bail!("GOAL_BLOCKED：工程已改变，请在当前工程输入新的目标")
        }
        let signature = format!(
            "{}:{}",
            geometry_hash(&workspace),
            goal.reviewed_geometry_hashes.len()
        );
        if goal.last_progress_signature == signature {
            goal.no_progress_attempts += 1;
        } else {
            goal.no_progress_attempts = 0;
        }
        goal.last_progress_signature = signature;
        if goal.continuations >= max_continuations || goal.no_progress_attempts >= 3 {
            goal.status = "blocked".into();
            goal.assessment = "达到续作上限或连续三次没有几何/复核进展；等待用户调整要求。".into();
            workspace.event("goal.blocked", Some(&guard.id), json!(&goal))?;
            bail!("GOAL_BLOCKED：达到续作上限或连续三次无进展，草稿和日志已保留")
        }
        goal.continuations += 1;
        workspace.event("goal.continued", Some(&guard.id), json!(&goal))?;
        let reminder = if language == "en" {
            format!("The creation goal is still active. Continue using tools toward: {}. Criteria: {}. Distinct geometry capture reviews: {}/{}. Inspect concrete weaknesses and improve the current draft, then use finish_artwork with goalAssessment. Do not restart or claim success in plain text. If blocked, explain the specific obstacle. All work remains within this run's remaining budget.",goal.objective,goal.criteria,goal.reviewed_geometry_hashes.len(),goal.min_review_rounds)
        } else {
            format!("创作目标仍在进行：{}。验收要求：{}。不同几何的截图复核轮数：{}/{}。请继续调用工具，检查具体短板并改进当前草稿，然后用finish_artwork和goalAssessment收尾；不要重建已完成部分或只用文字宣告完成。确有阻碍请说明具体原因。续作共用本run剩余预算。",goal.objective,goal.criteria,goal.reviewed_geometry_hashes.len(),goal.min_review_rounds)
        };
        state
            .conversation
            .input
            .push(json!({"role":"user","content":reminder}));
        state.queue_turn_complete = false;
        drop(workspace);
        drop(state);
        self.persist()?;
        Ok(true)
    }
}

pub(super) fn check_goal_finish(state: &State, workspace: &Workspace, args: &Value) -> Result<()> {
    if !state.settings.goal_mode {
        return Ok(());
    }
    if let Some(goal) = state
        .conversation
        .goal
        .as_ref()
        .filter(|g| g.status == "active")
    {
        if goal.project_id != workspace.project.id {
            bail!("GOAL_PROJECT_CONFLICT：目标工程已变化")
        }
        let signature = geometry_hash(workspace);
        if goal.reviewed_geometry_hashes.len() < goal.min_review_rounds
            || !goal.reviewed_geometry_hashes.contains(&signature)
        {
            bail!("GOAL_REVIEW_REQUIRED：需要{}轮不同几何的真实截图复核，当前{}轮；先修改再capture_views，并捕获当前几何",goal.min_review_rounds,goal.reviewed_geometry_hashes.len())
        }
        if args["goalAssessment"]
            .as_str()
            .is_none_or(|s| s.trim().is_empty() || s.len() > 16000)
        {
            bail!("GOAL_ASSESSMENT_REQUIRED：请用goalAssessment逐项对照目标与限制（最多16000字节）")
        }
    }
    Ok(())
}
