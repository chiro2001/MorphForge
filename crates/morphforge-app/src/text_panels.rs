//! Text panel authoring and shared binary font assets. No OS font dependency.
use crate::store::Workspace;
use anyhow::{bail, Context, Result};
use morphforge_core::{
    text_panel::{self, TextPanel},
    Operation, Scene,
};
use serde_json::{json, Value};
pub const ZH_WIDTHS: &[u8] = include_bytes!("../../../fonts/pixel16/zh.widths.bin");
pub const JP_WIDTHS: &[u8] = include_bytes!("../../../fonts/pixel16/jp.widths.bin");
const ZH: &[u8] = include_bytes!("../../../fonts/pixel16/zh.bin");
const JP: &[u8] = include_bytes!("../../../fonts/pixel16/jp.bin");
pub fn asset(font: &str, kind: &str) -> Option<&'static [u8]> {
    match (font, kind) {
        ("pixel16-zh", "bitmap") => Some(ZH),
        ("pixel16-jp", "bitmap") => Some(JP),
        ("pixel16-zh", "widths") => Some(ZH_WIDTHS),
        ("pixel16-jp", "widths") => Some(JP_WIDTHS),
        _ => None,
    }
}
pub fn widths(font: &str) -> &'static [u8] {
    if font == "pixel16-jp" {
        JP_WIDTHS
    } else {
        ZH_WIDTHS
    }
}
pub fn layout(panel: &TextPanel) -> Result<Value> {
    Ok(serde_json::to_value(text_panel::layout(
        panel,
        widths(&panel.font),
    )?)?)
}
pub fn layouts(scene: &Scene) -> Result<Value> {
    scene
        .text_panels
        .iter()
        .map(|p| Ok(json!({"panel":p,"layout":layout(p)?})))
        .collect::<Result<Vec<_>>>()
        .map(Value::Array)
}
pub fn target_report(scene: &Scene) -> Value {
    let counts = scene
        .text_panels
        .iter()
        .map(|p| crate::text_wire::characters(p).ok().map(|v| v.0.len()))
        .collect::<Vec<_>>();
    let glyphs = scene
        .text_panels
        .iter()
        .map(|p| {
            layout(p)
                .ok()
                .and_then(|v| v["glyphs"].as_array().map(Vec::len))
        })
        .collect::<Vec<_>>();
    let errors = scene
        .text_panels
        .iter()
        .enumerate()
        .filter_map(|(slot, p)| {
            crate::text_wire::encode(p, slot as u8)
                .err()
                .map(|e| json!({"id":p.id,"error":e.to_string()}))
        })
        .collect::<Vec<_>>();
    json!({"panelCount":scene.text_panels.len(),"panelCapacity":crate::text_wire::SLOTS,"characterSlotsPerPanel":128,"characterSlotCounts":counts,"visibleGlyphCounts":glyphs,"fits":scene.text_panels.len()<=crate::text_wire::SLOTS&&errors.is_empty(),"errors":errors,"fontIds":["pixel16-zh","pixel16-jp"]})
}
pub fn apply(w: &mut Workspace, args: &Value, run: Option<&str>) -> Result<Value> {
    let action = args["action"].as_str().unwrap_or("list");
    if action == "list" {
        return Ok(
            json!({"revision":w.project.scene.revision,"panels":layouts(&w.project.scene)?,"fonts":[{"id":"pixel16-zh","name":"点阵16 · 中文"},{"id":"pixel16-jp","name":"点阵16 · 日文"}]}),
        );
    }
    let id = args["id"].as_str().unwrap_or("");
    let existing = w
        .project
        .scene
        .text_panels
        .iter()
        .find(|p| p.id == id)
        .cloned();
    if action == "inspect" {
        let p = existing.context("文本框不存在")?;
        return Ok(json!({"revision":w.project.scene.revision,"panel":p,"layout":layout(&p)?}));
    }
    let base = args["baseRevision"]
        .as_u64()
        .context("文本框修改需要baseRevision")?;
    if base != w.project.scene.revision {
        bail!("REVISION_CONFLICT：文本框版本已变化")
    }
    let (ops, panel) = match action {
        "remove" => {
            existing.context("文本框不存在")?;
            (
                vec![Operation::RemoveTextPanels {
                    ids: vec![id.into()],
                }],
                None,
            )
        }
        "create" | "update" | "append" => {
            let requested_kind = args["preset"]
                .as_str()
                .or_else(|| args["panel"]["kind"].as_str());
            let inferred_caption = requested_kind == Some("caption")
                || requested_kind.is_none()
                    && args["panel"]["name"].as_str().is_some_and(|name| {
                        name.contains("字幕") || name.to_ascii_lowercase().contains("caption")
                    });
            let mut panel = if action == "create" {
                TextPanel {
                    id: if id.is_empty() {
                        format!("text-{}", uuid::Uuid::new_v4())
                    } else {
                        id.into()
                    },
                    ..if inferred_caption {
                        TextPanel::caption()
                    } else {
                        TextPanel::default()
                    }
                }
            } else {
                existing.clone().context("文本框不存在")?
            };
            if let Some(kind) = requested_kind {
                if !matches!(kind, "text" | "caption") {
                    bail!("文本预设须为text/caption");
                }
                if action != "create" && kind != panel.kind {
                    panel.kind = kind.into();
                    if kind == "caption" {
                        panel.max_lines = 4;
                        panel.size[1] = panel.size[1].max(40.);
                        panel.overflow = "tail".into();
                        panel.vertical_align = "bottom".into();
                    }
                }
            }
            if action == "create" && existing.is_some() {
                bail!("文本框id已存在，请使用update")
            }
            if action == "append" {
                panel
                    .text
                    .push_str(args["text"].as_str().context("append需要text")?)
            } else {
                let mut value = serde_json::to_value(&panel)?;
                if let Some(patch) = args.get("panel") {
                    for (k, v) in patch.as_object().context("panel需要对象")? {
                        if k == "id" {
                            bail!("id请放在顶层，不能在panel内改写")
                        };
                        value[k] = v.clone();
                    }
                }
                panel = serde_json::from_value(value)?;
            }
            if requested_kind.is_some_and(|kind| panel.kind != kind) {
                bail!("preset与panel.kind不一致，请使用同一个控件类型");
            }
            panel.validate()?;
            (
                vec![Operation::TextPanel {
                    value: panel.clone(),
                }],
                Some(panel),
            )
        }
        _ => bail!("文本框action须为list/inspect/create/update/append/remove"),
    };
    let panel_layout = panel.as_ref().map(layout).transpose()?;
    if args["dryRun"] == true {
        return Ok(
            json!({"ok":true,"dryRun":true,"revision":base,"panel":panel,"layout":panel_layout}),
        );
    }
    let result = w.apply(&ops, Some(base), args["requestId"].as_str(), run)?;
    Ok(
        json!({"ok":true,"revision":result["revision"],"panel":panel,"layout":panel_layout,"stats":result["stats"],"note":"贴图文字对象；不是体素字，发布需要目标Avatar支持text-panels能力。"}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_and_manual_caption_creation_share_a_fixed_size_preset() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = Workspace::open(dir.path().into()).unwrap();
        let manual = apply(
            &mut w,
            &json!({"action":"create","id":"manual","preset":"caption","baseRevision":0}),
            None,
        )
        .unwrap();
        let revision = w.project.scene.revision;
        let model=apply(&mut w,&json!({"action":"create","id":"model","baseRevision":revision,"panel":{"name":"直播实时字幕","position":[12,40,6]}}),None).unwrap();
        for value in [&manual, &model] {
            assert_eq!(value["panel"]["kind"], "caption");
            assert_eq!(value["panel"]["maxLines"], 4);
            assert_eq!(value["panel"]["fontSize"], 8.);
            assert_eq!(value["panel"]["overflow"], "tail");
        }
        assert_eq!(model["panel"]["position"], json!([12., 40., 6.]));
    }
    #[test]
    fn create_edit_undo_and_restart_preserve_text() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = Workspace::open(dir.path().into()).unwrap();
        let a=apply(&mut w,&json!({"action":"create","id":"caption","baseRevision":0,"panel":{"text":"A中こんにちは","font":"pixel16-jp"}}),None).unwrap();
        assert!(a["layout"]["missing"].as_array().unwrap().is_empty());
        let r = w.project.scene.revision;
        apply(
            &mut w,
            &json!({"action":"append","id":"caption","baseRevision":r,"text":"！"}),
            None,
        )
        .unwrap();
        assert!(w.project.scene.text_panels[0].text.ends_with('！'));
        w.undo(false, None).unwrap();
        assert_eq!(w.project.scene.text_panels[0].text, "A中こんにちは");
        drop(w);
        let w = Workspace::open(dir.path().into()).unwrap();
        assert_eq!(w.project.scene.text_panels[0].id, "caption");
    }
    #[test]
    fn normalized_japanese_and_unsupported_characters_are_reported() {
        let p = TextPanel {
            text: "か\u{3099}A🦊".into(),
            font: "pixel16-jp".into(),
            ..Default::default()
        };
        let l = layout(&p).unwrap();
        assert_eq!(l["glyphs"][0]["glyph"], json!('が' as u32));
        assert_eq!(l["missing"], json!(["🦊"]));
        assert!(asset("pixel16-zh", "bitmap").unwrap().len() == 2097152);
    }
}

