//! Compare actual codec plans from the locally sent ledger, not draft revisions.
use super::unified::Session;
use anyhow::{bail, Result};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Decision {
    pub selected: String,
    pub layout: String,
    pub compacted_boxes: usize,
    pub incremental_frames: Option<usize>,
    pub rebuild_frames: Option<usize>,
    pub incremental_seconds: Option<f64>,
    pub rebuild_seconds: Option<f64>,
    pub reason: String,
    pub baseline: &'static str,
}
pub(super) fn frames(mut session: Session, order: &str) -> Result<usize> {
    let mut n = 0;
    while let Some(plan) = session.next(order)? {
        plan.frame.encode((n % 16) as u8)?;
        session.commit(&plan);
        n += 1;
        if n > 65536 {
            bail!("发布计划未收敛")
        }
    }
    Ok(n)
}
pub fn compare(
    session: &Session,
    order: &str,
    interval: f64,
    queued_clear: usize,
    policy: &str,
) -> Decision {
    let mut incremental = session.clone();
    // Already queued clear packets will execute before births. Their old ledger
    // cannot be used as the baseline of this candidate.
    if queued_clear > 0 {
        incremental.clear_sent();
    }
    let inc = frames(incremental, order).ok().map(|n| n + queued_clear);
    let mut rebuild = session.clone();
    rebuild.clear_sent();
    let reset_count = queued_clear.max(2);
    let fresh = frames(rebuild, order).ok().map(|n| n + reset_count);
    let clear = match policy {
        "rebuild" => true,
        "incremental" => inc.is_none(),
        _ => match (inc, fresh) {
            (Some(a), Some(b)) => b < a,
            (None, Some(_)) => true,
            _ => false,
        },
    };
    Decision {
        layout: "boxes_and_regions".into(),
        compacted_boxes: 0,
        selected: if clear { "rebuild" } else { "incremental" }.into(),
        incremental_frames: inc,
        rebuild_frames: fresh,
        incremental_seconds: inc.map(|n| n as f64 * interval),
        rebuild_seconds: fresh.map(|n| n as f64 * interval),
        reason: if inc.is_none() {
            "增量池分布不可执行，重新平衡重建"
        } else if policy == "rebuild" {
            "手动要求清空重建"
        } else if policy == "incremental" {
            "显式保留显示"
        } else if clear {
            "包含两次Clear后重建仍需更少网络时隙"
        } else {
            "增量更快或持平，保留当前显示"
        }
        .into(),
        baseline: "local_udp_sent; not remote acknowledged",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use morphforge_core::{Box3, Scene};
    use std::collections::BTreeMap;
    fn settle(s: &mut Session) {
        while let Some(p) = s.next("volume").unwrap() {
            s.commit(&p);
        }
    }
    #[test]
    fn replacement_rebuilds_but_small_edits_and_equal_cost_keep_display() {
        let mut s = Session::default();
        let mut scene = Scene {
            boxes: (0..80).map(|i| Box3::new([i, 0, 0], [1; 3], 1)).collect(),
            ..Default::default()
        };
        s.set_target(&scene, &BTreeMap::new()).unwrap();
        settle(&mut s);
        scene.boxes[0].style_id = 2;
        s.set_target(&scene, &BTreeMap::new()).unwrap();
        let d = compare(&s, "volume", 0.25, 0, "auto");
        assert_eq!(d.selected, "incremental");
        assert_eq!(d.incremental_frames, Some(1));
        scene.boxes = (0..80).map(|i| Box3::new([i, 2, 0], [1; 3], 2)).collect();
        s.set_target(&scene, &BTreeMap::new()).unwrap();
        let d = compare(&s, "volume", 0.25, 0, "auto");
        assert_eq!(d.selected, "rebuild");
        assert!(d.rebuild_frames < d.incremental_frames);
        let forced = compare(&s, "volume", 0.25, 0, "incremental");
        assert_eq!(forced.selected, "incremental");
        assert_eq!(
            s.sent_counts().0,
            80,
            "dry-run must never change live ledger"
        );
    }
    #[test]
    fn queued_clear_is_in_both_candidates_and_identical_publish_needs_no_frames() {
        let mut s = Session::default();
        let scene = Scene {
            boxes: vec![Box3::new([0; 3], [2; 3], 1)],
            ..Default::default()
        };
        s.set_target(&scene, &BTreeMap::new()).unwrap();
        settle(&mut s);
        assert_eq!(
            compare(&s, "volume", 0.5, 0, "auto").incremental_frames,
            Some(0)
        );
        let d = compare(&s, "volume", 0.5, 2, "auto");
        assert_eq!(d.incremental_frames, Some(3));
        assert_eq!(d.rebuild_frames, Some(3));
        assert_eq!(d.selected, "incremental");
    }
}
