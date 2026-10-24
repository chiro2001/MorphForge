use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::path::Path;

pub fn defaults() -> Value { json!({"language":"auto"}) }
pub fn load(root: &Path) -> Result<Value> {
    let path = root.join("ui-settings.json");
    if path.exists() { patch(&defaults(), &serde_json::from_slice(&std::fs::read(path)?)?) }
    else { Ok(defaults()) }
}
pub fn patch(current: &Value, changes: &Value) -> Result<Value> {
    let mut result = current.clone();
    let fields = changes.as_object().ok_or_else(|| anyhow::anyhow!("UI settings must be an object"))?;
    for (key, value) in fields {
        if key != "language" || !matches!(value.as_str(), Some("auto" | "zh-CN" | "ja" | "en")) {
            bail!("Unsupported UI setting: {key}");
        }
        result[key] = value.clone();
    }
    Ok(result)
}
pub fn system_languages() -> Vec<String> {
    #[cfg(windows)]
    {
        let lang = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
        return vec![match lang & 0x3ff { 0x04 => "zh-CN", 0x11 => "ja", _ => "en" }.into()];
    }
    #[cfg(not(windows))]
    ["LC_ALL", "LC_MESSAGES", "LANG"].into_iter()
        .filter_map(|key| std::env::var(key).ok()).filter(|v| !v.is_empty())
        .map(|v| v.split('.').next().unwrap_or("en").replace('_', "-")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn language_preferences_are_independent_and_validated() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(load(root.path()).unwrap()["language"], "auto");
        for lang in ["auto", "zh-CN", "ja", "en"] {
            let saved = patch(&defaults(), &json!({"language":lang})).unwrap();
            crate::store::write_json(&root.path().join("ui-settings.json"), &saved).unwrap();
            assert_eq!(load(root.path()).unwrap(), saved);
        }
        assert!(patch(&defaults(), &json!({"language":"fr"})).is_err());
        assert!(patch(&defaults(), &json!({"translationLanguage":"ja"})).is_err());
        assert!(patch(&defaults(), &json!("en")).is_err());
    }
}
