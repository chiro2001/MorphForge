//! Exercise the public tool boundary, native planning and wire writer with no LLM/VRChat.
use morphforge_app::{
    avatar_motion,
    creative::{Clip, Keyframe, Track},
    server::App,
    target::AvatarCatalog,
};
use morphforge_core::{Box3, Scene};
use serde_json::json;
fn clip() -> Clip {
    Clip {
        name: "rotor".into(),
        duration: 4.,
        looping: true,
        tracks: vec![Track {
            group: "rotor".into(),
            pivot: [0., 30., 0.],
            interpolation: "linear".into(),
            keys: vec![
                Keyframe {
                    time: 0.,
                    translation: [0.; 3],
                    rotation: [0.; 3],
                    scale: [1.; 3],
                },
                Keyframe {
                    time: 2.,
                    translation: [0.; 3],
                    rotation: [0., 180., 0.],
                    scale: [1.; 3],
                },
                Keyframe {
                    time: 4.,
                    translation: [0.; 3],
                    rotation: [0., 360., 0.],
                    scale: [1.; 3],
                },
            ],
        }],
    }
}
#[tokio::test]
async fn public_animation_tools_validate_publish_control_and_project_isolation() {
    let dir = tempfile::tempdir().unwrap();
    let app = App::open(dir.path().into(), None).unwrap();
    {
        let mut w = app.workspace.lock().unwrap();
        let mut s = Scene::default();
        s.revision = 1;
        // Adjacent boxes can be compacted. The alternate compact planner must preserve binding.
        for x in -4..4 {
            let mut b = Box3::new([x, 26, -2], [1, 8, 4], 1);
            b.group = "rotor".into();
            s.boxes.push(b);
        }
        w.project.scene = s;
        w.project.creative.clips.insert("spin".into(), clip());
    }
    let dry = app
        .harness
        .tool(
            "animation",
            &json!({"action":"publish","id":"spin","dryRun":true}),
            None,
        )
        .await
        .unwrap();
    assert_eq!(dry["groups"]["rotor"], 1);
    assert_eq!(app.transport.state()["motion"]["clipId"], "");
    assert!(app
        .harness
        .tool("animation", &json!({"action":"publish","id":"spin"}), None)
        .await
        .unwrap_err()
        .to_string()
        .contains("UNSUPPORTED"));
    let mut catalog =
        AvatarCatalog::from_bytes(include_bytes!("../../../materials/mf6/catalog.json")).unwrap();
    catalog.capabilities.push(avatar_motion::CAPABILITY.into());
    {
        let mut s = app.transport.inner.lock().unwrap();
        s.avatar_catalog = Some(catalog);
        s.config.port = 19012;
    }
    let result = app
        .harness
        .tool(
            "animation",
            &json!({"action":"publish","id":"spin","playing":false}),
            None,
        )
        .await
        .unwrap();
    assert_eq!(result["motion"]["state"], 2);
    {
        let s = app.transport.inner.lock().unwrap();
        assert_eq!(s.unified.motion_groups["rotor"], 1);
        assert_eq!(s.text.motion_groups["rotor"], 1);
    }
    for action in ["resume", "pause", "seek", "stop"] {
        let result = app
            .harness
            .tool("animation", &json!({"action":action,"position":1.}), None)
            .await
            .unwrap();
        assert_eq!(result["motion"]["position"].as_f64().unwrap().floor(), 1.);
    }
    let status = app
        .harness
        .tool("animation", &json!({"action":"status"}), None)
        .await
        .unwrap();
    assert_eq!(status["state"], 0);
    let mut invalid = Scene::default();
    invalid.revision = 2;
    let mut large = Box3::new([96, 26, -64], [32, 8, 128], 1);
    large.group = "rotor".into();
    invalid.boxes.push(large);
    let before = app.transport.state()["desiredRevision"].clone();
    assert!(app
        .transport
        .publish_with_materials(&invalid, &morphforge_core::materials::builtins(), false)
        .unwrap_err()
        .to_string()
        .contains("绑定动画"));
    assert_eq!(
        app.transport.state()["desiredRevision"],
        before,
        "invalid edit must not mutate queued target"
    );
    let detached = app
        .harness
        .tool("animation", &json!({"action":"detach"}), None)
        .await
        .unwrap();
    assert_eq!(detached["motion"]["attached"], false);
    assert!(app
        .transport
        .inner
        .lock()
        .unwrap()
        .unified
        .motion_groups
        .is_empty());
    app.transport
        .publish_with_materials(&invalid, &morphforge_core::materials::builtins(), false)
        .unwrap();
    let (project, session) = {
        let w = app.workspace.lock().unwrap();
        (w.project.id.clone(), w.session_id.clone())
    };
    app.harness
        .workspace_action(
            &json!({"action":"new_project","baseProjectId":project,"baseSessionId":session}),
        )
        .unwrap();
    assert_eq!(app.transport.state()["motion"]["clipId"], "");
    assert!(app
        .transport
        .inner
        .lock()
        .unwrap()
        .unified
        .motion_groups
        .is_empty());
    app.transport.inner.lock().unwrap().stop = true;
}