/// Reserve independent line/slot budgets so a long translation cannot evict the source.
pub fn bilingual_caption(panel: &TextPanel, source: &str, translated: &str) -> Result<String> {
    let rows = caption_rows(panel);
    if rows < 2 {
        bail!("双语字幕至少需要两行高度，请增高字幕框；字号保持不变")
    }
    fn tail(panel: &TextPanel, text: &str, rows: usize, slots: usize) -> Result<String> {
        let mut part = panel.clone();
        part.kind = "caption".into();
        part.max_lines = rows;
        part.overflow = "tail".into();
        part.text = text.into();
        let (mut tokens, _) = crate::text_wire::characters(&part)?;
        if tokens.len() > slots {
            tokens.drain(..tokens.len() - slots);
        }
        while tokens.first() == Some(&10) {
            tokens.remove(0);
        }
        Ok(String::from_utf16_lossy(&tokens))
    }
    let original = tail(panel, source, rows.div_ceil(2), 63)?;
    let translation = tail(panel, translated, rows / 2, 64)?;
    Ok(if original.is_empty() {
        translation
    } else if translation.is_empty() {
        original
    } else {
        format!("{original}\n{translation}")
    })
}
pub fn caption_rows(panel: &TextPanel) -> usize {
    (((panel.size[1] + panel.line_gap) / (panel.font_size + panel.line_gap))
        .floor()
        .max(0.) as usize)
        .min(if panel.max_lines > 0 {
            panel.max_lines
        } else {
            128
        })
}

