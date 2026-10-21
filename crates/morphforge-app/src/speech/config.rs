use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub provider: String,
    pub language: String,
    pub device: Option<String>,
    pub model_root: PathBuf,
    pub vad_model_path: PathBuf,
    pub vad_threshold: f32,
    pub silence_seconds: f32,
    pub min_speech_seconds: f32,
    pub max_sentence_seconds: f32,
    pub use_reference_models: bool,
    pub threads: i32,
    pub keyword_enabled: bool,
    pub keyword: String,
    pub auto_caption: bool,
    pub tencent_engine: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            provider: "offline".into(),
            language: "auto".into(),
            device: None,
            model_root: PathBuf::new(),
            vad_model_path: PathBuf::new(),
            vad_threshold: 0.5,
            silence_seconds: 0.45,
            min_speech_seconds: 0.15,
            max_sentence_seconds: 20.,
            use_reference_models: true,
            threads: 2,
            keyword_enabled: false,
            keyword: "开始建模".into(),
            auto_caption: false,
            tencent_engine: "16k_zh".into(),
        }
    }
}
impl Settings {
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join("speech-settings.json");
        let mut s: Self = if path.exists() {
            serde_json::from_slice(&std::fs::read(path)?)?
        } else {
            Self::default()
        };
        if s.model_root.as_os_str().is_empty() {
            s.model_root = root.join("models/speech");
        }
        if s.vad_model_path.as_os_str().is_empty() {
            s.vad_model_path = root.join("models/speech/silero_vad.onnx");
        }
        s.validate()?;
        Ok(s)
    }
    pub fn validate(&self) -> Result<()> {
        if (!self.model_root.as_os_str().is_empty() && !self.model_root.is_absolute())
            || (!self.vad_model_path.as_os_str().is_empty() && !self.vad_model_path.is_absolute())
        {
            bail!("模型目录需要绝对路径")
        };
        if !matches!(self.provider.as_str(), "offline" | "hybrid" | "tencent")
            || !matches!(
                self.language.as_str(),
                "auto" | "zh" | "en" | "ja" | "ko" | "yue"
            )
            || !(1..=16).contains(&self.threads)
            || !(0.05..=0.95).contains(&self.vad_threshold)
            || !(0.15..=2.).contains(&self.silence_seconds)
            || !(0.05..=1.).contains(&self.min_speech_seconds)
            || !(2.0..=60.).contains(&self.max_sentence_seconds)
            || self.keyword.is_empty()
            || self.keyword.len() > 128
            || self.tencent_engine.len() > 64
            || !self
                .tencent_engine
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            bail!("语音配置参数无效")
        }
        Ok(())
    }
    fn resolve(&self, name: &str) -> PathBuf {
        let local = self.model_root.join(name);
        let reference = PathBuf::from(r"D:\Projects\vrc-chat-tool\models").join(name);
        if !local.join("model.int8.onnx").is_file()
            && self.use_reference_models
            && cfg!(windows)
            && reference.join("model.int8.onnx").is_file()
        {
            reference
        } else {
            local
        }
    }
    pub fn sv_dir(&self) -> PathBuf {
        self.resolve("sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17")
    }
    pub fn ctc_dir(&self) -> PathBuf {
        self.resolve("sherpa-onnx-streaming-zipformer-small-ctc-zh-int8-2025-04-01")
    }
    pub fn vad_path(&self) -> PathBuf {
        if self.vad_model_path.as_os_str().is_empty() {
            self.model_root.join("silero_vad.onnx")
        } else {
            self.vad_model_path.clone()
        }
    }
    pub fn models(&self) -> Value {
        json!({"root":self.model_root,"vad":self.vad_path().is_file(),"senseVoice":self.sv_dir().join("model.int8.onnx").is_file()&&self.sv_dir().join("tokens.txt").is_file(),"zipformer":self.ctc_dir().join("model.int8.onnx").is_file()&&self.ctc_dir().join("tokens.txt").is_file()})
    }
}
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Credentials {
    pub app_id: String,
    pub secret_id: String,
    pub secret_key: String,
}
impl Credentials {
    pub fn configured(&self) -> bool {
        !self.app_id.is_empty() && !self.secret_id.is_empty() && !self.secret_key.is_empty()
    }
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join("speech-credentials.json");
        if path.exists() {
            Ok(serde_json::from_slice(&std::fs::read(path)?)?)
        } else {
            Ok(Self::default())
        }
    }
    pub fn import_reference() -> Result<Self> {
        let path = Path::new(r"D:\Projects\vrc-chat-tool\.env");
        let mut c = Self::default();
        for row in dotenvy::from_path_iter(path)? {
            let (k, v) = row?;
            match k.as_str() {
                "TENCENT_APP_ID" => c.app_id = v,
                "TENCENT_SECRET_ID" => c.secret_id = v,
                "TENCENT_SECRET_KEY" => c.secret_key = v,
                _ => {}
            }
        }
        if !c.configured() {
            bail!("参考配置缺少腾讯云凭证")
        };
        Ok(c)
    }
    pub fn validate(&self) -> Result<()> {
        if (!self.app_id.is_empty() || !self.secret_id.is_empty() || !self.secret_key.is_empty())
            && !self.configured()
        {
            bail!("请完整填写三项腾讯云凭证")
        };
        if self.configured()
            && (!self.app_id.bytes().all(|b| b.is_ascii_digit())
                || self.secret_id.len() > 256
                || self.secret_key.len() > 256)
        {
            bail!("腾讯云凭证格式无效")
        }
        Ok(())
    }
}

pub fn ensure_packed_vad(path: &Path) -> Result<()> {
    if path.is_file() {
        return Ok(());
    }
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../models/speech/silero_vad.onnx"
    ));
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("VAD目录无效"))?;
    std::fs::create_dir_all(parent)?;
    let temp = parent.join(format!(".vad-{}", uuid::Uuid::new_v4()));
    std::fs::write(&temp, bytes)?;
    std::fs::rename(temp, path)?;
    Ok(())
}
