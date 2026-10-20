//! Persistent, local cost/cache guard. Only upstream usage is called a cache hit.
use super::{config::Connection, usage::normalize_usage, ProviderRequestTrace};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::VecDeque, path::Path};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Sample {
    input: u64,
    cached: Option<u64>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct CacheGuard {
    scope: String,
    mode: String,
    samples: VecDeque<Sample>,
    response_count: u64,
    last_request: String,
    input_estimate: u64,
    alert: Option<String>,
    prefix: Value,
    in_compact: bool,
    unreported_interruptions: u64,
    last_unreported_interruption: Option<Value>,
}
impl CacheGuard {
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join("cache-monitor.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        serde_json::from_slice(&std::fs::read(path)?).map_err(|e| {
            anyhow::anyhow!(
                "CACHE_STATE_INVALID：缓存监测文件无法读取，请先检查状态，未自动放行新请求：{e}"
            )
        })
    }
    pub fn save(&self, root: &Path) -> Result<()> {
        crate::store::write_json(&root.join("cache-monitor.json"), self)
    }
    pub fn prepare(
        &mut self,
        connection: &Connection,
        model: &Value,
        trace: &ProviderRequestTrace,
        prefix: &Value,
    ) -> Result<Value> {
        self.in_compact = trace.compact;
        if trace.compact {
            if (trace.cache_guard == "strict" || self.mode == "strict") && self.alert.is_some() {
                bail!("CACHE_GUARD_PAUSED：缓存告警尚未确认，compact不会绕过费用暂停");
            }
            return Ok(json!({"status":"compact","cacheRequired":false}));
        }
        let scope = format!(
            "{}:{model}:{}",
            connection.identity(),
            trace.context_epoch.unwrap_or_default()
        );
        if scope != self.scope || prefix["expectedReset"] == true {
            *self = Self {
                scope,
                ..Self::default()
            };
        }
        let official = reqwest::Url::parse(&connection.base_url)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned))
            .as_deref()
            == Some("api.deepseek.com");
        self.mode = match trace.cache_guard.as_str() {
            "strict" => "strict",
            "auto" if official => "strict",
            _ => "observe",
        }
        .into();
        self.prefix = prefix.clone();
        self.input_estimate = trace.estimated_input_tokens;
        if self.mode == "strict" {
            if let Some(alert) = &self.alert {
                bail!("CACHE_GUARD_PAUSED：{alert}；已保留草稿，请检查上游缓存后确认恢复");
            }
        }
        Ok(self.status())
    }
    pub fn block_prefix(&mut self, prefix: &Value) {
        self.prefix = prefix.clone();
        self.alert = Some("检测到未经授权的客户端前缀改写；请求未发送".into());
    }
    pub fn unfinished(&mut self, kind: &str, id: &str, data: &Value) -> Option<Value> {
        if self.in_compact || data["phase"] == "before_send" || self.last_request == id {
            return None;
        }
        let usage = data["response"]
            .get("usage")
            .filter(|usage| usage.is_object())
            .or_else(|| {
                data["terminal"]["response"]
                    .get("usage")
                    .filter(|usage| usage.is_object())
            })
            .or_else(|| {
                data["frames"].as_array()?.iter().rev().find_map(|frame| {
                    frame
                        .get("usage")
                        .filter(|usage| usage.is_object())
                        .or_else(|| {
                            frame["response"]
                                .get("usage")
                                .filter(|usage| usage.is_object())
                        })
                })
            })
            .cloned()
            .unwrap_or(Value::Null);
        // User cancellation cuts off the final usage frame on most streaming
        // APIs. Absence of that frame is not evidence of a cache miss. Keep an
        // explicit unknown entry, without inserting an estimated cache sample
        // or clearing any earlier, independently established alert. Actual
        // cache counters received before cancellation still use normal checks.
        if kind == "provider.cancelled"
            && normalize_usage(&usage)["input_tokens_details"]["cached_tokens"].is_null()
        {
            if self
                .last_unreported_interruption
                .as_ref()
                .is_none_or(|last| last["requestId"] != id)
            {
                self.unreported_interruptions = self.unreported_interruptions.saturating_add(1);
                self.last_unreported_interruption = Some(json!({
                    "requestId":id,"phase":data["phase"],"usageReported":usage.is_object(),
                    "cacheReported":false,"excludedFromCacheSamples":true,
                    "note":"请求已中断，未收到缓存结算信息；费用未知，不作为缓存未命中的证据。"
                }));
            }
            return Some(self.status());
        }
        Some(self.record(id, &json!({"usage":usage})))
    }
    pub fn record(&mut self, id: &str, value: &Value) -> Value {
        if self.last_request == id {
            return self.status();
        }
        self.last_request = id.into();
        let usage = normalize_usage(&value["usage"]);
        let input = usage["input_tokens"]
            .as_u64()
            .unwrap_or(self.input_estimate);
        let cached = usage["input_tokens_details"]["cached_tokens"]
            .as_u64()
            .filter(|n| *n <= input);
        let established = self
            .samples
            .iter()
            .any(|sample| sample.cached.is_some_and(|n| n > 0));
        self.response_count += 1;
        self.samples.push_back(Sample { input, cached });
        while self.samples.len() > 5 {
            self.samples.pop_front();
        }
        let zero = self
            .samples
            .iter()
            .rev()
            .take_while(|s| s.input >= 4096 && s.cached == Some(0))
            .count();
        let missing = self
            .samples
            .iter()
            .rev()
            .take_while(|s| s.cached.is_none())
            .count();
        let large = input.max(self.input_estimate) >= 131_072;
        let shared = self.prefix["appendOnly"] == true;
        self.alert = if large && cached.is_none_or(|n| n == 0) {
            Some("大输入请求没有得到有效缓存命中报告，已阻止继续发送".into())
        } else if shared && established && input >= 4096 && cached.is_none_or(|n| n == 0) {
            Some("此前已确认缓存命中，本次稳定前缀却未报告有效命中；暂停后续请求以止损".into())
        } else if shared && zero >= 3 {
            Some("连续三次稳定前缀请求未命中上游缓存".into())
        } else if self.response_count >= 2 && missing >= 2 {
            Some("上游连续未报告缓存信息，无法确认费用复用".into())
        } else if shared
            && self.samples.len() == 5
            && self
                .samples
                .iter()
                .all(|s| s.input >= 16_384 && s.cached.is_some())
            && self
                .samples
                .iter()
                .map(|s| s.cached.unwrap_or(0))
                .sum::<u64>() as f64
                / (self.samples.iter().map(|s| s.input).sum::<u64>() as f64)
                < 0.1
        {
            Some("最近五次上游缓存加权命中率低于10%；请检查路由及新增输入比例".into())
        } else {
            None
        };
        self.status()
    }
    pub fn acknowledge(&mut self) {
        self.alert = None;
        self.samples.clear();
        self.response_count = 0;
    }
    pub fn status(&self) -> Value {
        let (input, cached) = self
            .samples
            .iter()
            .filter_map(|s| Some((s.input, s.cached?)))
            .fold((0u64, 0u64), |(a, b), (i, c)| {
                (a.saturating_add(i), b.saturating_add(c))
            });
        let last = self.samples.back();
        json!({"mode":self.mode,"status":if self.alert.is_some(){"alert"}else if self.samples.len()<2{"warming"}else{"observing"},
            "cacheRequired":self.mode=="strict","alert":self.alert,"prefix":self.prefix,
            "requests":self.response_count,"windowRequests":self.samples.len(),
            "cacheReportedRequests":self.samples.iter().filter(|s|s.cached.is_some()).count(),
            "recentHitRate":if input>0 {Some(cached as f64/input as f64)}else{None},
            "lastInputTokens":last.map(|s|s.input),"lastCachedTokens":last.and_then(|s|s.cached),
            "lastHitRate":last.and_then(|s|s.cached.map(|c|if s.input>0{c as f64/s.input as f64}else{0.0})),
            "unreportedInterruptions":self.unreported_interruptions,"lastUnreportedInterruption":self.last_unreported_interruption,
            "source":"upstream_usage","clientPrefixIsNotCacheHit":true})
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn trace() -> ProviderRequestTrace {
        ProviderRequestTrace {
            cache_guard: "strict".into(),
            estimated_input_tokens: 20_000,
            ..Default::default()
        }
    }
    #[test]
    fn cancelled_unknown_usage_is_persisted_separately_from_cache_samples() {
        let mut g = CacheGuard::default();
        let c = Connection::default();
        let mut t = trace();
        t.estimated_input_tokens = 160_000;
        let p = json!({"appendOnly":true});
        g.prepare(&c, &json!("x"), &t, &p).unwrap();
        let data = json!({"phase":"streaming","frames":[{"type":"response.output_text.delta","delta":"partial"}]});
        let status = g.unfinished("provider.cancelled", "a", &data).unwrap();
        assert!(status["alert"].is_null());
        assert_eq!(status["windowRequests"], 0);
        assert!(status["recentHitRate"].is_null());
        assert_eq!(status["unreportedInterruptions"], 1);
        assert_eq!(status["lastUnreportedInterruption"]["cacheReported"], false);
        g.unfinished("provider.cancelled", "a", &data);
        assert_eq!(g.status()["unreportedInterruptions"], 1);
        let dir = tempfile::tempdir().unwrap();
        g.save(dir.path()).unwrap();
        let mut g = CacheGuard::load(dir.path()).unwrap();
        assert_eq!(g.status(), status);
        g.prepare(&c, &json!("x"), &t, &p).unwrap();
        g.record("b", &json!({"usage":{"input_tokens":160000,"input_tokens_details":{"cached_tokens":150000}}}));
        assert_eq!(g.status()["recentHitRate"], 0.9375);
        assert_eq!(g.status()["windowRequests"], 1);
    }

    #[test]
    fn cancellation_does_not_clear_a_real_alert_or_ignore_reported_cache_loss() {
        let c = Connection::default();
        let mut t = trace();
        t.estimated_input_tokens = 160_000;
        let p = json!({"appendOnly":true});
        let mut g = CacheGuard::default();
        g.prepare(&c, &json!("x"), &t, &p).unwrap();
        // A null placeholder must not hide valid counters in an earlier frame.
        g.unfinished(
            "provider.cancelled",
            "a",
            &json!({"phase":"streaming","frames":[
                {"usage":{"prompt_tokens":160000,"prompt_tokens_details":{"cached_tokens":0}}},
                {"usage":null}
            ]}),
        );
        let before = g.status();
        assert!(before["alert"].is_string());
        assert_eq!(before["lastCachedTokens"], 0);
        g.unfinished("provider.cancelled", "b", &json!({"phase":"sending"}));
        assert_eq!(g.status()["alert"], before["alert"]);
        assert!(g.prepare(&c, &json!("x"), &t, &p).is_err());
        g.unfinished("provider.cancelled", "c", &json!({"phase":"before_send"}));
        assert_eq!(g.status()["unreportedInterruptions"], 1);
    }

    #[test]
    fn incomplete_or_failed_requests_still_trigger_missing_cache_protection() {
        for kind in ["provider.incomplete", "provider.transport_error"] {
            let mut g = CacheGuard::default();
            let mut t = trace();
            t.estimated_input_tokens = 160_000;
            g.prepare(
                &Connection::default(),
                &json!("x"),
                &t,
                &json!({"appendOnly":true}),
            )
            .unwrap();
            let s = g.unfinished(kind, "a", &json!({})).unwrap();
            assert!(s["alert"].is_string(), "{kind}");
            assert_eq!(s["windowRequests"], 1);
        }
    }
    #[test]
    fn zero_cache_pauses_and_acknowledgement_does_not_change_prefix() {
        let mut g = CacheGuard::default();
        let c = Connection::default();
        let t = trace();
        let p = json!({"appendOnly":true});
        for i in 0..3 {
            g.prepare(&c, &json!("x"), &t, &p).unwrap();
            g.record(&i.to_string(),&json!({"usage":{"input_tokens":20000,"output_tokens":5,"input_tokens_details":{"cached_tokens":0}}}));
        }
        assert!(g.prepare(&c, &json!("x"), &t, &p).is_err());
        let mut compact = t.clone();
        compact.compact = true;
        assert!(g.prepare(&c, &json!("x"), &compact, &p).is_err());
        let dir = tempfile::tempdir().unwrap();
        g.save(dir.path()).unwrap();
        let mut g = CacheGuard::load(dir.path()).unwrap();
        assert!(g.prepare(&c, &json!("x"), &t, &p).is_err());
        g.acknowledge();
        assert!(g.prepare(&c, &json!("x"), &t, &p).is_ok());
    }
    #[test]
    fn unknown_is_not_zero_and_large_input_stops_immediately() {
        let mut g = CacheGuard::default();
        let c = Connection::default();
        let mut t = trace();
        t.estimated_input_tokens = 140_000;
        g.prepare(&c, &json!("x"), &t, &json!({"appendOnly":true}))
            .unwrap();
        let s = g.record("a", &json!({"usage":{"input_tokens":140000}}));
        assert!(s["recentHitRate"].is_null());
        assert!(s["alert"].is_string());
        assert!(g.prepare(&c, &json!("x"), &t, &json!({})).is_err());
    }
    #[test]
    fn established_cache_loss_stops_at_the_first_anomaly() {
        let mut g = CacheGuard::default();
        let c = Connection::default();
        let t = trace();
        let p = json!({"appendOnly":true});
        g.prepare(&c, &json!("x"), &t, &p).unwrap();
        g.record(
            "a",
            &json!({"usage":{"input_tokens":9000,"input_tokens_details":{"cached_tokens":8000}}}),
        );
        g.prepare(&c, &json!("x"), &t, &p).unwrap();
        g.record(
            "b",
            &json!({"usage":{"input_tokens":10000,"input_tokens_details":{"cached_tokens":0}}}),
        );
        assert!(g.prepare(&c, &json!("x"), &t, &p).is_err());
    }
    #[test]
    fn weighted_usage_and_compaction_remain_separate() {
        let mut g = CacheGuard::default();
        let c = Connection::default();
        let mut t = trace();
        g.prepare(&c, &json!("x"), &t, &json!({})).unwrap();
        g.record(
            "a",
            &json!({"usage":{"prompt_tokens":20000,"prompt_cache_hit_tokens":15000}}),
        );
        let before = g.status();
        t.compact = true;
        g.prepare(&c, &json!("x"), &t, &json!({})).unwrap();
        assert_eq!(g.status(), before);
        assert_eq!(g.status()["recentHitRate"], 0.75);
    }
    #[test]
    fn corrupt_monitor_state_is_not_silently_treated_as_a_new_session() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("cache-monitor.json"), b"{broken").unwrap();
        assert!(CacheGuard::load(dir.path()).is_err());
    }
    #[test]
    fn uncached_small_but_realistic_tool_context_stops_after_three_requests() {
        let mut g = CacheGuard::default();
        let c = Connection::default();
        let t = trace();
        let p = json!({"appendOnly":true});
        for i in 0..3 {
            g.prepare(&c, &json!("x"), &t, &p).unwrap();
            g.record(
                &i.to_string(),
                &json!({"usage":{"input_tokens":9000,"input_tokens_details":{"cached_tokens":0}}}),
            );
        }
        assert!(g.prepare(&c, &json!("x"), &t, &p).is_err());
    }
}
