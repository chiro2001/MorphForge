//! Observe the actual wire-message prefix after protocol and image conversion.
//! Store hashes only; this is a structural diagnostic, not a token/cache estimate.
use super::{config::Connection, ProviderRequestTrace};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Serialize, Deserialize)]
struct Snapshot {
    connection: String,
    static_hash: String,
    messages: Vec<String>,
    context_epoch: Option<u32>,
    image_policy: Option<bool>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub(super) struct PrefixTracker {
    previous: Option<Snapshot>,
    #[serde(default)]
    authorized_reset: Option<String>,
}

impl PrefixTracker {
    pub fn authorize(&mut self, reason: &str) {
        self.authorized_reset = Some(reason.into());
    }

    pub(super) fn observe(
        &mut self,
        payload: &Value,
        connection: &Connection,
        trace: &ProviderRequestTrace,
    ) -> Value {
        if trace.compact {
            return json!({"scope":"wire_messages","reason":"compact_request","appendOnly":null,"expectedReset":true});
        }
        let field = if payload.get("messages").is_some() {
            "messages"
        } else {
            "input"
        };
        let messages = payload[field]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        let hashes = messages
            .iter()
            .map(|message| crate::store::hash(&serde_json::to_vec(message).unwrap()))
            .collect::<Vec<_>>();
        let mut static_config = json!({
            "model":payload["model"],"instructions":payload["instructions"],"tools":payload["tools"],
            "reasoning":payload["reasoning"],"reasoningEffort":payload["reasoning_effort"],
            "chatTemplateKwargs":payload["chat_template_kwargs"],"toolChoice":payload["tool_choice"],
            "leadingInstructions":messages.iter().take_while(|item|matches!(item["role"].as_str(),Some("system"|"developer"))).collect::<Vec<_>>()
        });
        if !payload["thinking"].is_null() {
            static_config["thinking"] = payload["thinking"].clone();
        }
        let current = Snapshot {
            connection: connection.identity(),
            static_hash: crate::store::hash(&serde_json::to_vec(&static_config).unwrap()),
            messages: hashes,
            context_epoch: trace.context_epoch,
            image_policy: trace.image_policy,
        };
        let report = if let Some(previous) = &self.previous {
            let common = previous
                .messages
                .iter()
                .zip(&current.messages)
                .take_while(|(a, b)| a == b)
                .count();
            let same_static = previous.connection == current.connection
                && previous.static_hash == current.static_hash;
            let append_only = same_static && common == previous.messages.len();
            let reason = if previous.connection != current.connection {
                "provider_changed"
            } else if previous.context_epoch != current.context_epoch {
                "compact_or_context_reset"
            } else if previous.image_policy != current.image_policy {
                "image_policy_changed"
            } else if !same_static {
                "prefix_configuration_changed"
            } else if !append_only {
                "history_rewrite"
            } else {
                "append_only"
            };
            json!({
                "scope":"wire_messages","field":field,"appendOnly":append_only,"reason":reason,
                "expectedReset":self.authorized_reset.is_some() || reason=="compact_or_context_reset",
                "resetAuthorization":self.authorized_reset,
                "staticPrefixUnchanged":same_static,"sharedMessages":common,
                "previousMessages":previous.messages.len(),"currentMessages":current.messages.len(),
                "firstChangedMessage":if common<previous.messages.len(){Some(common)}else{None},
                "contextEpoch":trace.context_epoch
            })
        } else {
            json!({"scope":"wire_messages","field":field,"appendOnly":null,"reason":"first_request",
                   "currentMessages":current.messages.len(),"expectedReset":false,"contextEpoch":trace.context_epoch})
        };
        self.authorized_reset = None;
        self.previous = Some(current);
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wire_diagnostic_detects_history_rewrites_and_expected_compaction() {
        let mut tracker = PrefixTracker::default();
        let connection = Connection::default();
        let trace = ProviderRequestTrace {
            context_epoch: Some(0),
            ..Default::default()
        };
        let first =
            json!({"model":"x","instructions":"fixed","input":[{"role":"user","content":"one"}]});
        assert_eq!(
            tracker.observe(&first, &connection, &trace)["reason"],
            "first_request"
        );
        let mut next = first.clone();
        next["input"]
            .as_array_mut()
            .unwrap()
            .push(json!({"role":"assistant","content":"two"}));
        assert_eq!(
            tracker.observe(&next, &connection, &trace)["appendOnly"],
            true
        );
        next["input"][0]["content"] = json!("rewritten");
        assert_eq!(
            tracker.observe(&next, &connection, &trace)["reason"],
            "history_rewrite"
        );
        let compact = ProviderRequestTrace {
            context_epoch: Some(0),
            compact: true,
            ..Default::default()
        };
        assert_eq!(
            tracker.observe(&json!({}), &connection, &compact)["reason"],
            "compact_request"
        );
        let new_epoch = ProviderRequestTrace {
            context_epoch: Some(1),
            ..Default::default()
        };
        assert_eq!(
            tracker.observe(&first, &connection, &new_epoch)["reason"],
            "compact_or_context_reset"
        );
    }
}
