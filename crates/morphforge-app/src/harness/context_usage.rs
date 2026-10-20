//! UI-only context accounting. Never inserts metadata into model history.
use super::{estimate, omit_images, prompts, tools_for_language, Conversation};
use crate::provider::{Connection, ProviderSettings};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextUsageAnchor {
    pub request_id: String,
    pub scope: String,
    pub input_tokens: u64,
    pub history_messages: usize,
    pub history_tokens_estimate: usize,
    pub measured_at: String,
}

pub(super) fn decode_anchor<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<ContextUsageAnchor>, D::Error> {
    let value = Option::<Value>::deserialize(d)?;
    // A display-only calibration must not make an otherwise valid saved
    // conversation unreadable after a version change or metadata corruption.
    Ok(value.and_then(|v| serde_json::from_value(v).ok()))
}

pub(super) fn provider_scope(connection: &Connection) -> String {
    crate::store::hash(
        &serde_json::to_vec(
            &json!({"identity":connection.identity(),"config":connection.status()}),
        )
        .expect("provider scope serializes"),
    )
}

pub(super) fn scope(c: &Conversation, s: &ProviderSettings, provider_identity: &str) -> String {
    crate::store::hash(
        &serde_json::to_vec(&json!({
            "conversation":c.id,"compact":c.compact_count,"provider":provider_identity,
            "model":s.model,"reasoning":s.reasoning,"language":s.language,
            "preset":s.preset,"detail":s.detail_instructions,"images":s.send_images,
            "prompt":c.schema_prompt,"tools":c.schema_tools,
        }))
        .expect("context scope serializes"),
    )
}

fn history_estimate(input: &[Value], send_images: bool) -> usize {
    if send_images {
        estimate(input)
    } else {
        let mut delivered = input.to_vec();
        omit_images(&mut delivered);
        estimate(&delivered)
    }
}

pub(super) fn meter(
    c: &Conversation,
    s: &ProviderSettings,
    identity: &str,
    window: usize,
) -> Value {
    let scope = scope(c, s, identity);
    let anchor = c
        .context_usage
        .as_ref()
        .filter(|a| a.scope == scope && a.history_messages <= c.input.len());
    let (used, history, source) = if let Some(a) = anchor {
        // Ordinary history is immutable and only grows. Estimate only the
        // unsampled suffix instead of rescanning a million-token prefix.
        let added = history_estimate(&c.input[a.history_messages..], s.send_images);
        (
            a.input_tokens.saturating_add(added as u64),
            a.history_tokens_estimate.saturating_add(added),
            "upstream_input_plus_estimated_growth",
        )
    } else {
        let history = history_estimate(&c.input, s.send_images);
        let system = format!(
            "{}\n{}\n{}",
            prompts::system_prompt(&s.language),
            prompts::preset_prompt(&s.preset, &s.language),
            s.detail_instructions
        );
        let fixed = system
            .len()
            .div_ceil(3)
            .saturating_add(estimate(&[tools_for_language(&s.language)]))
            .saturating_add(2048);
        (
            history.saturating_add(fixed) as u64,
            history,
            "local_estimate",
        )
    };
    json!({
        "usedTokens":used,"windowTokens":window,"usedPercent":if window>0 {Some(used as f64/window as f64*100.)}else{None},
        "remainingTokens":(window as u64).saturating_sub(used),
        "historyTokensEstimate":history,"estimated":true,"source":source,
        "upstreamInputTokens":anchor.map(|a|a.input_tokens),"lastMeasuredAt":anchor.map(|a|a.measured_at.as_str()),
        "note":"当前会话输入上下文估算，含系统说明与工具。以上次上游输入usage校准并估算新增历史；图片和tokenizer可能影响精度。不是累计计费用量，也不把尚未生成的输出预算计作已用。"
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calibrated_input_adds_only_new_history_and_survives_serialization() {
        let settings = ProviderSettings::default();
        let mut c = Conversation {
            id: "test".into(),
            input: vec![json!({"role":"user","content":"one"})],
            ..Default::default()
        };
        let old = history_estimate(&c.input, true);
        c.context_usage = Some(ContextUsageAnchor {
            request_id: "a".into(),
            scope: scope(&c, &settings, "provider"),
            input_tokens: 125000,
            history_messages: 1,
            history_tokens_estimate: old,
            measured_at: "fixture".into(),
        });
        let extra = json!({"role":"assistant","content":"新增内容"});
        let growth = estimate(std::slice::from_ref(&extra));
        c.input.push(extra);
        let restored: Conversation =
            serde_json::from_value(serde_json::to_value(c).unwrap()).unwrap();
        let m = meter(&restored, &settings, "provider", 1000000);
        assert_eq!(m["usedTokens"], 125000 + growth);
        assert_eq!(m["historyTokensEstimate"], old + growth);
        assert_eq!(m["upstreamInputTokens"], 125000);
        assert_eq!(m["source"], "upstream_input_plus_estimated_growth");
        assert!(
            (m["usedPercent"].as_f64().unwrap() - (125000 + growth) as f64 / 10000.).abs() < 1e-9
        );
    }

    #[test]
    fn compaction_and_model_provider_changes_discard_stale_calibration() {
        let settings = ProviderSettings::default();
        let mut c = Conversation::default();
        c.context_usage = Some(ContextUsageAnchor {
            request_id: "a".into(),
            scope: scope(&c, &settings, "provider"),
            input_tokens: 999999,
            history_messages: 0,
            history_tokens_estimate: 0,
            measured_at: String::new(),
        });
        assert_eq!(
            meter(&c, &settings, "provider", 1000000)["usedTokens"],
            999999
        );
        let mut changed = settings.clone();
        changed.model = "another-tokenizer".into();
        assert_eq!(
            meter(&c, &changed, "provider", 1000000)["source"],
            "local_estimate"
        );
        assert_eq!(
            meter(&c, &settings, "other-provider", 1000000)["source"],
            "local_estimate"
        );
        c.compact_count += 1;
        let m = meter(&c, &settings, "provider", 1000000);
        assert_eq!(m["source"], "local_estimate");
        assert!(m["usedTokens"].as_u64().unwrap() < 999999);
        assert!(
            m["usedTokens"].as_u64().unwrap() > 0,
            "static instructions and tools occupy context even with empty history"
        );
    }

    #[test]
    fn input_estimates_respect_image_policy_and_do_not_change_history() {
        let input = vec![
            json!({"role":"user","content":[{"type":"input_text","text":"图片"},{"type":"input_image","artifact":"local.png"}]}),
        ];
        let original = input.clone();
        assert!(history_estimate(&input, true) > history_estimate(&input, false));
        assert_eq!(input, original);
    }

    #[test]
    fn invalid_optional_calibration_does_not_discard_conversation_history() {
        let mut value = serde_json::to_value(Conversation {
            input: vec![json!({"role":"user","content":"keep"})],
            ..Default::default()
        })
        .unwrap();
        value["contextUsage"] = json!({"invalid":"old calibration"});
        let restored: Conversation = serde_json::from_value(value).unwrap();
        assert_eq!(restored.input.len(), 1);
        assert!(restored.context_usage.is_none());
        let a = Connection::default();
        let mut b = a.clone();
        b.send_reasoning = !a.send_reasoning;
        assert_ne!(provider_scope(&a), provider_scope(&b));
    }
}
