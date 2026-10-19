const SYSTEM_ZH_CN: &str = include_str!("system.zh-CN.md");
const SYSTEM_EN: &str = include_str!("system.en.md");

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
            json!({"maxSteps":12,"maxRunTokens":120000,"maxRunSeconds":600,"maxOutputTokens":8192,"contextWindowTokens":65536,"compactTokens":40000,"goalMode":false,"goalMinReviewRounds":1,"goalMaxContinuations":3})
        }
        "standard" => {
            json!({"maxSteps":24,"maxRunTokens":250000,"maxRunSeconds":900,"maxOutputTokens":16384,"contextWindowTokens":131072,"compactTokens":60000,"goalMode":false,"goalMinReviewRounds":1,"goalMaxContinuations":6})
        }
        "detailed" => {
            json!({"maxSteps":128,"maxRunTokens":2000000,"maxRunSeconds":7200,"maxOutputTokens":32768,"contextWindowTokens":262144,"compactTokens":180000,"goalMode":true,"goalMinReviewRounds":3,"goalMaxContinuations":12})
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
