use morphforge_app::{avatar_motion, creative, text_wire};
use morphforge_core::{
    material_wire::Surface,
    mf6::{BoxRecord, Frame, Geometry, TriangleRecord},
    text_panel::TextPanel,
    Box3, Scene, Triangle3,
};
use serde_json::json;
fn main() {
    let mut scene = Scene::default();
    let mut b = Box3::new([-12, 40, -4], [24, 4, 8], 1);
    b.group = "wind".into();
    scene.boxes.push(b.clone());
    let triangle = Triangle3 {
        id: 1,
        vertices: [[20, 20, 0], [30, 20, 0], [20, 40, 0]],
        color: [60, 160, 230],
        group: "door".into(),
        double_sided: true,
    };
    scene.triangles.push(triangle.clone());
    scene.text_panels.push(TextPanel {
        text: "风车 Wind".into(),
        position: [0., 70., 0.],
        size: [80., 16.],
        group: "wind".into(),
        ..Default::default()
    });
    let key = |time, angle, translation| creative::Keyframe {
        time,
        rotation: [0., angle, 0.],
        translation,
        scale: [1.; 3],
    };
    let clip = creative::Clip {
        name: "Native motion fixture".into(),
        duration: 4.,
        looping: true,
        tracks: vec![
            creative::Track {
                group: "wind".into(),
                pivot: [0., 42., 0.],
                interpolation: "linear".into(),
                keys: vec![
                    key(0., 0., [0.; 3]),
                    key(2., 180., [0.; 3]),
                    key(4., 360., [0.; 3]),
                ],
            },
            creative::Track {
                group: "door".into(),
                pivot: [20., 20., 0.],
                interpolation: "linear".into(),
                keys: vec![key(0., 0., [0.; 3]), key(4., 90., [8., 0., 0.])],
            },
        ],
    };
    let definition = avatar_motion::encode(&scene, &clip, 1., 0, 0).unwrap();
    let mut seq = 1u8;
    let mut packets = vec![json!({"kind":"clear","packet":Frame::Clear.packet(0).unwrap()})];
    for m in &definition.messages {
        packets
            .push(json!({"kind":"definition","op":m.0[0],"group":m.0[1],"packet":m.packet(seq)}));
        seq = (seq + 1) % 16;
    }
    for group in [1, 2] {
        let mut d = [0u8; 29];
        d[0] = 18;
        d[1] = group;
        d[4] = 1;
        d[5] = 2;
        d[6] = 64;
        d[28] = 254;
        packets.push(json!({"kind":"pause","packet":text_wire::Message(d).packet(seq)}));
        seq = (seq + 1) % 16;
    }
    let boxed = Frame::Boxes {
        shape: 0,
        records: vec![BoxRecord {
            pool: 0,
            geometry: Geometry::from_box(&b).unwrap(),
            surface: Surface {
                mode: 1,
                bytes: [220, 80, 40],
            },
        }],
    };
    packets.push(json!({"kind":"box","packet":boxed.packet_group(seq,1).unwrap()}));
    seq = (seq + 1) % 16;
    let tris = Frame::Triangles {
        records: vec![TriangleRecord {
            id: 1,
            vertices: triangle.wire_vertices().unwrap(),
            color: triangle.color,
            double_sided: true,
        }],
    };
    packets.push(json!({"kind":"triangle","packet":tris.packet_group(seq,2).unwrap()}));
    seq = (seq + 1) % 16;
    for mut m in text_wire::encode(&scene.text_panels[0], 0).unwrap() {
        if m.0[0] == 3 {
            m.0[13] = 1;
        }
        packets.push(json!({"kind":"text","packet":m.packet(seq)}));
        seq = (seq + 1) % 16;
    }
    let mut controls = serde_json::Map::new();
    for (name, ticket, state, phase) in [
        ("zero", 2, 2, 0),
        ("quarter", 3, 2, 64),
        ("play", 4, 1, 0),
        ("duplicate", 4, 1, 128),
        ("pause", 5, 2, 128),
        ("resume", 6, 1, 128),
        ("stop", 7, 0, 0),
    ] {
        let mut d = [0u8; 29];
        d[0] = 18;
        d[1] = 1;
        d[4] = ticket;
        d[5] = state;
        d[6] = phase;
        d[28] = 254;
        controls.insert(name.into(), json!(text_wire::Message(d).packet(seq)));
        seq = (seq + 1) % 16;
    }
    let bank1 = avatar_motion::encode(&scene, &clip, 1., 1, 1).unwrap();
    let mut bank_packets = vec![];
    for m in &bank1.messages {
        if m.0[1] == 1 {
            bank_packets.push(json!(m.packet(seq)));
            seq = (seq + 1) % 16;
        }
    }
    let mut d = [0u8; 29];
    d[0] = 18;
    d[1] = 1;
    d[2] = 1;
    d[3] = 1;
    d[4] = 8;
    d[5] = 2;
    d[6] = 64;
    d[28] = 254;
    controls.insert("bank1".into(), json!(text_wire::Message(d).packet(seq)));
    let path = std::env::args().nth(1).expect("output");
    std::fs::write(path,serde_json::to_vec_pretty(&json!({"packets":packets,"controls":controls,"bank1":bank_packets,"scene":scene,"clip":clip,"nextSeq":seq})).unwrap()).unwrap();
}
