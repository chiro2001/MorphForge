//! Read-only, bounded projection of the durable event journal; no model requests.
use crate::store::Event;
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    fs::File,
    io::{BufRead, BufReader, Seek, SeekFrom, Write},
    path::Path,
};

fn clip(s: &str, n: usize) -> String {
    let mut out = s.chars().take(n).collect::<String>();
    if s.chars().count() > n {
        out.push_str("\n…（显示已截断；原始记录保留）");
    }
    out
}
fn light(value: &Value, n: usize, depth: u8) -> Value {
    if depth > 6 {
        return json!("…");
    }
    match value {
        Value::String(s) if s.starts_with("data:image/") => {
            json!("[图片像素保存在原始记录，不在时间轴重发]")
        }
        Value::String(s) => json!(clip(s, n)),
        Value::Array(a) => {
            Value::Array(a.iter().take(24).map(|v| light(v, n, depth + 1)).collect())
        }
        Value::Object(o) => Value::Object(
            o.iter()
                .take(32)
                .filter(|(k, _)| {
                    !matches!(
                        k.as_str(),
                        "apiKey" | "api_key" | "Authorization" | "authorization"
                    )
                })
                .map(|(k, v)| (k.clone(), light(v, n, depth + 1)))
                .collect(),
        ),
        _ => value.clone(),
    }
}
fn response_text(body: &Value) -> (String, String) {
    let mut reasoning = String::new();
    let mut text = String::new();
    for item in body["output"].as_array().into_iter().flatten() {
        if let Some(s) = item["chat_reasoning_content"].as_str() {
            reasoning.push_str(s);
        }
        let target = if item["type"] == "reasoning" {
            &mut reasoning
        } else {
            &mut text
        };
        if let Some(s) = item["text"].as_str() {
            target.push_str(s);
        }
        for part in item["summary"]
            .as_array()
            .into_iter()
            .flatten()
            .chain(item["content"].as_array().into_iter().flatten())
        {
            if let Some(s) = part["text"].as_str() {
                target.push_str(s);
                target.push('\n');
            }
        }
    }
    (reasoning, text)
}
fn tool_summary(data: &Value) -> String {
    let args = data["arguments"]
        .as_str()
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .unwrap_or_else(|| data["arguments"].clone());
    let name = data["name"].as_str().unwrap_or("工具");
    match name {
        "recipe_sources" => format!(
            "{}源码 · {}",
            match args["action"].as_str().unwrap_or("") {
                "run" => "运行",
                "read" => "读取",
                "patch" => "修改",
                "write" => "保存",
                "list" => "列出",
                _ => "管理",
            },
            args["path"].as_str().unwrap_or("工程文件")
        ),
        "capture_views" => format!(
            "观察模型 · {}个视角",
            args["views"].as_array().map_or(0, Vec::len)
                + args["shots"].as_array().map_or(0, Vec::len)
        ),
        "run_recipe" => "执行绘图代码".into(),
        "get_context" => "读取工程状态与材质".into(),
        "finish_artwork" => "复核完成并保存作品".into(),
        "define_palette" => "创建作品配色".into(),
        "define_materials" => "创建作品材质".into(),
        "read_artifact" => "查看图片或源码文件".into(),
        "transform_scene" => "调整作品整体位置".into(),
        "mesh_tools" => "编辑三角网格".into(),
        "components" => format!("组件 · {}", args["action"].as_str().unwrap_or("list")),
        "reference_workbench" => "参考图与相机对照".into(),
        "animation" => "编辑分组动画".into(),
        _ => name.into(),
    }
}
pub fn project(event: &Event, limit: usize) -> Value {
    let d = &event.data;
    let (title, body) = match event.kind.as_str() {
        "provider.request" => (
            "发送模型请求",
            json!({"requestId":d["requestId"],"model":d["body"]["model"],"outputCeiling":d["body"].get("max_tokens").or_else(||d["body"].get("max_output_tokens")).or_else(||d["body"].get("max_completion_tokens")),"trace":d["trace"],"prefix":d["prefix"]}),
        ),
        "provider.response" => {
            let (reasoning, text) = response_text(&d["body"]);
            (
                "模型响应",
                json!({"requestId":d["requestId"],"status":d["body"]["status"],"reasoning":clip(&reasoning,limit),"reasoningProvided":!reasoning.is_empty()||d["body"]["_mfStreamReasoningReturned"]==true,"text":clip(&text,limit),"usage":d["body"]["usage"],"seconds":d["latencySeconds"],"tools":d["body"]["output"].as_array().into_iter().flatten().filter(|v|v["type"]=="function_call").map(|v|light(v,limit,0)).collect::<Vec<_>>()}),
            )
        }
        "provider.reasoning_delta" => ("思考片段 · 上游流式返回", light(d, limit, 0)),
        "provider.text_delta" => ("模型文本 · 进行中", light(d, limit, 0)),
        "provider.attempt" => ("HTTP发送尝试", light(d, limit, 0)),
        "tool.call" | "tool.called" | "tool.request" => {
            let mut body = light(d, limit, 0);
            body["summary"] = json!(tool_summary(d));
            ("调用工具", body)
        }
        "tool.result" => ("工具结果", light(d, limit, 0)),
        "capture.completed" => ("真实截图", light(d, limit, 0)),
        "cache.guard.blocked" => ("缓存保护已暂停", light(d, limit, 0)),
        "cache.guard.status" => ("缓存与费用监测", light(d, limit, 0)),
        "harness.started" => ("开始运行", light(d, limit, 0)),
        "harness.finished" => ("运行结束", light(d, limit, 0)),
        "harness.message.applied" => ("用户消息已应用", light(d, limit, 0)),
        _ => (event.kind.as_str(), light(d, limit, 0)),
    };
    let result_revision = d["result"]["revision"]
        .as_u64()
        .or_else(|| d["result"]["result"]["revision"].as_u64());
    let capture_revision = if event.kind == "capture.completed" {
        d["revision"].as_u64()
    } else {
        None
    };
    let revision = capture_revision
        .or(result_revision)
        .unwrap_or(event.revision);
    let revision_source = if capture_revision.is_some() {
        "capture"
    } else if result_revision.is_some() {
        "result"
    } else {
        "at_event"
    };
    let image_role =
        if d["imageRole"] == "reinspection" || d["result"]["imageRole"] == "reinspection" {
            "reinspection"
        } else {
            "capture"
        };
    let mut images = vec![];
    for values in [d.get("images"), d["result"].get("images")]
        .into_iter()
        .flatten()
    {
        for image in values.as_array().into_iter().flatten() {
            if let Some(artifact) = image["artifact"]
                .as_str()
                .filter(|s| !s.contains('/') && !s.contains('\\') && !s.contains(".."))
            {
                images.push(json!({"artifact":artifact,"view":image["view"],"url":format!("/artifacts/{artifact}"),"role":image_role,"revision":if image_role=="capture"{Some(revision)}else{None}}));
            }
        }
    }
    json!({"projectId":event.project_id,"sessionId":event.session_id,"seq":event.seq,"time":event.time,"kind":event.kind,"runId":event.run_id,"revision":revision,"recordedRevision":event.revision,"revisionSource":revision_source,"requestId":d["requestId"],"callId":d.get("callId").or_else(||d.get("call_id")),"title":title,"body":body,"images":images})
}
pub fn write_row(writer: &mut impl Write, event: &Event) -> Result<()> {
    let mut row = project(event, 4096);
    row["callId"] = light(&row["callId"], 256, 0);
    if serde_json::to_vec(&row)?.len() > 64 * 1024 {
        row["body"] = json!({"preview":clip(&row["body"].to_string(),8000),"truncated":true,"note":"大记录可按需展开，原始事件保留"});
    }
    serde_json::to_writer(&mut *writer, &row)?;
    writer.write_all(b"\n")?;
    Ok(())
}
pub fn append(root: &Path, event: &Event) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join("events/timeline.jsonl"))?;
    write_row(&mut file, event)
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    pub cursor: Option<u64>,
    pub before: Option<u64>,
    pub limit: Option<usize>,
    pub run: Option<String>,
    pub project_id: Option<String>,
    pub session: Option<String>,
    pub kind: Option<String>,
    pub search: Option<String>,
}
pub fn page(root: &Path, q: &Query) -> Result<Value> {
    let file = File::open(root.join("events/timeline.jsonl"))?;
    let length = file.metadata()?.len();
    let reset = q.cursor.is_some_and(|n| n > length);
    let forward = q.cursor.is_some() && !reset;
    let end = if forward {
        length
    } else {
        q.before.unwrap_or(length).min(length)
    };
    let start = if forward {
        q.cursor.unwrap_or(0)
    } else {
        end.saturating_sub(512 * 1024)
    };
    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(start))?;
    let mut at = start;
    let mut line = String::new();
    if start > 0 && !forward {
        // A byte offset can land in the middle of a multibyte UTF-8 scalar.
        // Discard the partial line as bytes before decoding complete JSON lines.
        let mut partial = Vec::new();
        at += reader.read_until(b'\n', &mut partial)? as u64;
    }
    let limit = q.limit.unwrap_or(100).clamp(1, 200);
    let mut rows = VecDeque::new();
    let mut examined = 0u64;
    while at < end && examined < 512 * 1024 {
        line.clear();
        let size = reader.read_line(&mut line)?;
        if size == 0 {
            break;
        }
        if !line.ends_with('\n') {
            break;
        }
        let offset = at;
        at += size as u64;
        examined += size as u64;
        if at > end {
            break;
        }
        let Ok(mut row) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if q.project_id
            .as_ref()
            .is_some_and(|id| row["projectId"].as_str() != Some(id))
            || q.session
                .as_ref()
                .is_some_and(|id| row["sessionId"].as_str() != Some(id))
            || q.run
                .as_ref()
                .is_some_and(|run| row["runId"].as_str().is_none_or(|r| !r.contains(run)))
            || q.kind
                .as_ref()
                .is_some_and(|kind| !row["kind"].as_str().unwrap_or("").starts_with(kind))
            || q.search
                .as_ref()
                .is_some_and(|s| !line.to_lowercase().contains(&s.to_lowercase()))
        {
            continue;
        }
        row["cursor"] = json!(offset);
        rows.push_back((offset, row));
        if forward && rows.len() >= limit {
            break;
        }
        if rows.len() > limit {
            rows.pop_front();
        }
    }
    let previous = rows.front().map(|v| v.0).unwrap_or(start);
    Ok(
        json!({"events":rows.into_iter().map(|(_,row)|row).collect::<Vec<_>>(),"nextCursor":at,"previousCursor":previous,"hasOlder":previous>0,"hasMore":at<length,"reset":reset}),
    )
}
pub fn detail(root: &Path, seq: u64) -> Result<Value> {
    for line in BufReader::new(File::open(root.join("events/events.jsonl"))?).lines() {
        let line = line?;
        let Ok(event) = serde_json::from_str::<Event>(&line) else {
            continue;
        };
        if event.seq == seq {
            return Ok(project(&event, 500_000));
        }
    }
    anyhow::bail!("记录不存在")
}
pub fn brief(event: &Event) -> Value {
    let mut v = serde_json::to_value(event).unwrap_or(Value::Null);
    if matches!(
        event.kind.as_str(),
        "provider.request"
            | "provider.response"
            | "tool.call"
            | "tool.called"
            | "tool.request"
            | "tool.result"
    ) {
        v["data"] = json!({"requestId":event.data["requestId"],"name":event.data["name"]});
    } else {
        v["data"] = light(&event.data, 4096, 0);
    }
    v
}
#[cfg(test)]
mod tests {
    use super::*;
    fn event(seq: u64) -> Event {
        Event {
            project_id: None,
            session_id: None,
            seq,
            time: "2026-10-19T00:00:00Z".into(),
            kind: "provider.response".into(),
            run_id: Some("r".into()),
            revision: 3,
            data: json!({"requestId":"q","body":{"status":"completed","output":[{"type":"message","chat_reasoning_content":"真实思考","content":[{"type":"output_text","text":"操作完成"}]}]}}),
        }
    }
    #[test]
    fn projects_real_reasoning_and_avoids_request_pixels() {
        let e = event(1);
        let p = project(&e, 100);
        assert_eq!(p["body"]["reasoning"], "真实思考");
        let mut e = e;
        e.kind = "provider.request".into();
        e.data["body"]["messages"] = json!([{"content":"secret base64 pixels"}]);
        assert!(!project(&e, 100).to_string().contains("secret base64"));
    }
    #[test]
    fn durable_pagination_filters_and_full_details() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("events")).unwrap();
        let mut raw = File::create(dir.path().join("events/events.jsonl")).unwrap();
        for i in 1..=8 {
            let e = event(i);
            serde_json::to_writer(&mut raw, &e).unwrap();
            raw.write_all(b"\n").unwrap();
            append(dir.path(), &e).unwrap();
        }
        let tail = page(
            dir.path(),
            &Query {
                limit: Some(3),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(tail["events"][0]["seq"], 6);
        let older = page(
            dir.path(),
            &Query {
                before: tail["previousCursor"].as_u64(),
                limit: Some(3),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(older["events"][0]["seq"], 3);
        assert_eq!(detail(dir.path(), 1).unwrap()["revision"], 3);
    }
    #[test]
    fn partial_rows_do_not_advance_the_cursor() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("events")).unwrap();
        append(dir.path(), &event(1)).unwrap();
        let file = dir.path().join("events/timeline.jsonl");
        let end = std::fs::metadata(&file).unwrap().len();
        let mut writer = std::fs::OpenOptions::new()
            .append(true)
            .open(&file)
            .unwrap();
        let row = project(&event(2), 100);
        serde_json::to_writer(&mut writer, &row).unwrap();
        writer.flush().unwrap();
        let q = Query {
            cursor: Some(end),
            ..Default::default()
        };
        let page1 = page(dir.path(), &q).unwrap();
        assert_eq!(page1["nextCursor"], end);
        assert!(page1["events"].as_array().unwrap().is_empty());
        writer.write_all(b"\n").unwrap();
        writer.flush().unwrap();
        assert_eq!(page(dir.path(), &q).unwrap()["events"][0]["seq"], 2);
    }
    #[test]
    fn history_is_readable_after_the_in_memory_ring_has_evicted_it() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = crate::store::Workspace::open(dir.path().into()).unwrap();
        for _ in 0..5100 {
            w.event("probe", Some("old-run"), json!({"text":"old"}))
                .unwrap();
        }
        assert!(w.events.first().unwrap().seq > 1);
        assert_eq!(detail(dir.path(), 1).unwrap()["seq"], 1);
        drop(w);
        let _w = crate::store::Workspace::open(dir.path().into()).unwrap();
        assert_eq!(
            page(
                dir.path(),
                &Query {
                    cursor: Some(0),
                    limit: Some(1),
                    ..Default::default()
                }
            )
            .unwrap()["events"][0]["seq"],
            1
        );
    }
    #[test]
    fn historical_capture_links_its_target_revision_instead_of_the_current_editor_revision() {
        let mut e = event(1);
        e.revision = 4;
        e.kind = "capture.completed".into();
        e.data = json!({"revision":3,"images":[{"artifact":"abc.png","view":"front"}]});
        let row = project(&e, 100);
        assert_eq!(row["revision"], 3);
        assert_eq!(row["recordedRevision"], 4);
        assert_eq!(row["images"][0]["revision"], 3);
        e.kind = "tool.result".into();
        e.data = json!({"result":{"imageRole":"reinspection","images":[{"artifact":"abc.png","view":"historical"}]}});
        let row = project(&e, 100);
        assert!(row["images"][0]["revision"].is_null());
    }
    #[test]
    fn observation_filters_sessions_without_losing_raw_history() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("events")).unwrap();
        for (seq, session) in [(1, "old"), (2, "active"), (3, "old")] {
            let mut e = event(seq);
            e.session_id = Some(session.into());
            append(dir.path(), &e).unwrap();
        }
        let result = page(
            dir.path(),
            &Query {
                session: Some("active".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result["events"].as_array().unwrap().len(), 1);
        assert_eq!(result["events"][0]["seq"], 2);
        assert_eq!(
            page(dir.path(), &Query::default()).unwrap()["events"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
    }
    #[test]
    fn source_operation_summary_does_not_display_the_source_blob() {
        let mut e = event(1);
        e.kind = "tool.request".into();
        e.data = json!({"name":"recipe_sources","arguments":json!({"action":"patch","path":"bird.js","content":"long source"}).to_string()});
        let value = project(&e, 4096);
        assert_eq!(value["body"]["summary"], "修改源码 · bird.js");
    }
    #[test]
    fn backward_pages_can_start_inside_chinese_utf8() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("events")).unwrap();
        let mut bytes = Vec::new();
        for seq in 1..=100 {
            serde_json::to_writer(&mut bytes,&json!({"seq":seq,"kind":"provider.reasoning_delta","body":{"text":"中".repeat(2000)}})).unwrap();
            bytes.push(b'\n');
        }
        let prefix = bytes.clone();
        let mut split = false;
        for pad in 0..4 {
            bytes = prefix.clone();
            serde_json::to_writer(
                &mut bytes,
                &json!({"seq":101,"body":{"text":"x".repeat(pad)}}),
            )
            .unwrap();
            bytes.push(b'\n');
            let start = bytes.len() - 512 * 1024;
            if (128..192).contains(&bytes[start]) {
                split = true;
                break;
            }
        }
        assert!(split);
        std::fs::write(dir.path().join("events/timeline.jsonl"), bytes).unwrap();
        let result = page(dir.path(), &Query::default()).unwrap();
        assert_eq!(
            result["events"].as_array().unwrap().last().unwrap()["seq"],
            101
        );
    }
}
