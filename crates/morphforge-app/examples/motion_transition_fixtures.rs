//! Capture the actual writer while replacing rotation/sideways motion with waterfall translation.
use morphforge_app::{
    avatar_motion,
    creative::{Clip, Keyframe, Track},
    server::App,
    target::AvatarCatalog,
};
use morphforge_core::{Box3, Scene};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    net::UdpSocket,
    time::{Duration, Instant},
};
fn uniforms(clip: &Clip, bank: u8, generation: u8, scene: &Scene) -> Value {
    let definition = avatar_motion::encode(scene, clip, 1., bank, generation).unwrap();
    let mut values = BTreeMap::new();
    for m in definition.messages {
        let d = m.0;
        let g = d[1];
        let prefix = format!("_MF_M{g}B{bank}_");
        let word = |i| u16::from_le_bytes([d[i], d[i + 1]]) as f64;
        if d[0] == 16 {
            values.insert(format!("{prefix}Count"), d[6] as f64);
            values.insert(format!("{prefix}Interp"), d[7] as f64);
            for (a, axis) in ["X", "Y", "Z"].iter().enumerate() {
                values.insert(format!("{prefix}Pivot{axis}"), word(8 + a * 2) / 64. - 512.);
            }
        } else {
            let k = d[3];
            values.insert(format!("{prefix}K{k}T"), word(4) / 65535.);
            for (a, axis) in ["X", "Y", "Z"].iter().enumerate() {
                values.insert(format!("{prefix}K{k}P{axis}"), word(6 + a * 2) / 64. - 512.);
            }
            for (a, axis) in ["X", "Y", "Z", "W"].iter().enumerate() {
                values.insert(
                    format!("{prefix}K{k}Q{axis}"),
                    (word(12 + a * 2) - 32768.) / 32767.,
                );
            }
            values.insert(format!("{prefix}K{k}S"), word(20) / 4096.);
        }
    }
    json!(values)
}
#[tokio::main]
async fn main() {
    let dir = tempfile::tempdir().unwrap();
    let app = App::open(dir.path().into(), Some(dir.path().join("no-key.env"))).unwrap();
    let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
    receiver.set_nonblocking(true).unwrap();
    let mut catalog =
        AvatarCatalog::from_bytes(include_bytes!("../../../materials/mf6/catalog.json")).unwrap();
    catalog.capabilities.push(avatar_motion::CAPABILITY.into());
    {
        let mut s = app.transport.inner.lock().unwrap();
        s.avatar_catalog = Some(catalog);
        s.config.port = receiver.local_addr().unwrap().port();
        s.config.interval = 0.25;
    }
    let groups = ["fall-main", "fall-b", "stream"];
    let mut scene = Scene::default();
    scene.revision = 1;
    let mut materials = morphforge_core::materials::builtins();
    for (i, group) in groups.iter().enumerate() {
        let mut b = Box3::new(
            [-24 + i as i32 * 20, 50, if i == 2 { 20 } else { -4 }],
            [6, 10, 8],
            500 + i as u32,
        );
        b.group = group.to_string();
        b.id = i as u64 + 1;
        scene.boxes.push(b);
        let mut m = materials[0].clone();
        m.id = 500 + i as u32;
        m.uid = format!("transition-fixture-{i}");
        m.color = [[0.86, 0.15, 0.1], [0.1, 0.86, 0.15], [0.1, 0.15, 0.86]][i];
        materials.push(m);
    }
    let make = |old: bool| Clip {
        name: if old {
            "previous windmill"
        } else {
            "waterfall"
        }
        .into(),
        duration: 2.,
        looping: true,
        tracks: groups
            .iter()
            .enumerate()
            .map(|(g, group)| Track {
                group: group.to_string(),
                pivot: [0., 40., 0.],
                interpolation: "linear".into(),
                keys: [0., 0.667, 1.333, 2.]
                    .into_iter()
                    .enumerate()
                    .map(|(k, time)| Keyframe {
                        time,
                        scale: [1.; 3],
                        rotation: if old && g == 0 {
                            [0., 0., k as f64 * 120.]
                        } else {
                            [0.; 3]
                        },
                        translation: if old {
                            match g {
                                1 => [0., k as f64 * 5., 0.],
                                2 => [k as f64 * 4., 0., 0.],
                                _ => [0.; 3],
                            }
                        } else {
                            match g {
                                0 => [0., -(k as f64) * 6.667, 0.],
                                1 => [0., -(k as f64) * 5.333, 0.],
                                _ => [0., 0., k as f64 * 8.],
                            }
                        },
                    })
                    .collect(),
            })
            .collect(),
    };
    let mut stages = vec![];
    let mut bytes = [0; 8192];
    for (index, old) in [true, false, true, false].into_iter().enumerate() {
        let clip = make(old);
        app.transport
            .publish_motion(
                &scene,
                &materials,
                &clip,
                if old { "old" } else { "new" },
                1.,
                1.,
                false,
            )
            .unwrap();
        app.transport.inner.lock().unwrap().config.enabled = true;
        let start = Instant::now();
        let mut packets = vec![];
        loop {
            while let Ok(n) = receiver.recv(&mut bytes) {
                packets.push(
                    json!({"time":start.elapsed().as_secs_f64(),"packet":bytes[..n].to_vec()}),
                );
            }
            if app.transport.state()["pending"] == 0 {
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(45));
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let bank = (index % 2) as u8;
        stages.push(json!({"name":format!("{} bank{bank} generation{index}",clip.name),"packets":packets,"expectedUniforms":uniforms(&clip,bank,index as u8,&scene),"motion":app.transport.state()["motion"]}));
    }
    app.transport.inner.lock().unwrap().stop = true;
    std::fs::write(
        std::env::args().nth(1).expect("output"),
        serde_json::to_vec_pretty(&json!({"stages":stages})).unwrap(),
    )
    .unwrap();
}
