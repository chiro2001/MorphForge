//! Tencent ASR v2 signing follows the user's VRC Chat Tool implementation.
//! Signed URLs and credential-bearing errors are never logged or returned.
use super::config::{Credentials, Settings};
use anyhow::{bail, Context, Result};
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use sha1::Sha1;
use std::{
    collections::BTreeSet,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
pub fn signed_url(c: &Credentials, engine: &str) -> Result<String> {
    c.validate()?;
    anyhow::ensure!(c.configured(), "腾讯云凭证尚未配置");
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let mut params = vec![
        ("secretid", c.secret_id.clone()),
        ("timestamp", now.to_string()),
        ("expired", (now + 3600).to_string()),
        ("nonce", (now % u32::MAX as u64).to_string()),
        ("engine_model_type", engine.into()),
        ("voice_id", uuid::Uuid::new_v4().to_string()),
        ("voice_format", "1".into()),
        ("needvad", "1".into()),
        ("filter_dirty", "0".into()),
    ];
    params.sort_by_key(|x| x.0);
    let raw = params
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&");
    let payload = format!("asr.cloud.tencent.com/asr/v2/{}?{}", c.app_id, raw);
    let mut mac = Hmac::<Sha1>::new_from_slice(c.secret_key.as_bytes()).context("签名失败")?;
    mac.update(payload.as_bytes());
    let signature = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());
    let query = params
        .iter()
        .map(|(k, v)| format!("{k}={}", urlencoding::encode(v)))
        .chain(std::iter::once(format!(
            "signature={}",
            urlencoding::encode(&signature)
        )))
        .collect::<Vec<_>>()
        .join("&");
    Ok(format!(
        "wss://asr.cloud.tencent.com/asr/v2/{}?{}",
        c.app_id, query
    ))
}
pub async fn recognize(
    c: Credentials,
    s: Settings,
    mut audio: mpsc::Receiver<Vec<f32>>,
    stop: Arc<AtomicU8>,
    partial: Arc<dyn Fn(String) + Send + Sync>,
    final_text: Arc<dyn Fn(String) + Send + Sync>,
) -> Result<()> {
    let url = signed_url(&c, &s.tencent_engine)?;
    let (socket, _) = tokio::time::timeout(
        Duration::from_secs(10),
        tokio_tungstenite::connect_async(&url),
    )
    .await
    .map_err(|_| anyhow::anyhow!("腾讯云连接超时"))?
    .map_err(|_| anyhow::anyhow!("腾讯云连接失败（凭证或网络），未输出签名URL"))?;
    let (mut write, mut read) = socket.split();
    let mut ended = false;
    let mut deadline = None;
    let mut done = BTreeSet::new();
    let tick = tokio::time::sleep(Duration::from_millis(30));
    tokio::pin!(tick);
    loop {
        if stop.load(Ordering::SeqCst) >= 2 {
            let _ = write.close().await;
            return Ok(());
        }
        if deadline.is_some_and(|d| tokio::time::Instant::now() > d) {
            bail!("腾讯云句末响应超时")
        }
        tokio::select! {
         chunk=audio.recv(),if !ended=>match chunk{Some(samples)=>{let mut pcm=Vec::with_capacity(samples.len()*2);for v in samples{pcm.extend_from_slice(&((v.clamp(-1.,1.)*32767.).round()as i16).to_le_bytes());}write.send(Message::Binary(pcm)).await.map_err(|_|anyhow::anyhow!("腾讯云音频发送失败"))?;},None=>{write.send(Message::Text("{\"type\":\"end\"}".into())).await.map_err(|_|anyhow::anyhow!("腾讯云结束请求失败"))?;ended=true;deadline=Some(tokio::time::Instant::now()+Duration::from_secs(8));}},
         response=read.next()=>match response{
          Some(Ok(Message::Text(text)))=>{let v:serde_json::Value=serde_json::from_str(&text).context("腾讯云响应格式错误")?;if v["code"].as_i64().unwrap_or(0)!=0{bail!("腾讯云识别失败，错误码{}",v["code"])}
            if let Some(result)=v["result"].as_object(){let text=result.get("voice_text_str").and_then(|x|x.as_str()).unwrap_or("").trim();if !text.is_empty(){match result.get("slice_type").and_then(|x|x.as_u64()){Some(1)=>partial(text.into()),Some(2)=>{let index=result.get("index").and_then(|x|x.as_u64()).unwrap_or(0);if done.insert(index){final_text(text.into());}},_=>{}}}}
            if v["final"]==1{break}
          },Some(Ok(Message::Ping(bytes)))=>{write.send(Message::Pong(bytes)).await.map_err(|_|anyhow::anyhow!("腾讯云心跳失败"))?;},Some(Ok(Message::Close(_)))|None=>break,Some(Err(_))=>bail!("腾讯云连接中断"),_=>{}
         },
         _=&mut tick=>{tick.as_mut().reset(tokio::time::Instant::now()+Duration::from_millis(30));}
        }
    }
    Ok(())
}
