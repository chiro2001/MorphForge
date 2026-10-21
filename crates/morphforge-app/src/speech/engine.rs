//! Shared local model cache. Each recording session creates its own VAD/CTC
//! streams; the large offline recognizer stays warm between recordings.
use super::config::Settings;
use anyhow::{Context, Result};
use sherpa_onnx::{
    OfflineRecognizer, OfflineRecognizerConfig, OfflineSenseVoiceModelConfig, OnlineRecognizer,
    OnlineRecognizerConfig, VadModelConfig, VoiceActivityDetector,
};
pub struct Engines {
    pub signature: String,
    pub offline: Option<OfflineRecognizer>,
    pub online: Option<OnlineRecognizer>,
}
impl Engines {
    pub fn load(s: &Settings) -> Result<Self> {
        let signature = serde_json::to_string(&serde_json::json!([
            s.provider,
            s.language,
            s.model_root,
            s.threads
        ]))?;
        let offline = if s.provider != "tencent" {
            let dir = s.sv_dir();
            anyhow::ensure!(
                dir.join("model.int8.onnx").is_file() && dir.join("tokens.txt").is_file(),
                "缺少SenseVoice模型，请在语音设置中下载或选择模型目录"
            );
            let mut c = OfflineRecognizerConfig::default();
            c.model_config.sense_voice = OfflineSenseVoiceModelConfig {
                model: Some(dir.join("model.int8.onnx").to_string_lossy().into()),
                language: Some(s.language.clone()),
                use_itn: true,
            };
            c.model_config.tokens = Some(dir.join("tokens.txt").to_string_lossy().into());
            c.model_config.num_threads = s.threads;
            c.model_config.provider = Some("cpu".into());
            c.feat_config.sample_rate = 16000;
            Some(OfflineRecognizer::create(&c).context("SenseVoice初始化失败")?)
        } else {
            None
        };
        let online = if s.provider == "hybrid" {
            let dir = s.ctc_dir();
            anyhow::ensure!(
                dir.join("model.int8.onnx").is_file() && dir.join("tokens.txt").is_file(),
                "缺少Zipformer流式模型"
            );
            let mut c = OnlineRecognizerConfig::default();
            c.model_config.zipformer2_ctc.model =
                Some(dir.join("model.int8.onnx").to_string_lossy().into());
            c.model_config.tokens = Some(dir.join("tokens.txt").to_string_lossy().into());
            c.model_config.model_type = Some("zipformer2_ctc".into());
            c.model_config.num_threads = s.threads;
            c.model_config.provider = Some("cpu".into());
            c.feat_config.sample_rate = 16000;
            c.decoding_method = Some("greedy_search".into());
            Some(OnlineRecognizer::create(&c).context("Zipformer初始化失败")?)
        } else {
            None
        };
        Ok(Self {
            signature,
            offline,
            online,
        })
    }
    pub fn recognize(&self, samples: &[f32]) -> Result<String> {
        let recognizer = self.offline.as_ref().context("本地句末识别器未加载")?;
        let stream = recognizer.create_stream();
        stream.accept_waveform(16000, samples);
        recognizer.decode(&stream);
        Ok(stream
            .get_result()
            .map(|r| clean(&r.text))
            .unwrap_or_default())
    }
}
pub fn vad(s: &Settings) -> Result<VoiceActivityDetector> {
    anyhow::ensure!(
        s.vad_path().is_file(),
        "缺少本地Silero VAD，请在语音设置下载模型"
    );
    let mut c = VadModelConfig::default();
    c.silero_vad.model = Some(s.vad_path().to_string_lossy().into());
    c.silero_vad.threshold = s.vad_threshold;
    c.silero_vad.min_silence_duration = s.silence_seconds;
    c.silero_vad.min_speech_duration = s.min_speech_seconds;
    c.silero_vad.max_speech_duration = s.max_sentence_seconds;
    c.silero_vad.window_size = 512;
    c.sample_rate = 16000;
    c.num_threads = 1;
    c.provider = Some("cpu".into());
    VoiceActivityDetector::create(&c, 90.).context("Silero VAD初始化失败")
}
pub fn clean(text: &str) -> String {
    let mut out = String::new();
    let mut skip = false;
    let chars = text.chars().collect::<Vec<_>>();
    let mut i = 0;
    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '<' && chars[i + 1] == '|' {
            skip = true;
            i += 2;
            continue;
        }
        if skip && i + 1 < chars.len() && chars[i] == '|' && chars[i + 1] == '>' {
            skip = false;
            i += 2;
            continue;
        }
        if !skip {
            out.push(chars[i]);
        }
        i += 1;
    }
    out.trim().into()
}
