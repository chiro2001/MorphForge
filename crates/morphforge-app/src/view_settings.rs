use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::path::Path;
pub fn defaults() -> Value {
    json!({"quality":"high","captureQuality":"high","voxelGrid":false,"captureGrid":false,"surfaceCulling":true,"showTargetBounds":false})
}
pub fn load(root: &Path) -> Result<Value> {
    let p = root.join("view-settings.json");
    if p.exists() {
        patch(&defaults(), &serde_json::from_slice(&std::fs::read(p)?)?)
    } else {
        Ok(defaults())
    }
}
pub fn patch(current: &Value, changes: &Value) -> Result<Value> {
    let mut result = current.clone();
    let fields = changes
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("显示设置须为对象"))?;
    for (k, v) in fields {
        match k.as_str() {
            "quality" | "captureQuality" if matches!(v.as_str(), Some("high" | "standard")) => {}
            "voxelGrid" | "captureGrid" | "surfaceCulling" | "showTargetBounds"
                if v.is_boolean() => {}
            _ => bail!("未知或无效的显示设置：{k}"),
        }
        result[k] = v.clone();
    }
    Ok(result)
}
