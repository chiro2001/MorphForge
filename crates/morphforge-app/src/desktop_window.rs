//! Restricted declarative window preferences. The native shell watches only
//! this file in its own core data directory; web pages never gain generic IPC.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::Mutex};
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Preferences {
    pub revision: u64,
    pub focus: String,
    pub last_action: String,
    pub mini_open: bool,
    pub mode: String,
    pub mini_pinned: bool,
    pub main_pinned: bool,
    pub main_visible: bool,
    pub mini_width: u32,
    pub mini_height: u32,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            revision: 0,
            focus: String::new(),
            last_action: String::new(),
            mini_open: false,
            mode: "speech".into(),
            mini_pinned: false,
            main_pinned: false,
            main_visible: true,
            mini_width: 460,
            mini_height: 320,
        }
    }
}
pub struct Windows {
    root: PathBuf,
    prefs: Mutex<Preferences>,
    native: bool,
}
impl Windows {
    pub fn new(root: PathBuf) -> Result<Self> {
        let path = root.join("desktop-window.json");
        let prefs = if path.exists() {
            serde_json::from_slice(&std::fs::read(path)?)?
        } else {
            Preferences::default()
        };
        Ok(Self {
            root,
            prefs: Mutex::new(prefs),
            native: std::env::var("MORPHFORGE_DESKTOP_RUNTIME").as_deref() == Ok("1"),
        })
    }
    pub fn status(&self) -> Value {
        let native_status = std::fs::read(self.root.join("desktop-window-status.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
        json!({"native":self.native,"preferences":self.prefs.lock().unwrap().clone(),"nativeStatus":native_status})
    }
    pub fn command(&self, value: &Value) -> Result<Value> {
        let action = value["action"].as_str().context("窗口命令缺少action")?;
        let mut prefs = self.prefs.lock().unwrap();
        let mut p = prefs.clone();
        p.focus.clear();
        p.last_action = action.to_string();
        match action {
            "open" => {
                p.mini_open = true;
                p.focus = "mini".into();
                p.mode = value["mode"].as_str().unwrap_or("speech").into();
            }
            "close" => {
                p.mini_open = false;
                p.main_visible = true;
                p.focus = "main".into();
            }
            "show_main" => {
                p.main_visible = true;
                p.focus = "main".into();
            }
            "hide_main" => {
                if !p.mini_open {
                    bail!("先打开mini窗口再隐藏主窗口")
                };
                p.main_visible = false;
            }
            "pin" => {
                let pinned = value["pinned"].as_bool().context("pinned须为布尔值")?;
                match value["window"].as_str().unwrap_or("mini") {
                    "mini" => p.mini_pinned = pinned,
                    "main" => p.main_pinned = pinned,
                    _ => bail!("未知窗口"),
                }
            }
            "resize" => {
                p.mini_width = value["width"].as_u64().context("缺少width")?.try_into()?;
                p.mini_height = value["height"].as_u64().context("缺少height")?.try_into()?;
            }
            _ => bail!("未知窗口命令"),
        }
        if !matches!(p.mode.as_str(), "speech" | "progress" | "custom")
            || !(320..=1600).contains(&p.mini_width)
            || !(180..=1600).contains(&p.mini_height)
        {
            bail!("mini模式或尺寸无效")
        }
        p.revision = p.revision.checked_add(1).context("窗口版本溢出")?;
        crate::store::write_json(&self.root.join("desktop-window.json"), &p)?;
        *prefs = p;
        Ok(json!({"ok":true,"native":self.native,"preferences":prefs.clone()}))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_declared_window_actions_are_persisted() {
        let d = tempfile::tempdir().unwrap();
        let w = Windows::new(d.path().into()).unwrap();
        assert!(w.command(&json!({"action":"hide_main"})).is_err());
        w.command(&json!({"action":"open","mode":"speech"}))
            .unwrap();
        w.command(&json!({"action":"pin","pinned":true})).unwrap();
        w.command(&json!({"action":"hide_main"})).unwrap();
        assert_eq!(w.status()["preferences"]["mainVisible"], false);
        assert!(w
            .command(&json!({"action":"open","mode":"https://bad"}))
            .is_err());
        assert_eq!(
            Windows::new(d.path().into()).unwrap().status()["preferences"]["mode"],
            "speech"
        );
        w.command(&json!({"action":"close"})).unwrap();
        assert_eq!(w.status()["preferences"]["mainVisible"], true);
    }
}
