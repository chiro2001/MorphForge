const SYSTEM_ZH_CN: &str = include_str!("system.zh-CN.md");
const SYSTEM_EN: &str = include_str!("system.en.md");

pub fn system_prompt(language: &str) -> &'static str {
    match language {
        "en" => SYSTEM_EN,
        _ => SYSTEM_ZH_CN,
    }
}
