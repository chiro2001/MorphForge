use morphforge_app::{store::Workspace, transport::Transport};
use morphforge_core::{Box3, Operation, Scene};
use serde_json::json;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
struct Stop(Transport);
impl Drop for Stop {
    fn drop(&mut self) {
        self.0.inner.lock().unwrap().stop = true;
    }
}
#[test]
fn periodic_refresh_waits_until_the_entire_previous_rebuild_was_sent() {
    let folder = tempfile::tempdir().unwrap();
    let w = Arc::new(Mutex::new(Workspace::open(folder.path().into()).unwrap()));
    let ops = (0..48)
        .map(|i| Operation::Box {
            value: Box3::new([i * 2 - 48, 0, 0], [1; 3], 1),
        })
        .collect::<Vec<_>>();
    w.lock()
        .unwrap()
        .replace(
            Scene::default().apply(&ops, &Default::default()).unwrap(),
            None,
            None,
        )
        .unwrap();
    let receiver = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let port = receiver.local_addr().unwrap().port();
    let t = Transport::new(w.clone());
    let _stop = Stop(t.clone());
    t.configure(
        &json!({"enabled":true,"port":port,"interval":0.05,"refreshSeconds":1}),
        &mut w.lock().unwrap(),
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(7);
    let mut completed = false;
    while Instant::now() < deadline {
        let s = t.inner.lock().unwrap();
        if s.sent_revision == Some(1) && s.sent.len() == 48 {
            completed = true;
            break;
        }
        drop(s);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        completed,
        "refresh timer must not clear a rebuild that takes longer than the timer interval"
    );
    std::thread::sleep(Duration::from_millis(250));
    assert_eq!(
        t.inner.lock().unwrap().sent_revision,
        Some(1),
        "refresh delay starts after the completed snapshot, not before a long rebuild"
    );
    let before = t.config();
    assert!(t.placement(&json!({"world":false,"scale":-1})).is_err());
    assert_eq!(
        t.config().world,
        before.world,
        "placement validation is atomic"
    );
}
