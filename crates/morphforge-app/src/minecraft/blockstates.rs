//! Deterministic cube-model selection from a preserved Java resource pack.
use super::formats::BlockState;
use anyhow::{bail, Result};
use serde_json::Value;
use std::{collections::BTreeMap, path::Path};
pub type Mapping = BTreeMap<String, (String, u8)>;
fn properties(state: &BlockState, key: &str, value: &str) -> bool {
    state
        .properties
        .get(key)
        .is_some_and(|v| value.split('|').any(|s| s == v))
}
fn condition(state: &BlockState, value: &Value, depth: usize) -> bool {
    if depth > 16 {
        return false;
    }
    let Some(object) = value.as_object() else {
        return false;
    };
    object.iter().all(|(key, value)| match key.as_str() {
        "OR" => value
            .as_array()
            .is_some_and(|a| a.iter().any(|v| condition(state, v, depth + 1))),
        "AND" => value
            .as_array()
            .is_some_and(|a| a.iter().all(|v| condition(state, v, depth + 1))),
        _ => value.as_str().is_some_and(|v| properties(state, key, v)),
    })
}
fn select(value: &Value) -> Option<&Value> {
    if let Some(array) = value.as_array() {
        array
            .iter()
            .enumerate()
            .max_by_key(|(i, v)| (v["weight"].as_u64().unwrap_or(1), usize::MAX - *i))
            .map(|(_, v)| v)
    } else {
        Some(value)
    }
}
fn model(value: &Value) -> Option<(String, u8)> {
    let value = select(value)?;
    let name = value["model"].as_str()?;
    let rotation = |k| -> Option<i64> {
        match value.get(k) {
            None => Some(0),
            Some(v) => {
                let n = v.as_i64()?;
                if n % 90 != 0 || !(-360..=360).contains(&n) {
                    return None;
                }
                Some((n / 90).rem_euclid(4))
            }
        }
    };
    let x = rotation("x")?;
    let y = rotation("y")? as u8;
    // UV-lock needs an independent per-face UV frame; conservatively report it.
    if value["uvlock"] == true && (x != 0 || y != 0) {
        return None;
    }
    let code = match x {
        0 => y,
        1 => 8 + y,
        2 => 16 + (y + 2) % 4,
        _ => 12 + y,
    };
    let name = if name.contains(':') {
        name.to_owned()
    } else {
        format!("minecraft:{name}")
    };
    Some((name, code))
}
pub fn resolve(root: &Path, pack: &str, states: &[BlockState]) -> Result<(Mapping, Vec<String>)> {
    if pack == "builtin" {
        return Ok((Mapping::new(), vec![]));
    }
    if pack.len() != 64 || !pack.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("无效资源包ID");
    }
    let path = root.join("packs").join(format!("{pack}-blockstates.json"));
    if !path.is_file() {
        return Ok((
            Mapping::new(),
            vec![
                "所选旧资源包没有blockstate索引，按名称近似；重新导入ZIP可启用状态模型映射。"
                    .into(),
            ],
        ));
    }
    if std::fs::metadata(&path)?.len() > 32 * 1024 * 1024 {
        bail!("资源包方块状态索引超限");
    }
    let bytes = std::fs::read(&path)?;
    let report: Value = serde_json::from_slice(&std::fs::read(
        root.join("packs").join(format!("{pack}.json")),
    )?)?;
    if report["blockstatesHash"] != crate::store::hash(&bytes) {
        bail!("MC_PACK_CORRUPT：方块状态索引损坏，请重新导入资源包");
    }
    let index: BTreeMap<String, Value> = serde_json::from_slice(&bytes)?;
    let mut out = Mapping::new();
    let mut warnings = vec![];
    for state in states {
        let Some(definition) = index.get(&state.name) else {
            continue;
        };
        let mut selected = vec![];
        if let Some(variants) = definition["variants"].as_object() {
            if let Some((_, value)) = variants
                .iter()
                .filter(|(key, _)| {
                    key.is_empty()
                        || key.split(',').all(|p| {
                            p.split_once('=')
                                .is_some_and(|(k, v)| properties(state, k, v))
                        })
                })
                .max_by_key(|(key, _)| key.len())
            {
                if value.is_array() && warnings.len() < 128 {
                    warnings.push(format!(
                        "{}：加权模型采用固定最高权重项，便于可重复导入。",
                        state.canonical()
                    ));
                }
                selected.push(value);
            }
        } else if let Some(parts) = definition["multipart"].as_array() {
            for part in parts {
                if part.get("when").is_none_or(|v| condition(state, v, 0)) {
                    selected.push(&part["apply"]);
                }
            }
        }
        let mapped = if selected.len() == 1 {
            model(selected[0])
        } else {
            None
        };
        if let Some(value) = mapped {
            out.insert(state.canonical(), value);
        } else if warnings.len() < 128 {
            warnings.push(format!(
                "{}：多层multipart、UV锁定或非直角旋转暂未精确支持，使用名称匹配材质；源包保留。",
                state.canonical()
            ));
        }
    }
    Ok((out, warnings))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conditions_and_cube_rotation() {
        let state = BlockState::parse("minecraft:test[facing=north,lit=true]").unwrap();
        assert!(condition(
            &state,
            &serde_json::json!({"OR":[{"lit":"false"},{"facing":"north|south"}]}),
            0
        ));
        assert_eq!(
            model(&serde_json::json!({"model":"example:block/on","x":90,"y":90})),
            Some(("example:block/on".into(), 9))
        );
        assert_eq!(
            model(&serde_json::json!({"model":"block/on","x":90,"uvlock":true})),
            None
        );
    }
}
