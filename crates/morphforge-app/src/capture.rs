use crate::store::Workspace;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::oneshot;
#[derive(Clone)]
pub struct CaptureBroker {
    jobs: Arc<Mutex<HashMap<String, Job>>>,
}
struct Job {
    value: Value,
    claimed: bool,
    tx: oneshot::Sender<std::result::Result<Value, String>>,
}
struct PendingJobCleanup {
    jobs: Arc<Mutex<HashMap<String, Job>>>,
    id: String,
}
impl Drop for PendingJobCleanup {
    fn drop(&mut self) {
        if let Ok(mut jobs) = self.jobs.lock() {
            jobs.remove(&self.id);
        }
    }
}
impl CaptureBroker {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    pub async fn request(
        &self,
        workspace: Arc<Mutex<Workspace>>,
        options: &Value,
        run: Option<&str>,
    ) -> Result<Value> {
        let width = options
            .get("width")
            .and_then(Value::as_u64)
            .unwrap_or(768)
            .clamp(128, 2048);
        let height = options
            .get("height")
            .and_then(Value::as_u64)
            .unwrap_or(640)
            .clamp(128, 2048);
        let views = options
            .get("views")
            .cloned()
            .unwrap_or(json!(["hero", "front", "top"]));
        if !views.as_array().is_some_and(|a| {
            !a.is_empty()
                && a.len() <= 5
                && a.iter().all(|x| {
                    ["hero", "front", "top", "side", "detail"].contains(&x.as_str().unwrap_or(""))
                })
        }) {
            bail!("无效截图视角")
        }
        let id = uuid::Uuid::new_v4().to_string();
        let value = {
            let w = workspace.lock().unwrap();
            let rev = options
                .get("revision")
                .and_then(Value::as_u64)
                .unwrap_or(w.project.scene.revision);
            json!({"id":id,"revision":rev,"scene":w.scene_at(rev)?,"materials":w.materials_at(rev)?,"views":views,"width":width,"height":height,"mode":options.get("mode").and_then(Value::as_str).unwrap_or("art")})
        };
        let (tx, rx) = oneshot::channel();
        self.jobs.lock().unwrap().insert(
            id.clone(),
            Job {
                value,
                claimed: false,
                tx,
            },
        );
        let _cleanup = PendingJobCleanup {
            jobs: self.jobs.clone(),
            id: id.clone(),
        };
        let result = tokio::time::timeout(Duration::from_secs(45), rx).await;
        let value = result
            .context("CAPTURE_TIMEOUT: 截图渲染器未响应，请检查浏览器worker状态")??
            .map_err(anyhow::Error::msg)?;
        workspace
            .lock()
            .unwrap()
            .event("capture.completed", run, value.clone())?;
        Ok(value)
    }
    pub fn claim(&self) -> Value {
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(job) = jobs.values_mut().find(|j| !j.claimed) {
            job.claimed = true;
            job.value.clone()
        } else {
            Value::Null
        }
    }
    pub fn complete(&self, workspace: &mut Workspace, v: &Value) -> Result<Value> {
        use base64::Engine;
        let id = v["id"].as_str().context("缺少截图请求ID")?;
        let job = self
            .jobs
            .lock()
            .unwrap()
            .remove(id)
            .context("截图请求已失效")?;
        if v.get("error").is_some() {
            let e = v["error"].as_str().unwrap_or("截图失败").to_owned();
            let _ = job.tx.send(Err(e.clone()));
            bail!(e)
        }
        if v["revision"] != job.value["revision"] {
            let _ = job.tx.send(Err("截图版本不匹配".into()));
            bail!("截图版本不匹配")
        }
        let images = v["images"].as_array().context("缺少截图")?;
        let expected = job.value["views"].as_array().unwrap();
        if images.len() != expected.len() {
            let _ = job.tx.send(Err("截图数量不匹配".into()));
            bail!("截图数量不匹配")
        }
        let mut artifacts = vec![];
        for image in images {
            let view = image["view"].as_str().context("图片无视角")?;
            if !expected.iter().any(|x| x.as_str() == Some(view)) {
                bail!("截图视角不匹配")
            };
            let data = image["data"]
                .as_str()
                .context("图片缺少data URI")?
                .strip_prefix("data:image/png;base64,")
                .context("只支持PNG截图")?;
            if data.len() > 24 * 1024 * 1024 {
                bail!("截图过大")
            };
            let bytes = base64::engine::general_purpose::STANDARD.decode(data)?;
            if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                bail!("无效PNG")
            };
            let name = workspace.artifact(&bytes, "png")?;
            artifacts.push(json!({"view":view,"artifact":name,"url":format!("/artifacts/{name}"),"width":job.value["width"],"height":job.value["height"],"camera":image.get("camera")}));
        }
        let result = json!({"ok":true,"id":id,"revision":job.value["revision"],"images":artifacts,"stats":job.value["scene"].get("boxes").and_then(Value::as_array).map(|a|json!({"boxCount":a.len()}))});
        let _ = job.tx.send(Ok(result.clone()));
        Ok(result)
    }
}

impl Default for CaptureBroker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancelling_capture_request_removes_pending_job() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = Arc::new(Mutex::new(
            Workspace::open(dir.path().to_path_buf()).unwrap(),
        ));
        let broker = CaptureBroker::new();
        let worker = broker.clone();
        let request =
            tokio::spawn(async move { worker.request(workspace, &json!({}), None).await });
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if !broker.jobs.lock().unwrap().is_empty() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("capture request should register its job before waiting");
        request.abort();
        let _ = request.await;
        assert!(
            broker.jobs.lock().unwrap().is_empty(),
            "cancelled capture must not remain claimable"
        );
    }
}
