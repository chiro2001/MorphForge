use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, io::Write, path::Path};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiType {
    #[default]
    Responses,
    #[serde(
        rename = "chat",
        alias = "chat_completions",
        alias = "chat-completions"
    )]
    Chat,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageDelivery {
    #[default]
    Inline,
    Files,
}

/// Local-only connection file. Never serialize this structure into HTTP state or traces.
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Connection {
    pub base_url: String,
    pub api_type: ApiType,
    pub api_key: Option<String>,
    pub image_delivery: ImageDelivery,
    pub send_reasoning: bool,
    pub stream: bool,
    pub chat_token_field: String,
    pub requires_api_key: bool,
}

impl Default for Connection {
    fn default() -> Self {
        Self {
            base_url: "https://api.deepseek.com".into(),
            api_type: ApiType::Responses,
            api_key: None,
            image_delivery: ImageDelivery::Inline,
            send_reasoning: true,
            stream: true,
            chat_token_field: "max_tokens".into(),
            requires_api_key: true,
        }
    }
}

impl Connection {
    pub fn load(root: &Path, env_file: Option<&Path>) -> Result<Self> {
        let path = root.join("provider-connection.json");
        if path.exists() {
            let mut value: Self = serde_json::from_slice(&std::fs::read(&path)?)
                .context("本机Provider连接配置无效")?;
            value.validate()?;
            return Ok(value);
        }
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(std::path::PathBuf::from)
            .unwrap_or_default();
        let paths = env_file.map(|p| vec![p.to_path_buf()]).unwrap_or_else(|| {
            vec![
                home.join("litellm/.env"),
                home.join("AppData/Local/MorphForge/provider.env"),
            ]
        });
        let mut env = BTreeMap::new();
        for path in paths {
            if let Ok(entries) = dotenvy::from_path_iter(path) {
                for (key, value) in entries.flatten() {
                    env.entry(key).or_insert(value);
                }
            }
        }
        for key in [
            "DEEPSEEK_API_KEY",
            "MORPHFORGE_DEEPSEEK_BASE_URL",
            "MORPHFORGE_API_KEY",
            "MORPHFORGE_API_BASE_URL",
            "MORPHFORGE_API_TYPE",
        ] {
            if let Ok(value) = std::env::var(key) {
                env.insert(key.into(), value);
            }
        }
        let mut value = Self::default();
        if let Some(base) = env
            .get("MORPHFORGE_API_BASE_URL")
            .or_else(|| env.get("MORPHFORGE_DEEPSEEK_BASE_URL"))
        {
            value.base_url = base.clone();
            value.send_reasoning = reqwest::Url::parse(base)
                .ok()
                .and_then(|u| u.host_str().map(str::to_owned))
                .as_deref()
                == Some("api.deepseek.com");
        }
        if let Some(api) = env.get("MORPHFORGE_API_TYPE") {
            value.api_type = serde_json::from_value(json!(api))
                .context("MORPHFORGE_API_TYPE须为responses或chat")?;
        }
        value.validate()?;
        value.api_key = env
            .get("MORPHFORGE_API_KEY")
            .or_else(|| {
                (reqwest::Url::parse(&value.base_url).ok()?.host_str()? == "api.deepseek.com")
                    .then(|| env.get("DEEPSEEK_API_KEY"))
                    .flatten()
            })
            .filter(|v| !v.trim().is_empty())
            .cloned();
        value.validate()?;
        Ok(value)
    }

    pub fn patched(&self, patch: &Value) -> Result<Self> {
        let fields = patch.as_object().context("Provider配置须为对象")?;
        let mut next = serde_json::to_value(self)?;
        for (key, value) in fields {
            if key == "clearKey" {
                continue;
            }
            if next.get(key).is_none() {
                bail!("未知Provider设置 {key}")
            }
            if key == "apiKey" && value.as_str().is_some_and(|s| s.is_empty()) {
                continue;
            }
            next[key] = value.clone();
        }
        let mut next: Self = serde_json::from_value(next)?;
        next.validate()?;
        if next.base_url != self.base_url
            && !fields
                .get("apiKey")
                .and_then(Value::as_str)
                .is_some_and(|s| !s.trim().is_empty())
        {
            // Credentials never silently move to a different endpoint.
            next.api_key = None;
        }
        if patch["clearKey"] == true {
            next.api_key = None;
        }
        Ok(next)
    }

