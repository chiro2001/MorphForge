use super::{Provider, ProviderSettings};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const DEFAULT_CONTEXT_TOKENS: usize = 1_048_576;
pub const DEFAULT_OUTPUT_TOKENS: u32 = 393_216;
pub const DEFAULT_COMPACT_TOKENS: usize = 900_000;
pub const DEFAULT_RUN_TOKENS: u64 = 8_000_000;

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelLimits {
    pub context_window: Option<usize>,
    pub max_output_tokens: Option<u32>,
}
impl ModelLimits {
    pub fn context(&self, configured: usize) -> usize {
        self.context_window
            .map_or(configured, |limit| limit.min(configured))
    }
    pub fn output(&self, configured: u32) -> u32 {
        self.max_output_tokens
            .map_or(configured, |limit| limit.min(configured))
    }
    pub fn output_allowance(
        &self,
        settings: &ProviderSettings,
        input: usize,
        remaining: u64,
    ) -> anyhow::Result<u32> {
        let context = self.context(settings.context_window_tokens);
        let available = context.saturating_sub(input).saturating_sub(1024);
        if available == 0 {
            anyhow::bail!("CONTEXT_BUDGET：估算输入接近上游上下文上限，请compact或使用更大窗口；原始记录已保留")
        }
        Ok(self
            .output(settings.max_output_tokens)
            .min(available.min(u32::MAX as usize) as u32)
            .min(remaining.min(u32::MAX as u64) as u32))
    }
}

#[derive(Default)]
pub(super) struct LimitCache {
    pub connection: String,
    pub attempted: bool,
    pub models: BTreeMap<String, ModelLimits>,
}

pub(super) fn parse_models(value: &Value) -> BTreeMap<String, ModelLimits> {
    value["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|model| {
            let id = model["id"].as_str()?;
            let positive = |names: &[&str]| {
                names
                    .iter()
                    .find_map(|name| model[*name].as_u64().filter(|v| *v > 0))
            };
            Some((
                id.to_owned(),
                ModelLimits {
                    context_window: positive(&[
                        "context_window",
                        "max_model_len",
                        "context_length",
                        "max_context_length",
                    ])
                    .and_then(|n| usize::try_from(n).ok()),
                    max_output_tokens: positive(&["max_output_tokens", "max_completion_tokens"])
                        .and_then(|n| u32::try_from(n).ok()),
                },
            ))
        })
        .collect()
}

impl Provider {
    pub fn model_limits(&self, model: &str) -> ModelLimits {
        let identity = self.connection().identity();
        let cache = self.limits.lock().unwrap();
        if cache.connection == identity {
            cache.models.get(model).cloned().unwrap_or_default()
        } else {
            ModelLimits::default()
        }
    }
    pub fn limits_status(&self, settings: &ProviderSettings) -> Value {
        let limits = self.model_limits(&settings.model);
        let context = limits.context(settings.context_window_tokens);
        let output = limits
            .output(settings.max_output_tokens)
            .min(context.saturating_sub(1024).min(u32::MAX as usize) as u32);
        json!({"model":settings.model,"advertised":limits,
            "effectiveContextWindow":context,
            "effectiveOutputCeiling":output,
            "source":if limits.context_window.is_some() || limits.max_output_tokens.is_some(){"upstream_models"}else{"configured"}})
    }
    pub async fn discover_model_limits(&self, model: &str) -> ModelLimits {
        let identity = self.connection().identity();
        let needed = {
            let cache = self.limits.lock().unwrap();
            cache.connection != identity || !cache.attempted
        };
        if needed {
            // A missing/older /models endpoint is optional capability discovery;
            // never invent limits or block generation on its HTTP error.
            if self.models().await.is_err() {
                let mut cache = self.limits.lock().unwrap();
                cache.connection = identity;
                cache.attempted = true;
                cache.models.clear();
            }
        }
        self.model_limits(model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upstream_limits_clip_defaults_and_preserve_available_output_headroom() {
        let settings = ProviderSettings::default();
        assert_eq!(settings.context_window_tokens, 1_048_576);
        assert_eq!(settings.max_output_tokens, 393_216);
        assert_eq!(
            ModelLimits::default()
                .output_allowance(&settings, 5000, 8_000_000)
                .unwrap(),
            393_216
        );
        let limits = parse_models(
            &json!({"data":[{"id":"small","max_model_len":32768,"max_output_tokens":8192}]}),
        );
        assert_eq!(
            limits["small"]
                .output_allowance(&settings, 5000, 8_000_000)
                .unwrap(),
            8192
        );
        assert_eq!(
            limits["small"]
                .output_allowance(&settings, 30000, 8_000_000)
                .unwrap(),
            1744
        );
        assert!(limits["small"]
            .output_allowance(&settings, 32700, 8_000_000)
            .is_err());
        assert_eq!(
            ModelLimits::default()
                .output_allowance(&settings, 900000, 8_000_000)
                .unwrap(),
            147552
        );
        assert_eq!(
            ModelLimits::default()
                .output_allowance(&settings, 5000, 1000)
                .unwrap(),
            1000
        );
    }
}
