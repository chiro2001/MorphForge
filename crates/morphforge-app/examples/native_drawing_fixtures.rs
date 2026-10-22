use morphforge_app::transport::unified::Session;
use morphforge_core::{drawing::Drawing, mf6::Frame, Scene};
use serde_json::json;
use std::collections::BTreeMap;
fn main() {
    let mut scene = Scene::default();
    scene.drawings = vec![
        Drawing {
            id: 1,
            kind: "point".into(),
            start: [-20, 42, 0],
            end: [-20, 42, 0],
            diameter64: 6 * 64,
            color: [255, 64, 32],
            group: "plot".into(),
        },
        Drawing {
            id: 2,
            kind: "segment".into(),
            start: [-8, 42, 0],
            end: [24, 42, 0],
            diameter64: 2 * 64,
            color: [32, 150, 255],
            group: "plot".into(),
        },
        Drawing {
            id: 3,
            kind: "cube_point".into(),
            start: [0, 55, 0],
            end: [0, 55, 0],
            diameter64: 8 * 64,
            color: [64, 255, 64],
            group: "plot".into(),
        },
    ];
    let mut session = Session::default();
    let mut seq = 0;
    let clear = Frame::Clear.packet(seq).unwrap();
    seq += 1;
    let mut collect = |s: &mut Session| {
        let mut rows = vec![];
        while let Some(plan) = s.next("volume").unwrap() {
            rows.push(json!({"frame":plan.frame,"packet":plan.frame.packet_group(seq,s.frame_group(&plan)).unwrap()}));
            seq = (seq + 1) % 16;
            s.commit(&plan);
        }
        rows
    };
    session.set_target(&scene, &BTreeMap::new()).unwrap();
    let thin = collect(&mut session);
    scene.drawings[0].diameter64 = 12 * 64;
    scene.drawings[1].diameter64 = 6 * 64;
    session.set_target(&scene, &BTreeMap::new()).unwrap();
    let thick = collect(&mut session);
    session.start_replay();
    let replay = collect(&mut session);
    session.finish_replay();
    scene.drawings.remove(0);
    session.set_target(&scene, &BTreeMap::new()).unwrap();
    let delete = collect(&mut session);
    std::fs::write(std::env::args().nth(1).expect("output"),serde_json::to_vec_pretty(&json!({"clear":clear,"thin":thin,"thick":thick,"replay":replay,"delete":delete,"scene":scene})).unwrap()).unwrap();
}
