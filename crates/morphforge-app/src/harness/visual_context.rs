//! Bound screenshot replay in request copies, preserving the complete local history.
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashSet;

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VisualContextStats {
    pub retained_captures: usize,
    pub omitted_captures: usize,
}

pub(super) fn retain_latest_views(input: &mut [Value]) -> VisualContextStats {
    let captures: HashSet<String> = input
        .iter()
        .filter(|item| item["type"] == "function_call" && item["name"] == "capture_views")
        .filter_map(|item| item["call_id"].as_str().map(str::to_owned))
        .collect();
    let mut seen_views = HashSet::new();
    let mut stats = VisualContextStats::default();
    for item in input.iter_mut().rev() {
        if item["type"] != "function_call_output"
            || !item["call_id"]
                .as_str()
                .is_some_and(|id| captures.contains(id))
        {
            continue;
        }
        let Some(parts) = item["output"].as_array_mut() else {
            continue;
        };
        let Some(result) = parts
            .first()
            .and_then(|part| part["text"].as_str())
            .and_then(|text| serde_json::from_str::<Value>(text).ok())
            .filter(|result| result["ok"] == true)
        else {
            continue;
        };
        let Some(images) = result["images"].as_array() else {
            continue;
        };
        // Images are appended in result.images order. Matching the artifact as well
        // avoids filtering unknown or malformed historical content.
        for (part, image) in parts
            .iter_mut()
            .filter(|part| part["type"] == "input_image")
            .zip(images)
        {
            let Some(view) = image["view"]
                .as_str()
                .filter(|view| ["hero", "front", "top", "side", "detail"].contains(view))
            else {
                continue;
            };
            if part["artifact"].as_str().is_none() || part["artifact"] != image["artifact"] {
                continue;
            }
            if seen_views.insert(view.to_owned()) {
                stats.retained_captures += 1;
            } else {
                *part = json!({"type":"input_text","text":format!(
                    "[历史作品截图像素省略：view={view}，revision={}，artifact={}；本请求中有该视角的较新截图。原图和记录仍保存在本机。 / Older capture pixels omitted; a newer capture of this view is included. The original artifact remains available locally.]",
                    result["revision"], image["artifact"].as_str().unwrap_or_default()
                )});
                stats.omitted_captures += 1;
            }
        }
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capture(id: &str, revision: u64, views: &[(&str, &str)]) -> Vec<Value> {
        let images: Vec<_> = views
            .iter()
            .map(|(view, artifact)| json!({"view":view,"artifact":artifact}))
            .collect();
        let mut parts = vec![json!({"type":"input_text","text":json!({
            "ok":true,"revision":revision,"images":images
        }).to_string()})];
        parts.extend(
            views
                .iter()
                .map(|(_, artifact)| json!({"type":"input_image","artifact":artifact})),
        );
        vec![
            json!({"type":"function_call","call_id":id,"name":"capture_views"}),
            json!({"type":"function_call_output","call_id":id,"output":parts}),
        ]
    }

    #[test]
    fn newest_view_wins_without_losing_other_views_or_reference_images() {
        let mut input = capture("old", 1, &[("front", "old.png"), ("hero", "hero.png")]);
        input.extend(capture("new", 2, &[("front", "new.png")]));
        input.push(
            json!({"role":"user","content":[{"type":"input_image","artifact":"reference.png"}]}),
        );
        let history = input.clone();
        let stats = retain_latest_views(&mut input);
        assert_eq!(stats.omitted_captures, 1);
        assert_eq!(stats.retained_captures, 2);
        assert_eq!(input[1]["output"][1]["type"], "input_text");
        assert!(input[1]["output"][1]["text"]
            .as_str()
            .unwrap()
            .contains("old.png"));
        assert_eq!(input[1]["output"][2]["artifact"], "hero.png");
        assert_eq!(input[3]["output"][1]["artifact"], "new.png");
        assert_eq!(input[4], history[4]);
        assert_eq!(history[1]["output"][1]["type"], "input_image");
    }

    #[test]
    fn noncapture_tool_images_and_unknown_history_remain_intact() {
        let mut input = capture("other", 1, &[("front", "other.png")]);
        input[0]["name"] = json!("artworks");
        input.extend(capture("latest", 2, &[("front", "new.png")]));
        let unknown = json!({"type":"function_call_output","call_id":"missing","output":[{"type":"input_image","artifact":"unknown.png"}]});
        input.push(unknown.clone());
        let stats = retain_latest_views(&mut input);
        assert_eq!(stats.omitted_captures, 0);
        assert_eq!(input[1]["output"][1]["artifact"], "other.png");
        assert_eq!(input.last(), Some(&unknown));
    }
}
