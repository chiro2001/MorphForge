//! Exercise the real paced writer against a loopback UDP receiver. No VRChat.
use morphforge_app::{server::App, target::AvatarCatalog};
use morphforge_core::text_panel::TextPanel;
use std::{
    net::UdpSocket,
    time::{Duration, Instant},
};
#[tokio::test]
async fn continuous_long_captions_leave_bandwidth_for_geometry_and_version_completion() {
    let dir = tempfile::tempdir().unwrap();
    let app = App::open(dir.path().into(), None).unwrap();
    let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
    receiver.set_nonblocking(true).unwrap();
    let mut scene = morphforge_core::Scene::default();
    scene.revision = 1;
    for x in 0..32 {
        scene.boxes.push(morphforge_core::Box3::new(
            [(x % 8) * 4, 0, (x / 8) * 4],
            [1; 3],
            1,
        ));
    }
    scene.text_panels.push(TextPanel::caption());
    let catalog = AvatarCatalog {
        format: "MorphForgeMaterialCatalog/1".into(),
        version: String::new(),
        materials: vec![],
        capabilities: vec![
            "mf6-unified/1".into(),
            "mf5-runtime-materials/1".into(),
            "mf6-text-panels/1".into(),
            "mf6-text-latest14/1".into(),
        ],
        texture_options: None,
    };
    {
        let mut s = app.transport.inner.lock().unwrap();
        s.avatar_catalog = Some(catalog);
        s.config.port = receiver.local_addr().unwrap().port();
        s.config.interval = 0.05;
        s.config.enabled = true;
    }
    app.transport
        .publish_with_materials(&scene, &morphforge_core::materials::builtins(), false)
        .unwrap();
    let transport = app.transport.clone();
    let updates = tokio::spawn(async move {
        for i in 0..120 {
            transport
                .latest_text(
                    "text",
                    &format!("正在持续更新第{i}条实时字幕，必须保留多行并允许模型一起发布。"),
                )
                .unwrap();
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while app.transport.state()["sentRevision"] != 1 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        !updates.is_finished(),
        "geometry version must finish while captions are still streaming"
    );
    assert_eq!(app.transport.state()["sentObjects"]["boxes"], 32);
    updates.await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while app.transport.state()["pending"] != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    app.transport.inner.lock().unwrap().stop = true;
}
#[tokio::test]
async fn shared_writer_publishes_replays_and_deletes_text_without_geometry() {
    let dir = tempfile::tempdir().unwrap();
    let app = App::open(dir.path().into(), None).unwrap();
    let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
    receiver.set_nonblocking(true).unwrap();
    let port = receiver.local_addr().unwrap().port();
    let mut scene = morphforge_core::Scene::default();
    scene.revision = 1;
    scene.text_panels.push(TextPanel {
        text: "Hello 中文".into(),
        ..Default::default()
    });
    let catalog = AvatarCatalog {
        format: "MorphForgeMaterialCatalog/1".into(),
        version: String::new(),
        materials: vec![],
        capabilities: vec![
            "mf6-unified/1".into(),
            "mf5-runtime-materials/1".into(),
            "mf6-text-panels/1".into(),
            "mf6-text-latest14/1".into(),
        ],
        texture_options: None,
    };
    {
        let mut s = app.transport.inner.lock().unwrap();
        s.avatar_catalog = Some(catalog);
        s.config.port = port;
        s.config.interval = 0.05;
        s.config.enabled = true;
    }
    app.transport
        .publish_with_materials(&scene, &[], false)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut packets = vec![];
    let mut bytes = [0; 4096];
    while Instant::now() < deadline {
        while let Ok(n) = receiver.recv(&mut bytes) {
            packets.push(bytes[..n].to_vec())
        }
        if app.transport.state()["sentRevision"] == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        app.transport.state()["sentRevision"],
        1,
        "{}",
        app.transport.state()
    );
    assert!(packets.len() >= 3);
    {
        let mut s = app.transport.inner.lock().unwrap();
        s.config.enabled = false;
        s.text.latest_text("text", "实时字幕").unwrap();
    }
    app.transport
        .publish_with_materials(&scene, &[], false)
        .unwrap();
    assert_eq!(
        app.transport.state()["progress"]["totalFrames"],
        0,
        "live snapshots must not inflate completed publication progress"
    );
    app.transport.inner.lock().unwrap().config.enabled = true;
    let before = app.transport.state()["sentCommands"].as_u64().unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while app.transport.state()["sentCommands"].as_u64().unwrap() == before {
            tokio::time::sleep(Duration::from_millis(10)).await
        }
    })
    .await
    .unwrap();
    app.transport.repair().unwrap();
    assert!(app.transport.state()["pending"].as_u64().unwrap() > 0);
    scene.revision = 2;
    scene.text_panels.clear();
    app.transport
        .publish_with_materials(&scene, &[], false)
        .unwrap();
    tokio::time::timeout(Duration::from_secs(4), async {
        while app.transport.state()["sentRevision"] != 2 {
            tokio::time::sleep(Duration::from_millis(10)).await
        }
    })
    .await
    .unwrap();
    assert_eq!(app.transport.state()["pending"], 0);
    app.transport.inner.lock().unwrap().stop = true;
    assert_eq!(app.transport.state()["syncedBits"], 240);
}
