//! Real planner/codec costs, synthetic geometry, no OSC or model calls.
use morphforge_app::{
    store::Workspace,
    target::{self, AvatarCatalog},
    transport::{strategy, unified::Session, Transport},
};
use morphforge_core::{Box3, Scene};
use serde_json::json;
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};
fn drain(s: &mut Session) -> usize {
    let mut n = 0;
    while let Some(p) = s.next("volume").unwrap() {
        p.frame.encode((n % 16) as u8).unwrap();
        s.commit(&p);
        n += 1;
    }
    n
}
fn main() -> anyhow::Result<()> {
    let time = Instant::now();
    let mut c = AvatarCatalog::from_bytes(include_bytes!("../../../materials/mf6/catalog.json"))?;
    c.capabilities.push("mf6-region8/1".into());
    let mut colors = Vec::new();
    let mut scene = Scene {
        revision: 1,
        ..Default::default()
    };
    for i in 0..4096u32 {
        let mut m = morphforge_core::materials::builtins().remove(0);
        m.id = 25 + i;
        m.uid = format!("mf:runtime-benchmark-{i}");
        m.color = [(i % 256) as f32 / 255., (i / 256) as f32 / 255., 0.5];
        m.metallic = 0.;
        m.emission = 0.;
        m.opacity = 1.;
        colors.push(m);
        scene.boxes.push(Box3::new(
            [(i % 256) as i32 - 128, 0, (i / 256) as i32 - 128],
            [1; 3],
            25 + i,
        ));
    }
    let projected = target::project_wire(&scene, &colors, Some(&c))?;
    let mut old = Session::default();
    old.set_target(&projected.scene, &projected.surfaces)?;
    let normal = drain(&mut old);
    let mut packed = Session::default();
    packed.region_enabled = true;
    packed.set_target(&projected.scene, &projected.surfaces)?;
    let regions = drain(&mut packed);
    let moved = Scene {
        revision: 2,
        boxes: projected
            .scene
            .boxes
            .iter()
            .cloned()
            .map(|mut b| {
                b.min[1] += 3;
                b
            })
            .collect(),
        ..Default::default()
    };
    // Geometry keys change when translated; preserve each encoded surface.
    let moved_surfaces = moved
        .boxes
        .iter()
        .zip(&projected.scene.boxes)
        .map(|(a, b)| {
            (
                morphforge_core::protocol::key(a),
                projected.surfaces[&morphforge_core::protocol::key(b)],
            )
        })
        .collect();
    packed.set_target(&moved, &moved_surfaces)?;
    let replacement = strategy::compare(&packed, "volume", 0.4, 0, "auto");
    let dir = std::env::temp_dir().join(format!(
        "morphforge-publish-benchmark-{}",
        uuid::Uuid::new_v4()
    ));
    let w = Arc::new(Mutex::new(Workspace::open(dir.clone())?));
    let t = Transport::new(w);
    t.set_catalog(Some(c), &dir)?;
    let uniform = Scene {
        revision: 1,
        boxes: scene
            .boxes
            .iter()
            .cloned()
            .map(|mut b| {
                b.style_id = 1;
                b
            })
            .collect(),
        ..Default::default()
    };
    t.publish_with_materials(&uniform, &morphforge_core::materials::builtins(), false)?;
    let rle = t.state()["publication"].clone();
    t.inner.lock().unwrap().stop = true;
    let report = json!({"success":normal>regions&&replacement.selected=="rebuild","cells":4096,"variableRgb":{"regularFrames":normal,"regionFrames":regions,"secondsAt040":[normal as f64*0.4,regions as f64*0.4]},"replacement":replacement,"uniformRun":rle,"elapsedMs":time.elapsed().as_millis(),"scope":"Actual planner next/encode/commit including two Clear packets in the rebuild candidate. Synthetic scenes; no UDP, Avatar execution or multiplayer timing claim."});
    println!("{}", serde_json::to_string_pretty(&report)?);
    let _ = std::fs::remove_dir_all(dir);
    Ok(())
}
