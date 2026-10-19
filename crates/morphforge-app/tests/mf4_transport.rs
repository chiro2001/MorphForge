use morphforge_app::{store::Workspace, target::AvatarCatalog, transport::Transport};
use morphforge_core::{
    protocol::{self, Command},
    Box3, Scene,
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[test]
fn packed_appearance_preserves_all_fields_and_balances_capacity() {
    for style in [0, 1, 25, 26, 63, 64, 255, 256, 2048, 4095] {
        for tint in 0..64 {
            for motion in 0..64 {
                let mut b = Box3::new([0; 3], [1; 3], style);
                b.tint_id = tint;
                b.motion_phase = motion;
                let [r, g, b] = protocol::packed_appearance(&b);
                assert_eq!(r as u32 + 256 * (g as u32 % 16), style);
                assert_eq!(g / 16 + 16 * (b % 4), tint);
                assert_eq!(b / 4, motion);
            }
        }
    }
    let boxes: Vec<_> = (0..16384)
        .map(|i| Box3::new([i % 128 - 64, i / 128, 0], [1; 3], 1))
        .collect();
    let queues = protocol::wide_plan(&[], &boxes, &BTreeMap::new());
    assert_eq!(queues.iter().map(Vec::len).sum::<usize>(), 16384);
    assert!(queues
        .iter()
        .all(|q| q.len() <= protocol::WIDE_LANE_CAPACITY));
    let mut assignments = BTreeMap::new();
    for (lane, q) in queues.iter().enumerate() {
        for c in q {
            if let Command::Voxel { value } | Command::Box { value } = c {
                assignments.insert(protocol::key(value), lane);
            }
        }
    }
    let mut changed = boxes[1..].to_vec();
    changed[0].style_id = 4095;
    let delta = protocol::wide_plan(&boxes, &changed, &assignments);
    assert_eq!(delta.iter().map(Vec::len).sum::<usize>(), 2);
    assert!(matches!(
        delta[assignments[&protocol::key(&boxes[0])]][0],
        Command::Remove { .. }
    ));
    assert!(delta[assignments[&protocol::key(&boxes[1])]]
        .iter()
        .any(|c| matches!(c,Command::Voxel{value} if value.style_id==4095)));
}

fn decode(bytes: &[u8]) -> BTreeMap<String, i32> {
    assert_eq!(&bytes[..8], b"#bundle\0");
    let mut pos = 16;
    let mut out = BTreeMap::new();
    while pos < bytes.len() {
        let len = u32::from_be_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;
        let msg = &bytes[pos..pos + len];
        let end = msg.iter().position(|b| *b == 0).unwrap();
        let name = String::from_utf8(msg[..end].to_vec()).unwrap();
        let tag = (end + 4) / 4 * 4;
        if &msg[tag..tag + 2] == b",i" {
            out.insert(name, i32::from_be_bytes(msg[len - 4..].try_into().unwrap()));
        }
        pos += len;
    }
    out
}

#[test]
fn real_udp_three_lane_rebuild_idle_lane_sequence_and_custom_catalog_guard() {
    let folder = tempfile::tempdir().unwrap();
    let w = Arc::new(Mutex::new(Workspace::open(folder.path().into()).unwrap()));
    let t = Transport::new(w.clone());
    struct Stop(Transport);
    impl Drop for Stop {
        fn drop(&mut self) {
            self.0.inner.lock().unwrap().stop = true;
        }
    }
    let _stop = Stop(t.clone());
    let rx = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    rx.set_read_timeout(Some(Duration::from_millis(150)))
        .unwrap();
    let port = rx.local_addr().unwrap().port();
    assert!(t
        .configure(
            &json!({"enabled":true,"profile":"mf4","port":port}),
            &mut w.lock().unwrap()
        )
        .is_err());
    let catalog=AvatarCatalog::from_bytes(&serde_json::to_vec(&json!({"format":"MorphForgeMaterialCatalog/1","capabilities":["mf4-three-lane/1"],"materials":w.lock().unwrap().project.materials})).unwrap()).unwrap();
    t.set_catalog(Some(catalog), folder.path()).unwrap();
    let boxes: Vec<_> = (0..60)
        .map(|i| {
            let mut b = Box3::new([i - 30, 0, 0], [1; 3], (i % 24 + 1) as u32);
            b.tint_id = (i % 64) as u8;
            b.motion_phase = (63 - i % 64) as u8;
            b
        })
        .collect();
    let scene = Scene {
        boxes: boxes.clone(),
        revision: 1,
        ..Default::default()
    };
    t.configure(
        &json!({"enabled":true,"profile":"mf4","port":port,"interval":0.05}),
        &mut w.lock().unwrap(),
    )
    .unwrap();
    t.publish(&scene, false).unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut buffer = [0; 4096];
    let mut draws = 0;
    let mut batches = 0;
    let mut last = [None; 3];
    while Instant::now() < deadline {
        if let Ok(n) = rx.recv(&mut buffer) {
            let values = decode(&buffer[..n]);
            let mut count = 0;
            for lane in 0..3 {
                let prefix = format!("/avatar/parameters/MF4_L{lane}_");
                if let Some(ctl) = values.get(&(prefix.clone() + "Ctl")) {
                    let seq = ctl >> 4;
                    assert_ne!(last[lane], Some(seq));
                    last[lane] = Some(seq);
                    if ctl & 15 == 1 || ctl & 15 == 2 {
                        count += 1;
                        assert_eq!(values.len(), 30);
                        let x = values[&(prefix.clone() + "D00")] - 128;
                        let source = &boxes[(x + 30) as usize];
                        let appearance = protocol::packed_appearance(source);
                        for i in 0..3 {
                            assert_eq!(
                                values[&(prefix.clone() + &format!("D{:02}", i + 6))],
                                appearance[i] as i32
                            );
                        }
                    }
                }
            }
            if count > 0 {
                draws += count;
                batches += 1;
            }
        }
        if t.inner.lock().unwrap().sent_revision == Some(1) {
            break;
        }
    }
    assert_eq!(draws, 60);
    assert_eq!(batches, 20);
    assert_eq!(t.state()["syncedBits"], 240);
    // After sixteen global ticks on just one lane, the other lanes must retain
    // their own next sequence. A global sequence would alias the last commit.
    let lane0_key = t
        .inner
        .lock()
        .unwrap()
        .lanes
        .iter()
        .find(|(_, l)| **l == 0)
        .map(|(k, _)| *k)
        .unwrap();
    let idle_sequences = {
        let s = t.inner.lock().unwrap();
        s.wide_seqs
    };
    let mut changed = scene;
    for step in 0..16 {
        changed.revision += 1;
        changed
            .boxes
            .iter_mut()
            .find(|b| protocol::key(b) == lane0_key)
            .unwrap()
            .style_id = if step % 2 == 0 { 23 } else { 24 };
        t.publish(&changed, false).unwrap();
        let stop = Instant::now() + Duration::from_secs(2);
        while t.inner.lock().unwrap().sent_revision != Some(changed.revision)
            && Instant::now() < stop
        {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    let s = t.inner.lock().unwrap();
    assert_eq!(s.wide_seqs[1..], idle_sequences[1..]);
    assert_eq!(s.wide_seqs[0], idle_sequences[0]);
    drop(s);
    t.configure(&json!({"enabled":false}), &mut w.lock().unwrap())
        .unwrap();
    let mut custom = Box3::new([0; 3], [1; 3], 25);
    custom.tint_id = 0;
    let report = morphforge_app::target::compatibility(
        &Scene {
            boxes: vec![custom],
            ..Default::default()
        },
        &w.lock().unwrap().project.materials,
        None,
    );
    assert_eq!(report["matched"], false);
    assert_eq!(report["status"], "unverified_custom");
}

#[test]
fn same_numeric_id_cannot_alias_an_animated_material_or_wrong_wire_protocol() {
    let folder = tempfile::tempdir().unwrap();
    let w = Arc::new(Mutex::new(Workspace::open(folder.path().into()).unwrap()));
    let t = Transport::new(w.clone());
    t.inner.lock().unwrap().stop = true;
    let shipped: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../../materials/mf4/catalog.json")).unwrap();
    let palette: Vec<morphforge_core::materials::Material> =
        serde_json::from_value(shipped["materials"].clone()).unwrap();
    let scene = Scene {
        boxes: vec![Box3::new([0; 3], [1; 3], 25)],
        ..Default::default()
    };
    assert!(
        t.publish_with_materials(&scene, &palette, false).is_err(),
        "unverified custom IDs must fail before queue mutation"
    );
    let mut old = shipped.clone();
    let entry = old["materials"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|m| m["id"] == 25)
        .unwrap();
    entry["uid"] = json!("old:animated-fixture");
    entry["frames"] = json!(3);
    entry["frameTime"] = json!(0.05);
    entry["textures"] = json!(vec!["/artifacts/fixture.png"; 6]);
    let catalog = AvatarCatalog::from_bytes(&serde_json::to_vec(&old).unwrap()).unwrap();
    t.set_catalog(Some(catalog), folder.path()).unwrap();
    assert_eq!(
        t.material_report(&scene, &palette)["differentIds"],
        json!([25])
    );
    assert!(t.publish_with_materials(&scene, &palette, false).is_err());
    assert!(t.inner.lock().unwrap().desired.is_empty());
    assert_eq!(t.state()["sentCommands"], 0);
    assert!(t
        .configure(
            &json!({"profile":"mf3","enabled":true}),
            &mut w.lock().unwrap()
        )
        .unwrap_err()
        .to_string()
        .contains("PROTOCOL_MISMATCH"));
    let correct = AvatarCatalog::from_bytes(&serde_json::to_vec(&shipped).unwrap()).unwrap();
    t.set_catalog(Some(correct), folder.path()).unwrap();
    assert!(t.publish_with_materials(&scene, &palette, false).is_ok());
    assert_eq!(t.inner.lock().unwrap().desired.len(), 1);
}
