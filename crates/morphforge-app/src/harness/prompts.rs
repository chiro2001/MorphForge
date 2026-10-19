const SYSTEM_ZH_CN: &str = include_str!("system.zh-CN.md");
const SYSTEM_EN: &str = include_str!("system.en.md");

pub fn saved_settings(bytes: &[u8]) -> anyhow::Result<(crate::provider::ProviderSettings, bool)> {
    use serde_json::{json, Value};
    let mut value: Value = serde_json::from_slice(bytes)?;
    if value["budgetDefaultsVersion"].as_u64().unwrap_or_default() >= 1 {
        return Ok((serde_json::from_value(value)?, false));
    }
    let preset = value["preset"].as_str().unwrap_or("standard");
    let legacy = match preset {
        "sketch" => {
            json!({"maxRunTokens":120000,"maxRunSeconds":600,"maxOutputTokens":8192,"contextWindowTokens":65536,"compactTokens":40000})
        }
        "detailed" => {
            json!({"maxRunTokens":2000000,"maxRunSeconds":7200,"maxOutputTokens":32768,"contextWindowTokens":262144,"compactTokens":180000})
        }
        _ => {
            json!({"maxRunTokens":250000,"maxRunSeconds":900,"maxOutputTokens":16384,"contextWindowTokens":131072,"compactTokens":60000})
        }
    };
    let updated = preset_patch(preset).unwrap_or_else(|| preset_patch("standard").unwrap());
    for (key, old) in legacy.as_object().unwrap() {
        if value.get(key) == Some(old) {
            value[key] = updated[key].clone();
        }
    }
    value["budgetDefaultsVersion"] = json!(1);
    Ok((serde_json::from_value(value)?, true))
}

pub fn system_prompt(language: &str) -> &'static str {
    match language {
        "en" => SYSTEM_EN,
        _ => SYSTEM_ZH_CN,
    }
}

pub fn preset_patch(name: &str) -> Option<serde_json::Value> {
    use serde_json::json;
    Some(match name {
        "sketch" => {
            json!({"maxSteps":12,"maxRunTokens":2000000,"maxRunSeconds":3600,"maxOutputTokens":393216,"contextWindowTokens":1048576,"compactTokens":900000,"goalMode":false,"goalMinReviewRounds":1,"goalMaxContinuations":3})
        }
        "standard" => {
            json!({"maxSteps":24,"maxRunTokens":8000000,"maxRunSeconds":7200,"maxOutputTokens":393216,"contextWindowTokens":1048576,"compactTokens":900000,"goalMode":false,"goalMinReviewRounds":1,"goalMaxContinuations":6})
        }
        "detailed" => {
            json!({"maxSteps":128,"maxRunTokens":32000000,"maxRunSeconds":21600,"maxOutputTokens":393216,"contextWindowTokens":1048576,"compactTokens":900000,"goalMode":true,"goalMinReviewRounds":3,"goalMaxContinuations":12})
        }
        _ => return None,
    })
}

pub fn preset_prompt(preset: &str, language: &str) -> &'static str {
    match (preset, language) {
        ("sketch", "en") => include_str!("presets/sketch.en.md"),
        ("sketch", _) => include_str!("presets/sketch.zh-CN.md"),
        ("detailed", "en") => include_str!("presets/detailed.en.md"),
        ("detailed", _) => include_str!("presets/detailed.zh-CN.md"),
        (_, "en") => include_str!("presets/standard.en.md"),
        _ => include_str!("presets/standard.zh-CN.md"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_defaults_upgrade_without_overriding_custom_fields() {
        let (settings,changed)=saved_settings(br#"{"preset":"standard","contextWindowTokens":131072,"maxOutputTokens":16384,"maxRunTokens":1234567,"compactTokens":99000}"#).unwrap();
        assert!(changed);
        assert_eq!(settings.context_window_tokens, 1048576);
        assert_eq!(settings.max_output_tokens, 393216);
        assert_eq!(settings.max_run_tokens, 1234567);
        assert_eq!(settings.compact_tokens, 99000);
        let (_, again) = saved_settings(&serde_json::to_vec(&settings).unwrap()).unwrap();
        assert!(!again);
    }
    #[test]
    fn explicitly_reduced_budgets_are_not_upgraded_again_on_restart() {
        let settings = crate::provider::ProviderSettings {
            max_run_tokens: 250000,
            max_run_seconds: 900,
            max_output_tokens: 16384,
            ..Default::default()
        };
        let (restored, changed) = saved_settings(&serde_json::to_vec(&settings).unwrap()).unwrap();
        assert!(!changed);
        assert_eq!(restored.max_run_tokens, 250000);
        assert_eq!(restored.max_run_seconds, 900);
        assert_eq!(restored.max_output_tokens, 16384);
    }
}
