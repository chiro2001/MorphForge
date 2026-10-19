//! Upstream-reported usage, restored from the durable event journal.
//! Local token estimates deliberately stay separate from billing/cache metrics.
use crate::store::Event;
use anyhow::Result;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, VecDeque},
    io::{BufRead, BufReader},
    path::Path,
};

pub(super) fn normalize_usage(value: &Value) -> Value {
    let Some(object) = value.as_object() else {
        return Value::Null;
    };
    let mut result = object.clone();
    let input = value["input_tokens"]
        .as_u64()
        .or_else(|| value["prompt_tokens"].as_u64())
        .or_else(|| {
            Some(
                value["prompt_cache_hit_tokens"]
                    .as_u64()?
                    .saturating_add(value["prompt_cache_miss_tokens"].as_u64()?),
            )
        });
    let output = value["output_tokens"]
        .as_u64()
        .or_else(|| value["completion_tokens"].as_u64());
    let total = value["total_tokens"]
        .as_u64()
        .or_else(|| Some(input?.saturating_add(output?)));
    result.insert("input_tokens".into(), json!(input));
    result.insert("output_tokens".into(), json!(output));
    result.insert("total_tokens".into(), json!(total));
    let cached = value["input_tokens_details"]["cached_tokens"]
        .as_u64()
        .or_else(|| value["prompt_tokens_details"]["cached_tokens"].as_u64())
        .or_else(|| value["prompt_cache_hit_tokens"].as_u64());
    if let Some(cached) = cached {
        let details = result
            .entry("input_tokens_details")
            .or_insert_with(|| json!({}));
        if !details.is_object() {
            *details = json!({});
        }
        details["cached_tokens"] = json!(cached);
    }
    Value::Object(result)
}

#[derive(Clone, Default)]
struct Sample {
    run: String,
    input: Option<u64>,
    output: Option<u64>,
    total: Option<u64>,
    cached: Option<u64>,
}
impl Sample {
    fn from_usage(run: &str, usage: &Value) -> Self {
        let value = normalize_usage(usage);
        let input = value["input_tokens"].as_u64();
        let cached = value["input_tokens_details"]["cached_tokens"]
            .as_u64()
            .filter(|cached| input.is_some_and(|input| *cached <= input));
        Self {
            run: run.into(),
            input,
            output: value["output_tokens"].as_u64(),
            total: value["total_tokens"].as_u64(),
            cached,
        }
    }
    fn merge(&mut self, old: &Self) {
        self.input = self.input.or(old.input);
        self.output = self.output.or(old.output);
        self.total = self.total.or(old.total);
        self.cached = self.cached.or(old.cached);
    }
}

#[derive(Default)]
struct Totals {
    requests: u64,
    reported: u64,
    input: u64,
    output: u64,
    total: u64,
    cached: u64,
    cache_input: u64,
    cache_requests: u64,
}
impl Totals {
    fn apply(&mut self, s: &Sample, add: bool) {
        let op = |value: &mut u64, n: u64| {
            *value = if add {
                value.saturating_add(n)
            } else {
                value.saturating_sub(n)
            };
        };
        op(&mut self.requests, 1);
        op(&mut self.reported, u64::from(s.total.is_some()));
        op(&mut self.input, s.input.unwrap_or(0));
        op(&mut self.output, s.output.unwrap_or(0));
        op(&mut self.total, s.total.unwrap_or(0));
        if let Some(cached) = s.cached {
            op(&mut self.cached, cached);
            op(&mut self.cache_input, s.input.unwrap_or(0));
            op(&mut self.cache_requests, 1);
        }
    }
    fn value(&self) -> Value {
        json!({"requests":self.requests,"reportedRequests":self.reported,"unreportedRequests":self.requests.saturating_sub(self.reported),
            "inputTokens":self.input,"outputTokens":self.output,"totalTokens":self.total,
            "cachedInputTokens":self.cached,"cacheReportedInputTokens":self.cache_input,"cacheReportedRequests":self.cache_requests,
            "prefixHitRate":if self.cache_input>0 {Some(self.cached as f64/self.cache_input as f64)}else{None}})
    }
}

