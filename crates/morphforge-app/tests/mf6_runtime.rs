use morphforge_app::{store::Workspace, target::AvatarCatalog, transport::Transport};
use morphforge_core::{
    mf6::{Frame, BYTES},
    Box3, Scene,
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    net::UdpSocket,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
fn decode(packet: &[u8]) -> Option<Frame> {
    let mut cursor = 16;
    let mut values = BTreeMap::new();
    fn string(packet: &[u8], cursor: &mut usize) -> Option<String> {
        let end = packet[*cursor..].iter().position(|b| *b == 0)? + *cursor;
        let value = std::str::from_utf8(&packet[*cursor..end]).ok()?.to_string();
        *cursor = (end + 4) & !3;
        Some(value)
    }
    while cursor + 4 <= packet.len() {
        let size = u32::from_be_bytes(packet[cursor..cursor + 4].try_into().ok()?) as usize;
        cursor += 4;
        let end = cursor + size;
        if end > packet.len() {
            return None;
        }
        let path = string(packet, &mut cursor)?;
        let tag = string(packet, &mut cursor)?;
        if tag == ",i" {
            values.insert(
                path,
                i32::from_be_bytes(packet[cursor..cursor + 4].try_into().ok()?),
            );
        }
        cursor = end;
    }
    let mut bytes = [0u8; BYTES];
    for (i, byte) in bytes.iter_mut().enumerate().take(29) {
        *byte = *values.get(&format!("/avatar/parameters/MF6_D{i:02}"))? as u8;
    }
    bytes[29] = *values.get("/avatar/parameters/MF6_Ctl")? as u8;
    Frame::decode(&bytes).ok().map(|v| v.1)
}
static SERIAL: Mutex<()> = Mutex::new(());
struct Stop(Transport);
impl Drop for Stop {
    fn drop(&mut self) {
        self.0.inner.lock().unwrap().stop = true;
        let end = Instant::now() + Duration::from_secs(2);
        while self.0.inner.lock().unwrap().connected && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
fn read_until(t: &Transport, rx: &UdpSocket, revision: u64) -> Vec<Frame> {
    let deadline = Instant::now() + Duration::from_secs(6);
    let mut output = Vec::new();
    let mut bytes = [0; 8192];
    while Instant::now() < deadline {
        if let Ok(n) = rx.recv(&mut bytes) {
            if let Some(frame) = decode(&bytes[..n]) {
                output.push(frame);
            }
        }
        if t.inner.lock().unwrap().sent_revision == Some(revision) {
            return output;
        }
    }
    panic!("sender did not converge: {}", t.state())
}
#[test]
fn real_loopback_region_frames_and_auto_rebuild_preserve_color_and_coordinates() {
    let _serial = SERIAL.lock().unwrap();
    let root = tempfile::tempdir().unwrap();
    let w = Arc::new(Mutex::new(Workspace::open(root.path().into()).unwrap()));
    let t = Transport::new(w.clone());
    let _stop = Stop(t.clone());
    let mut c =
        AvatarCatalog::from_bytes(include_bytes!("../../../materials/mf6/catalog.json")).unwrap();
    c.capabilities.push("mf6-region8/1".into());
    t.set_catalog(Some(c), root.path()).unwrap();
    let rx = UdpSocket::bind("127.0.0.1:0").unwrap();
    rx.set_read_timeout(Some(Duration::from_millis(50)))
        .unwrap();
    let materials = (0..16)
        .map(|i| {
            let mut m = morphforge_core::materials::builtins().remove(0);
            m.id = i + 25;
            m.uid = format!("mf:runtime-{i}");
            m.color = [i as f32 / 15., 0.2, 0.3];
            m.metallic = 0.;
            m.opacity = 1.;
            m.emission = 0.;
            m
        })
        .collect::<Vec<_>>();
    let mut scene = Scene {
        revision: 1,
        boxes: (0..16)
            .map(|i| Box3::new([i, 0, 0], [1; 3], i as u32 + 25))
            .collect(),
        ..Default::default()
    };
    t.configure(&json!({"profile":"mf6","enabled":true,"port":rx.local_addr().unwrap().port(),"interval":0.05}),&mut w.lock().unwrap()).unwrap();
    t.publish_with_materials(&scene, &materials, false).unwrap();
    let frames = read_until(&t, &rx, 1);
    let regions = frames
        .iter()
        .filter_map(|f| {
            if let Frame::Region { region } = f {
                Some(region)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(t.state()["progress"]["percent"], 100.0);
    assert_eq!(t.state()["progress"]["remainingFrames"], 0);
    assert_eq!(t.state()["progress"]["receiverConfirmed"], false);
    assert_eq!(regions.len(), 2);
    assert_eq!(
        frames.iter().filter(|f| matches!(f, Frame::Clear)).count(),
        2
    );
    let records = regions
        .iter()
        .flat_map(|r| r.records().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 16);
    for (i, r) in records.iter().enumerate() {
        assert_eq!(r.geometry.min, [128 + i as u8, 0, 128]);
        assert_eq!(r.surface.bytes[0], (i * 17) as u8);
    }
    scene.revision = 2;
    for b in &mut scene.boxes {
        b.min[1] = 3;
    }
    t.publish_with_materials(&scene, &materials, false).unwrap();
    assert_eq!(t.state()["publication"]["selected"], "rebuild");
    let frames = read_until(&t, &rx, 2);
    assert_eq!(
        frames.iter().filter(|f| matches!(f, Frame::Clear)).count(),
        2
    );
    assert_eq!(t.inner.lock().unwrap().unified.sent_counts().0, 16);
    t.repair().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut data = [0; 8192];
    let mut births = 0;
    while Instant::now() < deadline {
        if let Ok(n) = rx.recv(&mut data) {
            if let Some(f) = decode(&data[..n]) {
                assert!(!matches!(f, Frame::Clear), "repair must preserve display");
                births += f.objects();
            }
        }
        if t.state()["repairing"] == false {
            break;
        }
    }
    assert_eq!(births, 16);
    assert_eq!(t.state()["progress"]["percent"], 100.0);
    assert_eq!(t.state()["remoteAcknowledged"], false);
}

#[test]
fn one_instance_owns_osc_and_another_can_take_over_after_release() {
    let _serial = SERIAL.lock().unwrap();
    let root = tempfile::tempdir().unwrap();
    let a = Arc::new(Mutex::new(Workspace::open(root.path().join("a")).unwrap()));
    let b = Arc::new(Mutex::new(Workspace::open(root.path().join("b")).unwrap()));
    let ta = Transport::new(a.clone());
    let tb = Transport::new(b.clone());
    let _a = Stop(ta.clone());
    let _b = Stop(tb.clone());
    let c =
        AvatarCatalog::from_bytes(include_bytes!("../../../materials/mf6/catalog.json")).unwrap();
    ta.set_catalog(Some(c.clone()), &a.lock().unwrap().root)
        .unwrap();
    tb.set_catalog(Some(c), &b.lock().unwrap().root).unwrap();
    let rx = UdpSocket::bind("127.0.0.1:0").unwrap();
    let port = rx.local_addr().unwrap().port();
    let patch = json!({"enabled":true,"port":port,"interval":0.05});
    ta.configure(&patch, &mut a.lock().unwrap()).unwrap();
    let end = Instant::now() + Duration::from_secs(2);
    while !ta.inner.lock().unwrap().connected && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(ta.inner.lock().unwrap().connected);
    tb.configure(&patch, &mut b.lock().unwrap()).unwrap();
    let end = Instant::now() + Duration::from_secs(2);
    while tb.inner.lock().unwrap().last_error.is_empty() && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!tb.config().enabled);
    assert_eq!(tb.state()["sentCommands"], 0);
    ta.configure(&json!({"enabled":false}), &mut a.lock().unwrap())
        .unwrap();
    let end = Instant::now() + Duration::from_secs(2);
    while ta.inner.lock().unwrap().connected && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(10));
    }
    tb.configure(&patch, &mut b.lock().unwrap()).unwrap();
    let end = Instant::now() + Duration::from_secs(2);
    while !tb.inner.lock().unwrap().connected && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(tb.inner.lock().unwrap().connected);
    assert!(!ta.inner.lock().unwrap().connected);
}