    pub fn validate(&mut self) -> Result<()> {
        let mut url = reqwest::Url::parse(self.base_url.trim()).context("Base URL无效")?;
        if !["http", "https"].contains(&url.scheme())
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            bail!("Base URL须为HTTP/HTTPS地址，不包含账号、密码、查询参数或fragment")
        }
        let path = url.path().trim_end_matches('/');
        let path = path
            .strip_suffix("/chat/completions")
            .or_else(|| path.strip_suffix("/responses"))
            .unwrap_or(path)
            .to_owned();
        url.set_path(&path);
        self.base_url = url.as_str().trim_end_matches('/').into();
        if !self.supports_file_upload() {
            self.image_delivery = ImageDelivery::Inline;
        }
        if !["max_tokens", "max_completion_tokens"].contains(&self.chat_token_field.as_str()) {
            bail!("Chat输出字段须为max_tokens或max_completion_tokens")
        }
        if let Some(key) = &mut self.api_key {
            *key = key.trim().into();
            if key.len() > 8192 || key.chars().any(char::is_control) {
                bail!("API Key格式无效")
            }
            if key.is_empty() {
                self.api_key = None;
            }
        }
        Ok(())
    }

    pub fn supports_file_upload(&self) -> bool {
        self.api_type == ApiType::Responses
            && reqwest::Url::parse(&self.base_url).ok().is_some_and(|url| {
                url.scheme() == "https"
                    && url.host_str() == Some("api.deepseek.com")
                    && url.port_or_known_default() == Some(443)
            })
    }

    pub fn configured(&self) -> bool {
        self.api_key.is_some()
            || !self.requires_api_key
            || reqwest::Url::parse(&self.base_url)
                .ok()
                .and_then(|u| u.host_str().map(str::to_owned))
                .is_some_and(|h| ["127.0.0.1", "localhost", "[::1]", "::1"].contains(&h.as_str()))
    }

    pub fn status(&self) -> Value {
        json!({"configured":self.configured(),"baseUrl":self.base_url,"apiType":self.api_type,
            "interface":self.api_type,"imageDelivery":self.image_delivery,"fileUpload":self.image_delivery==ImageDelivery::Files,"supportsFileUpload":self.supports_file_upload(),
            "sendReasoning":self.send_reasoning,"stream":self.stream,"chatTokenField":self.chat_token_field,
            "requiresApiKey":self.requires_api_key,"keyConfigured":self.api_key.is_some()})
    }

    pub fn identity(&self) -> String {
        crate::store::hash(
            format!(
                "{}\n{:?}\n{}",
                self.base_url,
                self.api_type,
                self.api_key.as_deref().unwrap_or("")
            )
            .as_bytes(),
        )
    }

    pub fn save(&self, root: &Path) -> Result<()> {
        let path = root.join("provider-connection.json");
        let temp = root.join(format!("provider-connection.{}.tmp", uuid::Uuid::new_v4()));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let result = (|| -> Result<()> {
            let mut file = options.open(&temp)?;
            file.write_all(&serde_json::to_vec_pretty(self)?)?;
            file.sync_all()?;
            drop(file);
            std::fs::rename(&temp, &path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changing_endpoint_never_inherits_secret_and_status_is_public() {
        let original = Connection {
            api_key: Some("test-only-secret".into()),
            ..Connection::default()
        };
        let changed = original.patched(&json!({"baseUrl":"https://example.com/v1/chat/completions","apiType":"chat","apiKey":""})).unwrap();
        assert_eq!(changed.base_url, "https://example.com/v1");
        assert!(changed.api_key.is_none());
        assert!(!original.status().to_string().contains("test-only-secret"));
        assert!(original
            .patched(&json!({"baseUrl":"https://user:pass@example.com"}))
            .is_err());
        assert_eq!(
            original
                .patched(&json!({"apiType":"chat","imageDelivery":"files"}))
                .unwrap()
                .image_delivery,
            ImageDelivery::Inline
        );
        assert!(original.patched(&json!({"unknown":true})).is_err());
    }
    #[test]
    fn saved_secret_roundtrips_without_env_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let connection = Connection {
            base_url: "http://127.0.0.1:1234/v1".into(),
            api_type: ApiType::Chat,
            api_key: Some("local-test-key".into()),
            ..Connection::default()
        };
        connection.save(dir.path()).unwrap();
        let restored = Connection::load(dir.path(), None).unwrap();
        assert_eq!(restored.api_key, connection.api_key);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(dir.path().join("provider-connection.json"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
}
