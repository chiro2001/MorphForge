use serde_json::Value;
use std::time::{Duration, Instant};
pub(super) struct ReasoningProgress {
    text: String,
    last: Instant,
    pub part: u64,
}
impl Default for ReasoningProgress {
    fn default() -> Self {
        Self {
            text: String::new(),
            last: Instant::now(),
            part: 0,
        }
    }
}
impl ReasoningProgress {
    pub fn push(&mut self, event: &Value) -> Option<String> {
        if let Some(text) = event["choices"][0]["delta"]["reasoning_content"].as_str() {
            self.text.push_str(text);
        }
        if matches!(
            event["type"].as_str(),
            Some("response.reasoning_text.delta" | "response.reasoning_summary_text.delta")
        ) {
            if let Some(text) = event["delta"].as_str() {
                self.text.push_str(text);
            }
        }
        if self.text.len() >= 4096 || self.last.elapsed() >= Duration::from_millis(800) {
            self.flush()
        } else {
            None
        }
    }
    pub fn flush(&mut self) -> Option<String> {
        if self.text.is_empty() {
            return None;
        }
        self.part += 1;
        self.last = Instant::now();
        Some(std::mem::take(&mut self.text))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn only_real_upstream_reasoning_is_shown() {
        let mut p = ReasoningProgress::default();
        p.push(&json!({"choices":[{"delta":{"reasoning_content":"实际思考"}}]}));
        p.push(&json!({"choices":[{"delta":{"content":"正文"}}]}));
        assert_eq!(p.flush().as_deref(), Some("实际思考"));
        assert!(p.flush().is_none());
    }
}
