//! Downloads only named official model assets. Existing reference models are
//! read-only fallbacks; new files always go in this application's model root.
use super::Speech;
use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::{
    path::{Component, Path},
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
impl Speech {
    pub fn download_models(self: &Arc<Self>, value: &Value) -> Result<Value> {
        if value["cancel"] == true {
            self.download_stop.store(1, Ordering::SeqCst);
            return Ok(json!({"ok":true}));
        }
        if self.state.lock().unwrap()["active"] == true {
            bail!("请先结束录音再下载模型")
        }
        let mut state = self.download.lock().unwrap();
        if state["active"] == true {
            bail!("模型已在下载")
        }
        let settings = self.settings.lock().unwrap().clone();
        let kind = value["kind"]
            .as_str()
            .unwrap_or(&settings.provider)
            .to_string();
        if !matches!(kind.as_str(), "offline" | "hybrid" | "tencent") {
            bail!("模型下载类型无效")
        }
        *state = json!({"active":true,"phase":"downloading","bytes":0,"error":null});
        drop(state);
        self.download_stop.store(0, Ordering::SeqCst);
        let service = self.clone();
        tokio::spawn(async move {
            let result = service.download_inner(&settings, &kind).await;
            let mut state = service.download.lock().unwrap();
            state["active"] = json!(false);
            state["phase"] = json!(if result.is_ok() { "complete" } else { "error" });
            if let Err(e) = result {
                state["error"] = json!(e.to_string());
            }
        });
        Ok(json!({"ok":true}))
    }
    async fn download_inner(&self, s: &super::config::Settings, kind: &str) -> Result<()> {
        std::fs::create_dir_all(&s.model_root)?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(900))
            .build()?;
        let mut files: Vec<(String, bool)> = vec![];
        super::config::ensure_packed_vad(&s.vad_path())?;
        if kind != "tencent" && !s.sv_dir().join("model.int8.onnx").is_file() {
            files.push((
                "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17.tar.bz2".into(),
                true,
            ))
        }
        if kind == "hybrid" && !s.ctc_dir().join("model.int8.onnx").is_file() {
            files.push((
                "sherpa-onnx-streaming-zipformer-small-ctc-zh-int8-2025-04-01.tar.bz2".into(),
                true,
            ))
        }
        for (file, archive) in files {
            if self.download_stop.load(Ordering::SeqCst) != 0 {
                bail!("模型下载已取消")
            }
            let url = format!(
                "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/{file}"
            );
            let response = client.get(&url).send().await?.error_for_status()?;
            let length = response.content_length();
            if length.is_some_and(|n| n > 512 * 1024 * 1024) {
                bail!("模型文件超过512MiB限制")
            }
            {
                let mut state = self.download.lock().unwrap();
                state["file"] = json!(file);
                state["bytes"] = json!(0);
                state["totalBytes"] = json!(length);
            }
            let temp = s
                .model_root
                .join(format!(".download-{}", uuid::Uuid::new_v4()));
            let mut output = tokio::fs::File::create(&temp).await?;
            let mut stream = response.bytes_stream();
            let mut received = 0u64;
            use tokio::io::AsyncWriteExt;
            let result: Result<()> = async {
                while let Some(chunk) = stream.next().await {
                    if self.download_stop.load(Ordering::SeqCst) != 0 {
                        bail!("模型下载已取消")
                    };
                    let chunk = chunk?;
                    received += chunk.len() as u64;
                    if received > 512 * 1024 * 1024 {
                        bail!("模型文件过大")
                    };
                    output.write_all(&chunk).await?;
                    self.download.lock().unwrap()["bytes"] = json!(received);
                }
                output.flush().await?;
                Ok(())
            }
            .await;
            drop(output);
            if let Err(e) = result {
                let _ = tokio::fs::remove_file(&temp).await;
                return Err(e);
            }
            if length.is_some_and(|n| n != received) {
                let _ = tokio::fs::remove_file(&temp).await;
                bail!("模型下载长度不匹配")
            }
            if archive {
                let path = temp.clone();
                let root = s.model_root.clone();
                let prefix = file.trim_end_matches(".tar.bz2").to_string();
                self.download.lock().unwrap()["phase"] = json!("extracting");
                let result = tokio::task::spawn_blocking(move || {
                    let stage = root.join(format!(".unpack-{}", uuid::Uuid::new_v4()));
                    std::fs::create_dir_all(&stage)?;
                    let result: Result<()> = (|| {
                        unpack(&path, &stage, &prefix)?;
                        let incoming = stage.join(&prefix);
                        anyhow::ensure!(
                            incoming.join("model.int8.onnx").metadata()?.len() > 1_000_000
                                && incoming.join("tokens.txt").metadata()?.len() > 0,
                            "模型包缺少完整模型和词表"
                        );
                        let destination = root.join(&prefix);
                        let backup = root.join(format!(".old-{}", uuid::Uuid::new_v4()));
                        let had = destination.exists();
                        if had {
                            std::fs::rename(&destination, &backup)?;
                        }
                        if let Err(e) = std::fs::rename(&incoming, &destination) {
                            if had {
                                let _ = std::fs::rename(&backup, &destination);
                            }
                            return Err(e.into());
                        }
                        if had {
                            std::fs::remove_dir_all(backup)?;
                        }
                        Ok(())
                    })();
                    let _ = std::fs::remove_dir_all(stage);
                    result
                })
                .await?;
                let _ = tokio::fs::remove_file(&temp).await;
                result?;
            } else {
                tokio::fs::rename(&temp, s.model_root.join(&file)).await?;
            }
        }
        Ok(())
    }
}
fn unpack(path: &Path, root: &Path, prefix: &str) -> Result<()> {
    let input = std::fs::File::open(path)?;
    let decoder = bzip2::read::BzDecoder::new(input);
    let mut archive = tar::Archive::new(decoder);
    let mut total = 0u64;
    for item in archive.entries()? {
        let mut entry = item?;
        let path = entry.path()?.into_owned();
        let components = path
            .components()
            .filter(|c| *c != Component::CurDir)
            .collect::<Vec<_>>();
        if components.is_empty() {
            continue;
        }
        if !components.iter().all(|c| matches!(c, Component::Normal(_)))
            || components[0].as_os_str() != std::ffi::OsStr::new(prefix)
        {
            bail!("模型包路径不安全")
        };
        let kind = entry.header().entry_type();
        if !kind.is_file() && !kind.is_dir() {
            bail!("模型包包含不支持的链接或设备文件")
        };
        total = total.checked_add(entry.size()).context("模型包大小溢出")?;
        if total > 1024 * 1024 * 1024 {
            bail!("模型包解压超过1GiB")
        };
        if !entry.unpack_in(root)? {
            bail!("模型包条目越界")
        }
    }
    Ok(())
}
