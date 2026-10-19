//! The harness keeps one ordered Responses-shaped history. Only this boundary changes wire formats.
use super::config::Connection;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn text(parts: &Value) -> String {
    if let Some(s) = parts.as_str() {
        return s.into();
    }
    parts
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn chat_parts(parts: &Value) -> Result<Value> {
    if parts.is_string() {
        return Ok(parts.clone());
    }
    let mut out = Vec::new();
    for part in parts.as_array().context("消息content须为文本或数组")? {
        match part["type"].as_str().unwrap_or("") {
            "input_text" | "output_text" | "text" => {
                out.push(json!({"type":"text","text":part["text"]}))
            }
            "input_image" => {
                let url = part["image_url"]
                    .as_str()
                    .context("Chat图片须先解析为内嵌URL")?;
                out.push(json!({"type":"image_url","image_url":{"url":url,"detail":part.get("detail").and_then(Value::as_str).unwrap_or("high")}}));
            }
            other => bail!("无法转换Chat内容类型 {other}"),
        }
    }
    Ok(Value::Array(out))
}

fn flush_assistant(messages: &mut Vec<Value>, assistant: &mut Option<Value>) {
    if let Some(value) = assistant.take() {
        messages.push(value);
    }
}
fn flush_images(messages: &mut Vec<Value>, images: &mut Vec<Value>) {
    if !images.is_empty() {
        let mut content =
            vec![json!({"type":"text","text":"以上工具结果对应的真实截图；按标签检查视图。"})];
        content.append(images);
        messages.push(json!({"role":"user","content":content}));
    }
}

pub(super) fn chat_request(payload: &Value, connection: &Connection) -> Result<Value> {
    let mut messages = Vec::new();
    if let Some(instructions) = payload["instructions"].as_str() {
        messages.push(json!({"role":"system","content":instructions}));
    }
    let mut assistant: Option<Value> = None;
    let mut images = Vec::new();
    for item in payload["input"].as_array().context("缺少有序input")? {
        let kind = item["type"].as_str().unwrap_or("");
        if kind != "function_call_output" {
            flush_images(&mut messages, &mut images);
        }
        match kind {
            "reasoning" => {} // Provider-specific reasoning items are not chat messages.
            "function_call" => {
                let pending =
                    assistant.get_or_insert_with(|| json!({"role":"assistant","content":null}));
                if !pending["tool_calls"].is_array() {
                    pending["tool_calls"] = json!([]);
                }
                pending["tool_calls"].as_array_mut().unwrap().push(json!({
                    "id":item["call_id"],"type":"function",
                    "function":{"name":item["name"],"arguments":item["arguments"]}
                }));
            }
            "function_call_output" => {
                flush_assistant(&mut messages, &mut assistant);
                messages.push(json!({"role":"tool","tool_call_id":item["call_id"],"content":text(&item["output"])}));
                let mut image_label = None;
                for part in item["output"].as_array().into_iter().flatten() {
                    if part["type"] == "input_text" {
                        image_label = part["text"].as_str();
                    }
                    if part["type"] == "input_image" {
                        let url = part["image_url"]
                            .as_str()
                            .context("Chat工具图片缺少内嵌数据")?;
                        if let Some(label) = image_label.take() {
                            images.push(json!({"type":"text","text":label}));
                        }
                        images.push(json!({"type":"image_url","image_url":{"url":url,"detail":part.get("detail").and_then(Value::as_str).unwrap_or("high")}}));
                    }
                }
            }
            "message" | "" => {
                let role = item["role"].as_str().context("消息缺少role")?;
                if role == "assistant" {
                    let pending =
                        assistant.get_or_insert_with(|| json!({"role":"assistant","content":null}));
                    let content = text(&item["content"]);
                    if !content.is_empty() {
                        let previous = pending["content"].as_str().unwrap_or("");
                        pending["content"] = json!(if previous.is_empty() {
                            content
                        } else {
                            format!("{previous}\n{content}")
                        });
                    }
                    if let Some(reasoning) = item.get("chat_reasoning_content").filter(|_| {
                        item["_mf_provider"]
                            .as_str()
                            .is_none_or(|id| id == connection.identity())
                    }) {
                        pending["reasoning_content"] = reasoning.clone();
                    }
                } else {
                    flush_assistant(&mut messages, &mut assistant);
                    messages.push(json!({"role":role,"content":chat_parts(&item["content"])?}));
                }
            }
            other => bail!("无法转换Chat历史项 {other}"),
        }
    }
    flush_assistant(&mut messages, &mut assistant);
    flush_images(&mut messages, &mut images);
    let mut request =
        json!({"model":payload["model"],"messages":messages,"stream":connection.stream});
    if connection.stream {
        request["stream_options"] = json!({"include_usage":true});
    }
    request[&connection.chat_token_field] = payload["max_output_tokens"].clone();
    if let Some(tools) = payload["tools"]
        .as_array()
        .filter(|tools| !tools.is_empty())
    {
        request["tools"] = json!(tools
            .iter()
            .map(|tool| {
                let mut function = tool.clone();
                function.as_object_mut().unwrap().remove("type");
                json!({"type":"function","function":function})
            })
            .collect::<Vec<_>>());
    }
    if connection.send_reasoning {
        if let Some(effort) = payload["reasoning"]["effort"].as_str() {
            let official = reqwest::Url::parse(&connection.base_url)
                .ok()
                .and_then(|u| u.host_str().map(str::to_owned))
                .as_deref()
                == Some("api.deepseek.com");
            if official {
                request["thinking"] = json!({"type":if effort=="none"{"disabled"}else{"enabled"}});
            }
            if !official || effort != "none" {
                request["reasoning_effort"] = json!(effort);
            }
        }
    }
    Ok(request)
}

pub(super) fn chat_response(value: &Value) -> Result<Value> {
    let choice = value["choices"].get(0).context("Chat响应缺少choices[0]")?;
    let message = &choice["message"];
    let reason = choice["finish_reason"]
        .as_str()
        .context("Chat响应缺少finish_reason，未执行工具")?;
    let status = match reason {
        "stop" | "tool_calls" | "function_call" => "completed",
        "length" => "incomplete",
        _ => "failed",
    };
    let mut output = Vec::new();
    let content = text(&message["content"]);
    if !content.is_empty() || message["reasoning_content"].is_string() {
        let mut item = json!({"type":"message","role":"assistant","content":if content.is_empty(){vec![]}else{vec![json!({"type":"output_text","text":content})]}});
        if let Some(reasoning) = message.get("reasoning_content") {
            item["chat_reasoning_content"] = reasoning.clone();
        }
        output.push(item);
    }
    for call in message["tool_calls"].as_array().into_iter().flatten() {
        output.push(json!({"type":"function_call","call_id":call["id"],"name":call["function"]["name"],"arguments":call["function"]["arguments"]}));
    }
    Ok(json!({"id":value["id"],"status":status,"output":output,
        "incomplete_details":if status=="incomplete"{json!({"reason":"max_output_tokens"})}else{json!({"reason":reason})},
        "usage":super::usage::normalize_usage(&value["usage"])}))
}

#[derive(Default)]
pub(super) struct ChatStream {
    id: Value,
    text: String,
    reasoning: String,
    calls: BTreeMap<u64, Value>,
    finish_reason: Option<String>,
    usage: Value,
}

fn append_fragment(target: &mut Value, fragment: &str, repeatable: bool) {
    let old = target.as_str().unwrap_or("");
    if repeatable && old == fragment {
        return;
    }
    *target = json!(format!("{old}{fragment}"));
}

impl ChatStream {
    pub(super) fn push(&mut self, event: &Value) -> Result<Option<String>> {
        if let Some(error) = event.get("error") {
            bail!(
                "Chat流返回错误：{}",
                error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
            )
        }
        if let Some(id) = event.get("id") {
            self.id = id.clone();
        }
        if event["usage"].is_object() {
            self.usage = event["usage"].clone();
        }
        let mut text_delta = None;
        for choice in event["choices"].as_array().into_iter().flatten() {
            if choice["index"].as_u64().unwrap_or(0) != 0 {
                continue;
            }
            if let Some(reason) = choice["finish_reason"].as_str() {
                self.finish_reason = Some(reason.into());
            }
            let delta = &choice["delta"];
            if let Some(content) = delta["content"].as_str() {
                self.text.push_str(content);
                text_delta = Some(content.into());
            }
            if let Some(reasoning) = delta["reasoning_content"].as_str() {
                self.reasoning.push_str(reasoning);
            }
            for call in delta["tool_calls"].as_array().into_iter().flatten() {
                let index = call["index"].as_u64().context("Chat工具片段缺少index")?;
                let stored = self.calls.entry(index).or_insert_with(
                    || json!({"id":"","type":"function","function":{"name":"","arguments":""}}),
                );
                if let Some(id) = call["id"].as_str() {
                    append_fragment(&mut stored["id"], id, true);
                }
                if let Some(name) = call["function"]["name"].as_str() {
                    append_fragment(&mut stored["function"]["name"], name, true);
                }
                if let Some(arguments) = call["function"]["arguments"].as_str() {
                    append_fragment(&mut stored["function"]["arguments"], arguments, false);
                }
            }
        }
        Ok(text_delta)
    }

    pub(super) fn finish(self) -> Result<Value> {
        let reason = self
            .finish_reason
            .context("Chat流没有finish_reason，未执行部分工具")?;
        let mut message = json!({"role":"assistant","content":self.text,"tool_calls":self.calls.into_values().collect::<Vec<_>>()});
        if !self.reasoning.is_empty() {
            message["reasoning_content"] = json!(self.reasoning);
        }
        chat_response(
            &json!({"id":self.id,"choices":[{"message":message,"finish_reason":reason}],"usage":self.usage}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn images_follow_all_tool_results_and_preserve_reasoning() {
        let connection = Connection::default();
        let payload = json!({"model":"arbitrary-model","instructions":"中文说明","max_output_tokens":1234,"input":[
            {"type":"message","role":"assistant","content":[],"chat_reasoning_content":"检查图像"},
            {"type":"function_call","call_id":"a","name":"capture_views","arguments":"{}"},
            {"type":"function_call","call_id":"b","name":"get_context","arguments":"{}"},
            {"type":"function_call_output","call_id":"a","output":[{"type":"input_text","text":"hero"},{"type":"input_image","image_url":"data:image/png;base64,fixture"}]},
            {"type":"function_call_output","call_id":"b","output":[{"type":"input_text","text":"context"}]},
        ]});
        let result = chat_request(&payload, &connection).unwrap();
        let messages = result["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 5);
        assert_eq!(messages[1]["tool_calls"].as_array().unwrap().len(), 2);
        assert_eq!(messages[1]["reasoning_content"], "检查图像");
        assert_eq!(messages[2]["role"], "tool");
        assert_eq!(messages[3]["tool_call_id"], "b");
        assert_eq!(messages[4]["role"], "user");
        assert_eq!(messages[4]["content"][1]["text"], "hero");
        assert_eq!(messages[4]["content"][2]["type"], "image_url");
        assert_eq!(result["max_tokens"], 1234);
    }

    #[test]
    fn streamed_tools_usage_and_truncation_are_normalized() {
        let mut stream = ChatStream::default();
        stream.push(&json!({"choices":[{"index":0,"delta":{"reasoning_content":"考虑","tool_calls":[{"index":0,"id":"call-1","function":{"name":"run_recipe","arguments":"{\"code\":\""}}]}}]})).unwrap();
        stream.push(&json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"中文\"}"}}]},"finish_reason":"tool_calls"}]})).unwrap();
        stream.push(&json!({"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":20,"total_tokens":30}})).unwrap();
        let response = stream.finish().unwrap();
        assert_eq!(response["status"], "completed");
        assert_eq!(response["usage"]["total_tokens"], 30);
        assert_eq!(response["output"][1]["arguments"], "{\"code\":\"中文\"}");
        assert!(ChatStream::default().finish().is_err());
        let truncated = chat_response(
            &json!({"choices":[{"message":{"tool_calls":[]},"finish_reason":"length"}]}),
        )
        .unwrap();
        assert_eq!(truncated["status"], "incomplete");
    }
    #[test]
    fn official_chat_thinking_toggle_is_explicit_and_none_is_not_an_effort() {
        let connection = Connection::default();
        let base = json!({"model":"deepseek-flash","input":[],"max_output_tokens":2048,"reasoning":{"effort":"none"}});
        let none = chat_request(&base, &connection).unwrap();
        assert_eq!(none["thinking"]["type"], "disabled");
        assert!(none.get("reasoning_effort").is_none());
        let mut high = base;
        high["reasoning"]["effort"] = json!("high");
        let high = chat_request(&high, &connection).unwrap();
        assert_eq!(high["thinking"]["type"], "enabled");
        assert_eq!(high["reasoning_effort"], "high");
    }
}