#[derive(Default)]
pub(super) struct UsageLedger {
    total: Totals,
    runs: BTreeMap<String, Totals>,
    run_order: VecDeque<String>,
    samples: BTreeMap<String, Sample>,
    sample_order: VecDeque<String>,
}
impl UsageLedger {
    pub(super) fn load(root: &Path) -> Result<Self> {
        let mut ledger = Self::default();
        let path = root.join("events/events.jsonl");
        if path.exists() {
            for line in BufReader::new(std::fs::File::open(path)?).lines() {
                let Ok(line) = line else {
                    continue;
                };
                if let Ok(event) = serde_json::from_str::<Event>(&line) {
                    ledger.observe(&event);
                }
            }
        }
        Ok(ledger)
    }
    pub(super) fn relevant(kind: &str) -> bool {
        matches!(
            kind,
            "provider.response" | "provider.incomplete" | "provider.cancelled"
        )
    }
    pub(super) fn observe(&mut self, event: &Event) {
        if !Self::relevant(&event.kind) || event.data["phase"] == "before_send" {
            return;
        }
        let Some(id) = event.data["requestId"].as_str() else {
            return;
        };
        let run = event.run_id.as_deref().unwrap_or("");
        let data = &event.data;
        let usage = [
            &data["body"]["usage"],
            &data["response"]["usage"],
            &data["terminal"]["response"]["usage"],
        ]
        .into_iter()
        .find(|v| v.is_object())
        .or_else(|| {
            data["frames"].as_array()?.iter().rev().find_map(|v| {
                [&v["usage"], &v["response"]["usage"]]
                    .into_iter()
                    .find(|v| v.is_object())
            })
        })
        .unwrap_or(&Value::Null);
        let mut sample = Sample::from_usage(run, usage);
        if let Some(old) = self.samples.get(id) {
            if old.run != run {
                return;
            }
            sample.merge(old);
            if sample
                .cached
                .is_some_and(|cached| sample.input.is_none_or(|input| cached > input))
            {
                sample.cached = None;
            }
            self.total.apply(old, false);
            if let Some(total) = self.runs.get_mut(run) {
                total.apply(old, false);
            }
        } else {
            self.sample_order.push_back(id.into());
            if self.sample_order.len() > 4096 {
                if let Some(old) = self.sample_order.pop_front() {
                    self.samples.remove(&old);
                }
            }
        }
        self.total.apply(&sample, true);
        if !self.runs.contains_key(run) {
            self.run_order.push_back(run.into());
            if self.run_order.len() > 256 {
                if let Some(old) = self.run_order.pop_front() {
                    self.runs.remove(&old);
                }
            }
        }
        self.runs
            .entry(run.into())
            .or_default()
            .apply(&sample, true);
        self.samples.insert(id.into(), sample);
    }
    pub(super) fn value(&self, run: Option<&str>) -> Value {
        json!({"total":self.total.value(),"run":run.map(|id|self.runs.get(id).map(Totals::value).unwrap_or_else(||Totals::default().value())),
            "scope":"workspace","source":"upstream_reported_usage","prefixDenominator":"cache_reported_input_tokens"})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(id: &str, usage: Value) -> Event {
        serde_json::from_value(json!({"seq":1,"time":"now","kind":"provider.response","runId":"run-a","revision":0,"data":{"requestId":id,"body":{"usage":usage}}})).unwrap()
    }
    #[test]
    fn supports_chat_responses_and_deepseek_usage_without_losing_cache_fields() {
        for usage in [
            json!({"input_tokens":1000,"output_tokens":100,"input_tokens_details":{"cached_tokens":600}}),
            json!({"prompt_tokens":1000,"completion_tokens":100,"prompt_tokens_details":{"cached_tokens":600}}),
            json!({"prompt_tokens":1000,"completion_tokens":100,"prompt_cache_hit_tokens":600,"prompt_cache_miss_tokens":400}),
        ] {
            let s = Sample::from_usage("r", &usage);
            assert_eq!(s.total, Some(1100));
            assert_eq!(s.cached, Some(600));
        }
    }
    #[test]
    fn weighted_cache_rate_counts_actual_tokens_once_and_handles_unknown_usage() {
        let mut ledger = UsageLedger::default();
        let a = event(
            "a",
            json!({"input_tokens":100,"output_tokens":10,"input_tokens_details":{"cached_tokens":100}}),
        );
        ledger.observe(&a);
        ledger.observe(&a);
        ledger.observe(&event("b",json!({"input_tokens":900,"output_tokens":90,"input_tokens_details":{"cached_tokens":0}})));
        ledger.observe(&event("c", json!({"total_tokens":50})));
        ledger.observe(&event("d", Value::Null));
        let v = ledger.value(Some("run-a"));
        assert_eq!(v["total"]["requests"], 4);
        assert_eq!(v["total"]["totalTokens"], 1150);
        assert_eq!(v["total"]["prefixHitRate"], 0.1);
        assert_eq!(v["total"]["cacheReportedRequests"], 2);
        assert_eq!(v["total"]["unreportedRequests"], 1);
    }
    #[test]
    fn absent_cache_is_not_zero_percent_and_incomplete_footer_usage_is_kept() {
        let mut ledger = UsageLedger::default();
        ledger.observe(&event("a", json!({"input_tokens":10,"output_tokens":1})));
        assert!(ledger.value(None)["total"]["prefixHitRate"].is_null());
        let mut e = event("b", Value::Null);
        e.kind = "provider.incomplete".into();
        e.data = json!({"requestId":"b","frames":[{"usage":{"prompt_tokens":100,"completion_tokens":20,"prompt_cache_hit_tokens":50}}]});
        ledger.observe(&e);
        assert_eq!(ledger.value(None)["total"]["totalTokens"], 131);
        assert_eq!(ledger.value(None)["total"]["prefixHitRate"], 0.5);
    }
}
