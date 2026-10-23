//! Bounded MF6 text extension. External layout inserts explicit line breaks;
//! shaders only accumulate half/full-width advances and alignment.
use anyhow::{bail, Result};
use morphforge_core::{
    protocol,
    text_panel::{self, TextPanel},
    Scene,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
pub const CAPABILITY: &str = "mf6-text-panels/1";
pub const SLOTS: usize = 4;
pub const CHARACTERS: usize = 128;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message(pub [u8; 29]);
impl Message {
    pub fn packet(&self, seq: u8) -> Vec<u8> {
        let mut values = self
            .0
            .iter()
            .enumerate()
            .map(|(i, b)| (format!("MF6_D{i:02}"), protocol::OscValue::Int(*b as i32)))
            .collect::<Vec<_>>();
        values.push((
            "MF6_Ctl".into(),
            protocol::OscValue::Int((seq % 16 * 16) as i32),
        ));
        protocol::bundle(&values)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub slot: u8,
    pub epoch: u8,
    pub data: [u8; 29],
}
impl Snapshot {
    pub fn packet(&self) -> Vec<u8> {
        let mut values = self
            .data
            .iter()
            .enumerate()
            .map(|(i, b)| (format!("MF6_D{i:02}"), protocol::OscValue::Int(*b as i32)))
            .collect::<Vec<_>>();
        values.push((
            "MF6_Ctl".into(),
            protocol::OscValue::Int((((self.epoch & 3) * 4 + self.slot) * 16) as i32),
        ));
        protocol::bundle(&values)
    }
}
fn command(op: u8, slot: u8) -> Message {
    let mut d = [0; 29];
    d[0] = op;
    d[1] = slot;
    d[28] = 254;
    Message(d)
}
fn word(out: &mut [u8], index: usize, value: u16) {
    out[index..index + 2].copy_from_slice(&value.to_le_bytes());
}
fn fixed(value: f64, scale: f64) -> Result<u16> {
    let v = (value * scale).round();
    if !v.is_finite() || !(0.0..=65535.0).contains(&v) {
        bail!("TEXT_WIRE_RANGE：文字配置超出固定小数范围")
    };
    Ok(v as u16)
}
pub fn characters(panel: &TextPanel) -> Result<(Vec<u16>, f64)> {
    let layout = text_panel::layout(panel, crate::text_panels::widths(&panel.font))?;
    let mut tokens = Vec::new();
    let mut line = 0;
    for glyph in layout.glyphs {
        while line < glyph.line {
            tokens.push(10);
            line += 1;
        }
        tokens.push(glyph.glyph);
    }
    // Trailing empty visible lines affect vertical alignment, including empty
    // lines that contain no glyph and therefore were absent from glyphs above.
    while line + 1 < layout.visible_lines {
        tokens.push(10);
        line += 1;
    }
    if tokens.len() > CHARACTERS {
        bail!(
            "TEXT_WIRE_CAPACITY：每个文本框最多128字符槽（包括显式换行），当前{}",
            tokens.len()
        )
    }
    Ok((tokens, layout.font_size))
}
pub fn encode(panel: &TextPanel, slot: u8) -> Result<Vec<Message>> {
    panel.validate()?;
    if slot as usize >= SLOTS {
        bail!("TEXT_WIRE_CAPACITY：最多4文本框")
    }
    let (glyphs, size) = characters(panel)?;
    if panel.size.iter().any(|v| *v > 256.) {
        bail!("TEXT_WIRE_RANGE：Avatar文本框宽高最多256格")
    }
    let mut style = command(3, slot);
    style.0[2..6].copy_from_slice(&panel.background);
    style.0[6] = u8::from(panel.font == "pixel16-jp");
    style.0[7] = match panel.align.as_str() {
        "center" => 1,
        "right" => 2,
        _ => 0,
    };
    style.0[8] = match panel.vertical_align.as_str() {
        "middle" => 1,
        "bottom" => 2,
        _ => 0,
    };
    word(&mut style.0, 9, fixed(panel.letter_spacing, 64.)?);
    word(&mut style.0, 11, fixed(panel.line_gap, 64.)?);
    let mut messages = vec![style];
    for (page, chars) in glyphs.chunks(12).enumerate() {
        let mut m = command(2, slot);
        m.0[2] = page as u8;
        for (i, g) in chars.iter().enumerate() {
            word(&mut m.0, 3 + i * 2, *g);
        }
        messages.push(m);
    }
    let mut config = command(1, slot);
    for a in 0..3 {
        word(
            &mut config.0,
            2 + a * 2,
            fixed(panel.position[a] + 512., 64.)?,
        );
        word(
            &mut config.0,
            8 + a * 2,
            ((panel.rotation[a].rem_euclid(360.) / 360. * 65536.).round() as u32 % 65536) as u16,
        );
    }
    word(&mut config.0, 14, fixed(panel.size[0], 64.)?);
    word(&mut config.0, 16, fixed(panel.size[1], 64.)?);
    word(&mut config.0, 18, fixed(size, 64.)?);
    config.0[20..23].copy_from_slice(&panel.color);
    config.0[23] = (panel.opacity * 255.).round() as u8;
    config.0[24] = glyphs.len() as u8;
    messages.push(config);
    Ok(messages)
}
#[derive(Clone, Default)]
pub struct Session {
    pub motion_groups: BTreeMap<String, u8>,
    target: BTreeMap<String, (u8, Vec<Message>)>,
    panels: BTreeMap<String, TextPanel>,
    live_text: BTreeMap<String, String>,
    live_pages: BTreeMap<u8, VecDeque<Message>>,
    configured_fonts: BTreeMap<u8, u16>,
    started_live: BTreeSet<u8>,
    deferred_live: BTreeMap<u8, (String, String)>,
    epoch: [u8; SLOTS],
    tombstones: BTreeSet<u8>,
    sent: BTreeMap<u8, Vec<Message>>,
    pending: VecDeque<Message>,
    latest: BTreeMap<u8, Snapshot>,
    last_snapshot: BTreeMap<u8, Snapshot>,
}
impl Session {
    pub fn set_target(&mut self, scene: &Scene) -> Result<()> {
        if scene.text_panels.len() > SLOTS {
            bail!("TEXT_WIRE_CAPACITY：最多4文本框")
        }
        // Build and validate before mutating any queued/sent state.
        let mut available = (0..SLOTS as u8).collect::<Vec<_>>();
        for p in &scene.text_panels {
            if let Some((slot, _)) = self.target.get(&p.id) {
                available.retain(|s| s != slot);
            }
        }
        let mut next = BTreeMap::new();
        for p in &scene.text_panels {
            let slot = self
                .target
                .get(&p.id)
                .map(|v| v.0)
                .unwrap_or_else(|| available.remove(0));
            let mut messages = encode(p, slot)?;
            for m in &mut messages {
                if m.0[0] == 3 {
                    m.0[13] = self.motion_groups.get(&p.group).copied().unwrap_or(0);
                }
            }
            next.insert(p.id.clone(), (slot, messages));
        }
        let mut epochs = self.epoch;
        let mut tombstones = self.tombstones.clone();
        for (_, (slot, _)) in &self.target {
            if !next.values().any(|(s, _)| s == slot) {
                epochs[*slot as usize] = (epochs[*slot as usize] + 1) % 4;
                tombstones.insert(*slot);
            }
        }
        for (id, (slot, messages)) in &mut next {
            if let Some((old_id, (_, old_messages))) =
                self.target.iter().find(|(_, (s, _))| s == slot)
            {
                let mut previous = old_messages.clone();
                for m in &mut previous {
                    if m.0[0] == 1 {
                        m.0[25] = 0;
                    }
                    if m.0[0] == 3 {
                        m.0[14] = 0;
                    }
                }
                if old_id != id || previous != *messages {
                    epochs[*slot as usize] = (epochs[*slot as usize] + 1) % 4;
                }
            }
            tombstones.remove(slot);
            for m in messages {
                if m.0[0] == 1 {
                    m.0[25] = epochs[*slot as usize];
                }
                if m.0[0] == 3 {
                    m.0[14] = epochs[*slot as usize];
                }
            }
        }
        let next_panels = scene
            .text_panels
            .iter()
            .map(|p| (p.id.clone(), p.clone()))
            .collect::<BTreeMap<_, _>>();
        if self.target == next && self.panels == next_panels {
            return Ok(());
        }
        // A geometry update or another panel edit must not discard a live
        // caption. A newly authored text for that same panel supersedes it.
        self.live_text.retain(|id, _| {
            next_panels
                .get(id)
                .zip(self.panels.get(id))
                .is_some_and(|(a, b)| a.text == b.text)
        });
        self.latest.clear();
        self.last_snapshot.clear();
        self.live_pages.clear();
        self.started_live.clear();
        self.deferred_live.clear();
        self.epoch = epochs;
        self.tombstones = tombstones;
        self.target = next;
        self.panels = next_panels;
        self.rebuild_queue(false);
        self.restore_live()?;
        Ok(())
    }
    fn rebuild_queue(&mut self, force: bool) {
        self.pending.clear();
        for slot in &self.tombstones {
            let mut message = command(4, *slot);
            message.0[2] = self.epoch[*slot as usize];
            self.pending.push_back(message);
        }
        for (slot, messages) in self.target.values() {
            if force || self.sent.get(slot) != Some(messages) {
                self.pending.extend(messages.iter().cloned());
            }
        }
    }
    pub fn replay(&mut self) {
        self.rebuild_queue(true);
        self.last_snapshot.clear();
        self.latest.clear();
        self.live_pages.clear();
        self.started_live.clear();
        self.deferred_live.clear();
        // Entries were validated when queued. Retained panel configurations
        // are revalidated by set_target before this replay can run.
        self.restore_live().expect("validated live text replay");
    }
    pub fn reset(&mut self) {
        self.sent.clear();
        self.configured_fonts.clear();
        self.last_snapshot.clear();
        self.replay();
    }
    pub fn latest_text(&mut self, id: &str, text: &str) -> Result<()> {
        if self.live_text.get(id).is_some_and(|old| old == text) {
            return Ok(());
        }
        self.queue_live(id, text)?;
        self.live_text.insert(id.into(), text.into());
        Ok(())
    }
    fn restore_live(&mut self) -> Result<()> {
        for (id, text) in self.live_text.clone() {
            self.queue_live(&id, &text)?;
        }
        Ok(())
    }
    fn queue_live(&mut self, id: &str, text: &str) -> Result<()> {
        let slot = self
            .target
            .get(id)
            .map(|v| v.0)
            .ok_or_else(|| anyhow::anyhow!("请先发布选定文本框，再启用实时字幕"))?;
        let mut panel = self
            .panels
            .get(id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("文本框配置不存在"))?;
        panel.text = text.into();
        let (chars, font_size) = characters(&panel)?;
        let (_, base_font_size) = characters(&self.panels[id])?;
        let configured = self
            .configured_fonts
            .get(&slot)
            .copied()
            .unwrap_or(fixed(base_font_size, 64.)?);
        if self.started_live.contains(&slot) {
            self.deferred_live.insert(slot, (id.into(), text.into()));
            return Ok(());
        }
        if chars.len() > 14 || fixed(font_size, 64.)? != configured {
            let mut messages = encode(&panel, slot)?;
            for m in &mut messages {
                if m.0[0] == 1 {
                    m.0[25] = self.epoch[slot as usize];
                }
                if m.0[0] == 3 {
                    m.0[14] = self.epoch[slot as usize];
                    m.0[13] = self.motion_groups.get(&panel.group).copied().unwrap_or(0);
                }
            }
            self.latest.remove(&slot);
            self.last_snapshot.remove(&slot);
            // Replace unsent partials for this slot only, keeping other
            // captions and the shared geometry sender intact.
            self.live_pages.insert(slot, messages.into());
            return Ok(());
        }
        self.live_pages.remove(&slot);
        let mut data = [0; 29];
        for (i, c) in chars.iter().enumerate() {
            data[i * 2..i * 2 + 2].copy_from_slice(&c.to_le_bytes());
        }
        data[28] = 128 + chars.len() as u8;
        let snapshot = Snapshot {
            slot,
            epoch: self.epoch[slot as usize],
            data,
        };
        if self.last_snapshot.get(&slot) == Some(&snapshot) {
            self.latest.remove(&slot);
        } else {
            self.latest.insert(slot, snapshot);
        }
        Ok(())
    }
    pub fn next_latest(&self) -> Option<Snapshot> {
        self.latest
            .values()
            .find(|snapshot| {
                !self.pending.iter().any(|m| m.0[1] == snapshot.slot)
                    && !self.live_pages.contains_key(&snapshot.slot)
            })
            .cloned()
    }
    pub fn commit_latest(&mut self, m: &Snapshot) {
        if self.latest.get(&m.slot) == Some(m) {
            self.latest.remove(&m.slot);
            self.last_snapshot.insert(m.slot, m.clone());
        }
    }
    pub fn pending_commands(&self) -> usize {
        self.pending.len()
            + self.live_pages.values().map(VecDeque::len).sum::<usize>()
            + self.deferred_live.len()
    }
    pub fn pending_saved(&self) -> usize {
        self.pending.len()
    }
    pub fn pending_live(&self) -> usize {
        self.latest.len()
    }
    pub fn pending(&self) -> usize {
        self.pending_commands() + self.latest.len()
    }
    pub fn next(&self) -> Option<Message> {
        self.pending
            .front()
            .cloned()
            .or_else(|| self.live_pages.values().find_map(|q| q.front().cloned()))
    }
    pub fn commit(&mut self, message: &Message) {
        if self.pending.front() != Some(message) {
            let slot = message.0[1];
            if let Some(queue) = self.live_pages.get_mut(&slot) {
                if queue.front() == Some(message) {
                    queue.pop_front();
                    self.started_live.insert(slot);
                    if message.0[0] == 1 {
                        self.configured_fonts
                            .insert(slot, u16::from_le_bytes([message.0[18], message.0[19]]));
                    }
                }
                if queue.is_empty() {
                    self.live_pages.remove(&slot);
                    self.started_live.remove(&slot);
                    if let Some((id, text)) = self.deferred_live.remove(&slot) {
                        self.queue_live(&id, &text)
                            .expect("validated deferred caption");
                    }
                }
            }
            return;
        }
        self.pending.pop_front();
        let slot = message.0[1];
        if message.0[0] == 4 {
            self.sent.remove(&slot);
        } else if message.0[0] == 1 {
            self.configured_fonts
                .insert(slot, u16::from_le_bytes([message.0[18], message.0[19]]));
            if let Some((_, messages)) = self.target.values().find(|(s, _)| *s == slot) {
                self.sent.insert(slot, messages.clone());
            }
        }
    }
    pub fn active(&self) -> bool {
        !self.target.is_empty() || !self.sent.is_empty() || !self.tombstones.is_empty()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn drain(session: &mut Session) -> Vec<Message> {
        let mut frames = vec![];
        while let Some(m) = session.next() {
            session.commit(&m);
            frames.push(m);
        }
        frames
    }
    #[test]
    fn replay_restores_live_content_after_saved_panel_configuration() {
        let mut s = Session::default();
        let mut scene = Scene::default();
        scene.text_panels.push(TextPanel {
            text: "old".into(),
            ..TextPanel::caption()
        });
        s.set_target(&scene).unwrap();
        drain(&mut s);
        s.latest_text("text", "最新字幕").unwrap();
        let first = s.next_latest().unwrap();
        s.commit_latest(&first);
        s.replay();
        assert!(s.next_latest().is_none());
        drain(&mut s);
        assert_eq!(s.next_latest(), Some(first));
    }
    #[test]
    fn streaming_partials_finish_each_started_page_set_and_coalesce_the_next_one() {
        let mut s = Session::default();
        let mut scene = Scene::default();
        scene.text_panels.push(TextPanel::caption());
        s.set_target(&scene).unwrap();
        drain(&mut s);
        s.latest_text("text", "第一段字幕内容较长，至少会超过十四个字符。")
            .unwrap();
        let first = s.next().unwrap();
        s.commit(&first);
        s.latest_text("text", "第二段字幕内容较长，至少会超过十四个字符。")
            .unwrap();
        s.latest_text("text", "最新一段字幕内容较长，至少会超过十四个字符。")
            .unwrap();
        let frames = drain(&mut s);
        assert_eq!(
            frames.iter().filter(|m| m.0[0] == 1).count(),
            2,
            "finish the active version, then the newest pending version"
        );
        assert_eq!(s.pending(), 0);
        s.replay();
        let replay = drain(&mut s);
        assert!(replay.last().unwrap().0[24] > 14);
    }
    #[test]
    fn long_live_captions_keep_full_visible_lines_and_survive_other_panel_edits() {
        let mut s = Session::default();
        let mut scene = Scene::default();
        scene.text_panels.push(TextPanel {
            text: "等待".into(),
            ..TextPanel::caption()
        });
        scene.text_panels.push(TextPanel {
            id: "label".into(),
            text: "标签".into(),
            ..Default::default()
        });
        s.set_target(&scene).unwrap();
        drain(&mut s);
        let live = "过期第零行\n第一行字幕\n第二行字幕\n第三行字幕\n最新第四行字幕";
        s.latest_text("text", live).unwrap();
        let frames = drain(&mut s);
        let count = frames.last().unwrap().0[24] as usize;
        let encoded = frames
            .iter()
            .filter(|m| m.0[0] == 2)
            .flat_map(|m| {
                m.0[3..27]
                    .chunks_exact(2)
                    .map(|b| u16::from_le_bytes([b[0], b[1]]))
                    .collect::<Vec<_>>()
            })
            .take(count)
            .collect::<Vec<_>>();
        let displayed = String::from_utf16(&encoded).unwrap();
        assert_eq!(displayed, "第一行字幕\n第二行字幕\n第三行字幕\n最新第四行字幕");
        assert_eq!(
            u16::from_le_bytes([frames.last().unwrap().0[18], frames.last().unwrap().0[19]]),
            8 * 64
        );
        s.latest_text("text", live).unwrap();
        assert_eq!(s.pending(), 0);
        scene.text_panels[1].text = "新标签".into();
        s.set_target(&scene).unwrap();
        drain(&mut s);
        s.replay();
        let replay = drain(&mut s);
        assert_eq!(replay.last().unwrap().0[24] as usize, count);
    }
    #[test]
    fn deletion_replays_and_old_live_generations_are_invalidated() {
        let mut s = Session::default();
        let mut scene = Scene::default();
        scene.text_panels.push(TextPanel {
            text: "old".into(),
            ..Default::default()
        });
        s.set_target(&scene).unwrap();
        while let Some(m) = s.next() {
            s.commit(&m)
        }
        s.latest_text("text", "partial").unwrap();
        let old = s.next_latest().unwrap();
        assert_eq!(old.epoch, 0);
        scene.text_panels[0].text = "final".into();
        s.set_target(&scene).unwrap();
        while let Some(m) = s.next() {
            s.commit(&m)
        }
        s.latest_text("text", "new").unwrap();
        assert_eq!(s.next_latest().unwrap().epoch, 1);
        scene.text_panels.clear();
        s.set_target(&scene).unwrap();
        let delete = s.next().unwrap();
        assert_eq!(delete.0[0], 4);
        assert_eq!(delete.0[2], 2);
        s.commit(&delete);
        assert!(s.active());
        s.replay();
        assert_eq!(s.next().unwrap(), delete);
        scene.text_panels.push(TextPanel {
            id: "replacement".into(),
            text: "new".into(),
            ..Default::default()
        });
        s.set_target(&scene).unwrap();
        while let Some(m) = s.next() {
            s.commit(&m)
        }
        s.latest_text("replacement", "voice").unwrap();
        assert_eq!(s.next_latest().unwrap().epoch, 2);
    }
    #[test]
    fn pages_keep_unicode_and_explicit_word_layout() {
        let p = TextPanel {
            text: "ABC 中日\nEF".into(),
            ..Default::default()
        };
        let frames = encode(&p, 0).unwrap();
        assert_eq!(frames.last().unwrap().0[0], 1);
        assert_eq!(frames[1].0[28], 254);
        let (chars, _) = characters(&p).unwrap();
        assert!(chars.contains(&10));
        assert!(chars.contains(&('中' as u16)));
    }
    #[test]
    fn latest_window_coalesces_and_never_spends_the_command_sequence() {
        let mut s = Session::default();
        let mut scene = Scene::default();
        scene.text_panels.push(TextPanel::default());
        s.set_target(&scene).unwrap();
        s.latest_text("text", "old").unwrap();
        s.latest_text("text", "最新十四个字以内").unwrap();
        assert!(s.next_latest().is_none());
        while let Some(m) = s.next() {
            s.commit(&m)
        }
        let latest = s.next_latest().unwrap();
        assert_eq!(latest.data[28], 128 + 8);
        let bytes = latest.packet();
        assert!(!bytes.is_empty());
        s.commit_latest(&latest);
        assert!(s.next_latest().is_none());
        s.latest_text("text", "最新十四个字以内").unwrap();
        assert!(s.next_latest().is_none());
        assert!(s.latest_text("wrong", "a").is_err());
    }
    #[test]
    fn target_coalesces_against_sent_and_replays_without_geometry_clear() {
        let mut s = Session::default();
        let mut scene = Scene::default();
        scene.text_panels.push(TextPanel {
            text: "甲".into(),
            ..Default::default()
        });
        s.set_target(&scene).unwrap();
        while let Some(m) = s.next() {
            s.commit(&m)
        }
        assert_eq!(s.pending(), 0);
        s.set_target(&scene).unwrap();
        assert_eq!(s.pending(), 0);
        scene.text_panels[0].text = "乙".into();
        s.set_target(&scene).unwrap();
        scene.text_panels[0].text = "丙".into();
        s.set_target(&scene).unwrap();
        let msgs = s.pending.iter().cloned().collect::<Vec<_>>();
        assert_eq!(
            u16::from_le_bytes([msgs[1].0[3], msgs[1].0[4]]),
            '丙' as u16
        );
        while let Some(m) = s.next() {
            s.commit(&m)
        }
        s.replay();
        assert!(s.pending() > 0);
        scene.text_panels.clear();
        s.set_target(&scene).unwrap();
        assert_eq!(s.next().unwrap().0[0], 4);
    }
}