#[cfg(test)]
mod bilingual_tests {
    use super::*;
    #[test]
    fn bilingual_windows_keep_both_languages_at_fixed_size_and_fit_wire() {
        for width in [32., 128., 256.] {
            let panel = TextPanel {
                size: [width, 40.],
                ..TextPanel::caption()
            };
            let text = bilingual_caption(
                &panel,
                &"Original ABC ".repeat(150),
                &"日本語の翻訳です。".repeat(150),
            )
            .unwrap();
            let composed = TextPanel { text, ..panel };
            let l = text_panel::layout(&composed, widths(&composed.font)).unwrap();
            assert_eq!(l.font_size, 8.);
            assert!((2..=4).contains(&l.visible_lines));
            if width <= 128. {
                assert_eq!(l.visible_lines, 4);
            }
            assert!(l.glyphs.iter().any(|g| g.line < 2 && g.glyph < 128));
            assert!(l.glyphs.iter().any(|g| g.line >= 2 && g.glyph > 128));
            assert!(crate::text_wire::characters(&composed).unwrap().0.len() <= 128);
        }
    }
    #[test]
    fn custom_three_line_panels_still_keep_source_and_translation() {
        let panel = TextPanel {
            max_lines: 3,
            size: [80., 32.],
            ..TextPanel::caption()
        };
        let text = bilingual_caption(&panel, "原文一行\n原文二行\n原文三行", "翻訳一行\n翻訳二行")
            .unwrap();
        assert!(text.contains("原文二行"));
        assert!(text.contains("原文三行"));
        assert!(text.contains("翻訳二行"));
        assert!(!text.contains("原文一行"));
        assert!(bilingual_caption(
            &TextPanel {
                size: [128., 8.],
                max_lines: 1,
                ..panel
            },
            "原文",
            "translation"
        )
        .unwrap_err()
        .to_string()
        .contains("两行"));
    }
}