fn wire_bytes(packet: &[u8]) -> Option<[u8; 30]> {
    let mut cursor = 16;
    let mut out = [0; 30];
    let mut mask = 0u32;
    while cursor + 4 <= packet.len() {
        let size = u32::from_be_bytes(packet[cursor..cursor + 4].try_into().ok()?) as usize;
        cursor += 4;
        let end = cursor + size;
        if end > packet.len() {
            return None;
        }
        let length = packet[cursor..end].iter().position(|b| *b == 0)?;
        let path = std::str::from_utf8(&packet[cursor..cursor + length]).ok()?;
        cursor = (cursor + length + 4) & !3;
        if packet.get(cursor..cursor + 3)? == b",i\0" {
            let value = *packet.get(end - 1)?;
            let n = path
                .strip_prefix("/avatar/parameters/MF6_D")
                .and_then(|s| s.parse::<usize>().ok())
                .or_else(|| (path == "/avatar/parameters/MF6_Ctl").then_some(29));
            if let Some(n) = n.filter(|n| *n < 30) {
                out[n] = value;
                mask |= 1 << n;
            }
        }
        cursor = end;
    }
    (mask == (1 << 30) - 1).then_some(out)
}
#[tokio::test]
async fn actual_writer_sends_motion_tags_and_repair_keeps_control_ticket() {
    use std::{
        net::UdpSocket,
        time::{Duration, Instant},
    };
    let dir = tempfile::tempdir().unwrap();
    let app = App::open(dir.path().into(), None).unwrap();
    let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
    receiver.set_nonblocking(true).unwrap();
    let mut catalog =
        AvatarCatalog::from_bytes(include_bytes!("../../../materials/mf6/catalog.json")).unwrap();
    catalog.capabilities.push(avatar_motion::CAPABILITY.into());
    {
        let mut state = app.transport.inner.lock().unwrap();
        state.avatar_catalog = Some(catalog);
        state.config.port = receiver.local_addr().unwrap().port();
        state.config.interval = 0.05;
    }
    let mut scene = Scene::default();
    scene.revision = 1;
    let mut b = Box3::new([-4, 26, -2], [8, 8, 4], 1);
    b.group = "rotor".into();
    scene.boxes.push(b);
    app.transport
        .publish_motion(
            &scene,
            &morphforge_core::materials::builtins(),
            &clip(),
            "spin",
            1.,
            0.,
            false,
        )
        .unwrap();
    app.transport.inner.lock().unwrap().config.enabled = true;
    async fn drain(app: &App, receiver: &UdpSocket) -> Vec<[u8; 30]> {
        let end = Instant::now() + Duration::from_secs(5);
        let mut packets = vec![];
        let mut buffer = [0; 8192];
        while Instant::now() < end {
            while let Ok(n) = receiver.recv(&mut buffer) {
                if let Some(p) = wire_bytes(&buffer[..n]) {
                    packets.push(p)
                }
            }
            if app.transport.state()["sentRevision"] == 1 {
                return packets;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("writer failed: {}", app.transport.state())
    }
    let packets = drain(&app, &receiver).await;
    let control = packets.iter().find(|p| p[28] == 254 && p[0] == 18).unwrap();
    let ticket = control[4];
    assert!(packets.iter().any(|p| p[28] == 254 && p[0] == 16));
    assert_eq!(
        packets
            .iter()
            .filter(|p| p[28] == 254 && p[0] == 17)
            .count(),
        3
    );
    assert!(packets
        .iter()
        .filter_map(|p| morphforge_core::mf6::Frame::decode_group(p).ok())
        .any(|(_, group, frame)| group == 1 && frame.objects() > 0));
    app.transport.repair().unwrap();
    let repair = drain(&app, &receiver).await;
    assert!(repair
        .iter()
        .any(|p| p[28] == 254 && p[0] == 18 && p[4] == ticket));
    assert!(
        !repair.iter().any(|p| p[29] & 15 == 15),
        "blind repair must not clear geometry"
    );
    app.transport.inner.lock().unwrap().stop = true;
}
