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
pub fn render_options(options: &Value) -> Result<(&str, &str, u64)> {
    let mode = options["mode"].as_str().unwrap_or("art");
    let quality = options["quality"].as_str().unwrap_or("high");
    let supersampling = if let Some(value) = options.get("supersampling") {
        value.as_u64().context("supersampling须为1..3的整数")?
    } else if quality == "high" {
        2
    } else {
        1
    };
    if !["art", "vrc"].contains(&mode)
        || !["standard", "high"].contains(&quality)
        || !(1..=3).contains(&supersampling)
    {
        bail!("截图模式或画质参数无效");
    }
    Ok((mode, quality, supersampling))
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
        let (mode, quality, supersampling) = render_options(options)?;
        let (views, shots) = view_options(options)?;
        let id = uuid::Uuid::new_v4().to_string();
        let value = {
            let w = workspace.lock().unwrap();
            let rev = options
                .get("revision")
                .and_then(Value::as_u64)
                .unwrap_or(w.project.scene.revision);
            json!({"id":id,"revision":rev,"scene":w.scene_at(rev)?,"materials":w.materials_at(rev)?,"views":views,"shots":shots,"width":width,"height":height,"mode":mode,"quality":quality,"supersampling":supersampling,"voxelGrid":options["voxelGrid"].as_bool().unwrap_or(false),"surfaceCulling":options["surfaceCulling"].as_bool().unwrap_or(true),"surfaces":morphforge_core::surfaces::transparent_interfaces(&w.scene_at(rev)?,&w.materials_at(rev)?)})
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
        let mut returned = std::collections::BTreeSet::new();
        for image in images {
            let view = image["view"].as_str().context("图片无视角")?;
            if !returned.insert(view) || !expected.iter().any(|x| x.as_str() == Some(view)) {
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
            artifacts.push(json!({"view":view,"artifact":name,"url":format!("/artifacts/{name}"),"width":job.value["width"],"height":job.value["height"],"camera":image.get("camera"),"rendering":image.get("rendering")}));
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

pub fn view_options(options: &Value) -> Result<(Value, Value)> {
    let shots = options
        .get("shots")
        .filter(|v| !v.is_null())
        .cloned()
        .unwrap_or(json!([]));
    if !shots.is_array() {
        bail!("shots必须是数组")
    }
    if !shots.as_array().unwrap().is_empty() {
        validate_shots(&shots)?;
    }
    let defaults = if shots.as_array().unwrap().is_empty() {
        json!(["hero", "front", "top"])
    } else {
        json!([])
    };
    let views = options
        .get("views")
        .filter(|v| !v.is_null())
        .cloned()
        .unwrap_or(defaults);
    let views = views.as_array().context("views必须是数组")?;
    let mut names = Vec::new();
    for view in views {
        if !["hero", "front", "top", "side", "detail"].contains(&view.as_str().unwrap_or("")) {
            bail!("无效预设视角")
        }
        if !names.contains(view) {
            names.push(view.clone());
        }
    }
    for shot in shots.as_array().unwrap() {
        if !names.contains(&shot["id"]) {
            names.push(shot["id"].clone());
        }
    }
    if names.is_empty() || names.len() > 5 {
        bail!("每次截图需要1至5个不同视角")
    }
    Ok((json!(names), shots))
}

fn validate_shots(value: &Value) -> Result<()> {
    let shots = value.as_array().context("shots必须是数组")?;
    if shots.is_empty() || shots.len() > 5 {
        bail!("每次截图需要1至5个shots")
    }
    let mut ids = std::collections::BTreeSet::new();
    for shot in shots {
        let id = shot["id"].as_str().context("shot需要id")?;
        if id.is_empty()
            || id.len() > 64
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || !ids.insert(id)
        {
            bail!("shot id无效或重复")
        }
        if shot
            .get("projection")
            .is_some_and(|p| !matches!(p.as_str(), Some("perspective" | "orthographic")))
        {
            bail!("无效相机投影")
        }
        for (key, min, max) in [
            ("yaw", -360., 360.),
            ("pitch", -90., 90.),
            ("margin", 0., 1.),
            ("fov", 10., 120.),
        ] {
            if let Some(v) = shot.get(key) {
                if !v
                    .as_f64()
                    .is_some_and(|v| v.is_finite() && v >= min && v <= max)
                {
                    bail!("相机参数越界：{key}")
                }
            }
        }
        for key in ["position", "target"] {
            if let Some(v) = shot.get(key) {
                if !valid_point(v) {
                    bail!("相机坐标需要三个有限数：{key}")
                }
            }
        }
        if shot.get("position").is_some() && shot["position"] == shot["target"] {
            bail!("相机position和target不能相同")
        }
        if let Some(fit) = shot.get("fit") {
            if let Some(group) = fit.get("group") {
                if !group
                    .as_str()
                    .is_some_and(|s| !s.is_empty() && s.len() <= 128)
                {
                    bail!("无效取景分组")
                }
            }
            if let Some(bounds) = fit.get("bounds") {
                if !valid_point(&bounds["min"]) || !valid_point(&bounds["max"]) {
                    bail!("无效取景bounds")
                }
                for i in 0..3 {
                    if bounds["min"][i].as_f64().unwrap() >= bounds["max"][i].as_f64().unwrap() {
                        bail!("取景bounds必须有正体积")
                    }
                }
            }
        }
    }
    Ok(())
}
fn valid_point(value: &Value) -> bool {
    value.as_array().is_some_and(|a| {
        a.len() == 3
            && a.iter().all(|v| {
                v.as_f64()
                    .is_some_and(|n| n.is_finite() && n.abs() <= 1_000_000.)
            })
    })
}
#[cfg(test)]
mod shot_tests {
    use super::*;
    #[test]
    fn validates_free_camera_and_rejects_duplicates_or_bad_numbers() {
        assert!(validate_shots(&json!([{"id":"face","fit":{"group":"face"},"projection":"orthographic","yaw":0,"pitch":0}])).is_ok());
        assert!(validate_shots(&json!([{"id":"a"},{"id":"a"}])).is_err());
        assert!(validate_shots(&json!([{"id":"a","pitch":91}])).is_err());
    }
    #[test]
    fn presets_mix_with_custom_shots_and_same_name_overrides_without_duplicates() {
        let (views,shots)=view_options(&json!({"views":["hero","front"],"shots":[{"id":"front","projection":"orthographic"},{"id":"fin","fit":{"group":"fins"}}]})).unwrap();
        assert_eq!(views, json!(["hero", "front", "fin"]));
        assert_eq!(shots.as_array().unwrap().len(), 2);
    }
}
