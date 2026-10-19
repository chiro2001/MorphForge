//! Deliberate allowlist: browser profiles, .env, credentials and provider-file cache are excluded.
use crate::store::Event;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    io::{BufRead, Cursor, Read, Write},
    path::{Path, PathBuf},
};
fn files(root: &Path, directory: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !directory.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        let p = entry.path();
        if kind.is_dir() {
            files(root, &p, out)?
        } else if kind.is_file() {
            let rel = p.strip_prefix(root)?;
            if rel.components().count() < 5
                && matches!(
                    p.extension().and_then(|x| x.to_str()),
                    Some("json" | "jsonl" | "js" | "txt")
                )
            {
                out.push(p)
            }
        }
    }
    Ok(())
}
pub fn bundle(root: &Path, redact: impl Fn(&str) -> String) -> Result<Vec<u8>> {
    let mut paths = vec![];
    for name in [
        "project.json",
        "connection.json",
        "harness-settings.json",
        "harness.json",
        "conversation.json",
    ] {
        let p = root.join(name);
        if p.is_file() {
            paths.push(p)
        }
    }
    for dir in ["events", "runs", "snapshots", "artworks", "packs"] {
        files(root, &root.join(dir), &mut paths)?
    }
    paths.sort();
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut total = 0usize;
    let mut manifest = vec![];
    for p in paths {
        let mut bytes = String::new();
        std::fs::File::open(&p)?
            .take(32 * 1024 * 1024 + 1)
            .read_to_string(&mut bytes)?;
        if bytes.len() > 32 * 1024 * 1024 {
            bail!("单份记录超过诊断导出保护值")
        };
        total += bytes.len();
        if total > 128 * 1024 * 1024 {
            bail!("诊断文本超过128MiB，请按run单独归档")
        };
        let name = p.strip_prefix(root)?.to_string_lossy().replace('\\', "/");
        archive.start_file(&name, options)?;
        archive.write_all(redact(&bytes).as_bytes())?;
        manifest.push(name);
    }
    archive.start_file("manifest.json", options)?;
    archive.write_all(serde_json::to_string_pretty(&json!({"format":"MorphForgeDiagnostics/1","createdAt":crate::store::now(),"files":manifest,"excluded":["credentials","provider-files cache","browser profile","binary artifacts"],"artifactReferences":"PNG and other binary artifacts stay in the local workspace; exported events retain hashes"}))?.as_bytes())?;
    Ok(archive.finish()?.into_inner())
}
pub fn metrics(root: &Path) -> Result<Value> {
    let p = root.join("events/events.jsonl");
    if !p.exists() {
        return Ok(json!({"events":0}));
    }
    let file = std::io::BufReader::new(std::fs::File::open(p)?);
    let mut count = 0;
    let mut kinds = BTreeMap::<String, u64>::new();
    let mut tools = BTreeMap::<String, Vec<f64>>::new();
    let mut errors = 0;
    let mut tokens = 0u64;
    let mut provider = vec![];
    let mut compact = 0;
    for line in file.lines() {
        let line = line.context("读取运行日志")?;
        let Ok(e) = serde_json::from_str::<Event>(&line) else {
            continue;
        };
        count += 1;
        *kinds.entry(e.kind.clone()).or_default() += 1;
        match e.kind.as_str() {
            "tool.result" => {
                let name = e.data["name"].as_str().unwrap_or("unknown").to_owned();
                if let Some(ms) = e.data["elapsedMs"].as_f64() {
                    tools.entry(name).or_default().push(ms)
                }
                if e.data["result"]["ok"] == false {
                    errors += 1
                }
            }
            "provider.response" => {
                tokens += e.data["body"]["usage"]["total_tokens"]
                    .as_u64()
                    .unwrap_or(0);
                if let Some(s) = e.data["latencySeconds"].as_f64() {
                    provider.push(s * 1000.)
                }
            }
            "compact.completed" => compact += 1,
            _ => {}
        }
    }
    fn distribution(mut v: Vec<f64>) -> Value {
        if v.is_empty() {
            return Value::Null;
        }
        v.sort_by(f64::total_cmp);
        let pct = |p: f64| v[((v.len() - 1) as f64 * p).ceil() as usize];
        json!({"count":v.len(),"p50Ms":pct(0.5),"p95Ms":pct(0.95),"maxMs":v.last()})
    }
    Ok(
        json!({"events":count,"eventKinds":kinds,"toolFailures":errors,"providerTokens":tokens,"compactCount":compact,"providerLatency":distribution(provider),"tools":tools.into_iter().map(|(k,v)|(k,distribution(v))).collect::<BTreeMap<_,_>>()}),
    )
}
