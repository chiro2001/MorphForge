use morphforge_app::{
    server::App,
    store::Workspace,
    target::{self, AvatarCatalog},
    transport::Transport,
};
use morphforge_core::{
    material_wire::{self as wire, Surface},
    materials::{Material, Procedural},
    protocol::{self, Command},
    Box3, Scene,
};
use serde_json::json;
use std::sync::{Arc, Mutex};
fn catalog() -> AvatarCatalog {
    AvatarCatalog::from_bytes(include_bytes!("../../../materials/mf6/catalog.json")).unwrap()
}
fn material() -> Material {
    let mut m = morphforge_core::materials::builtins()[0].clone();
    m.id = 25;
    m.uid = "mf:runtime-test".into();
    m.color = [0.12, 0.53, 0.98];
    m.metallic = 0.;
    m.roughness = 0.9;
    m.opacity = 1.;
    m.emission = 0.;
    m
}
#[test]
fn wire_color_and_parametric_fields_and_signed_scale_are_independent() {
    let mut m = material();
    for r in 0..256 {
        for g in [0, 1, 63, 127, 128, 254, 255] {
            m.color = [r as f32 / 255., g as f32 / 255., 0.37];
            let s = Surface::runtime(&m, 0).unwrap();
            assert_eq!(s.mode, 1);
            assert_eq!(s.bytes[0], r as u8);
            assert_eq!(s.bytes[1], g as u8);
        }
    }
    for metal in 0..4 {
        for rough in 0..4 {
            for alpha in 0..4 {
                for light in 0..4 {
                    m.metallic = metal as f32 / 3.;
                    m.roughness = rough as f32 / 3.;
                    m.opacity = 1. - alpha as f32 / 3.;
                    m.emission = light as f32;
                    let s = Surface::runtime(&m, 0).unwrap();
                    let d = s.decode().unwrap();
                    assert_eq!(s.mode, 2);
                    assert!((d.metallic - m.metallic).abs() < 1e-6);
                    assert!((d.roughness - m.roughness).abs() < 1e-6);
                    assert!((d.opacity - m.opacity).abs() < 1e-6);
                    assert_eq!(d.emission, m.emission);
                }
            }
        }
    }
    m.metallic = 0.;
    m.procedural = Some(Procedural {
        kind: "wood".into(),
        scale: 2.,
        seed: 123,
    });
    let s = Surface::runtime(&m, 0).unwrap();
    assert_eq!(s.mode, 5);
    assert_eq!(s.decode().unwrap().seed, 3);
    for (physical, expected) in [(0.0005, -1.), (0.05, 1. / 3.), (0.5, 1.)] {
        let wire = wire::scale_wire(physical);
        assert!((wire - expected).abs() < 1e-5);
        assert!((wire::scale_physical(wire) - physical).abs() < 1e-6);
    }
    let mut previous = 0.;
    for n in 0..=254 {
        let p = wire::scale_physical(n as f32 / 127. - 1.);
        assert!(p >= previous);
        previous = p;
    }
}
#[tokio::test]
async fn runtime_materials_are_atomic_reusable_and_resolve_without_baked_ids() {
    let dir = tempfile::tempdir().unwrap();
    let app = App::open(dir.path().into(), Some(dir.path().join("none.env"))).unwrap();
    let r=app.harness.tool("define_materials",&json!({"baseRevision":0,"materials":[{"name":"桃红","kind":"color","color":"#F179AD"},{"name":"银","kind":"pbr","color":"#C1D3E8","metallic":1,"roughness":0.25},{"name":"湖","kind":"water","color":"#246F9B"},{"name":"云","kind":"cloud","color":"#F6F8FC"},{"name":"木","kind":"wood","color":"#A46D3A","seed":3},{"name":"石","kind":"stone","color":"#889099"}]}),None).await.unwrap();
    assert_eq!(r["added"], 6);
    let before = {
        let w = app.workspace.lock().unwrap();
        serde_json::to_value(&w.project).unwrap()
    };
    assert!(app.harness.tool("define_materials",&json!({"baseRevision":1,"materials":[{"name":"ok","kind":"color","color":"#000011"},{"name":"bad","kind":"pbr","color":"#FFFFFF","roughness":3}]}),None).await.is_err());
    assert_eq!(
        serde_json::to_value(&app.workspace.lock().unwrap().project).unwrap(),
        before
    );
    let same=app.harness.tool("define_materials",&json!({"baseRevision":1,"materials":[{"name":"同一粉","kind":"color","color":"#F179AD"}]}),None).await.unwrap();
    assert_eq!(same["added"], 0);
    assert_eq!(same["revision"], 1);
    let w = app.workspace.lock().unwrap();
    let boxes: Vec<_> = r["materials"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(i, m)| {
            Box3::new(
                [i as i32 * 2, 0, 0],
                [1; 3],
                m["id"].as_u64().unwrap() as u32,
            )
        })
        .collect();
    let scene = Scene {
        boxes,
        ..Default::default()
    };
    let c = catalog();
    let projected = target::project_wire(&scene, &w.project.materials, Some(&c)).unwrap();
    assert_eq!(
        projected
            .surfaces
            .values()
            .map(|s| s.mode)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5, 6]
    );
    assert_eq!(
        target::compatibility(&scene, &w.project.materials, Some(&c))["matched"],
        true
    );
    let mut changed = w.project.materials.clone();
    let style = scene.boxes[0].style_id;
    changed.iter_mut().find(|m| m.id == style).unwrap().color = [1., 0., 0.];
    let changed = target::project_wire(&scene, &changed, Some(&c)).unwrap();
    assert_ne!(changed.surfaces, projected.surfaces);
}
#[test]
fn builtin_identity_remap_and_texture_tint_preserve_missing_image_guard() {
    let c = catalog();
    let original = c
        .materials
        .iter()
        .find(|m| m.uid == "mf-pixel:stone")
        .unwrap();
    let mut moved = original.clone();
    moved.id = 900;
    let scene = Scene {
        boxes: vec![Box3::new([0; 3], [1; 3], 900)],
        ..Default::default()
    };
    let projected = target::project_wire(&scene, &[moved.clone()], Some(&c)).unwrap();
    let s = *projected.surfaces.values().next().unwrap();
    assert_eq!(s.mode, 0);
    assert_eq!(wire::bits(s.bytes) & 4095, original.id);
    moved.color = [1., 0.2, 0.4];
    moved.uid = "tinted:stone".into();
    let p = target::project_wire(&scene, &[moved.clone()], Some(&c)).unwrap();
    assert_eq!(p.surfaces.values().next().unwrap().mode, 7);
    moved.textures = vec!["/artifacts/not-baked.png".into(); 6];
    assert!(target::project_wire(&scene, &[moved], Some(&c)).is_err());
    let a = Box3::new([0, 0, 0], [1; 3], 1);
    let b = Box3::new([1, 0, 0], [1; 3], 1);
    for mode in 0..3 {
        let vals = protocol::mf5_values(
            &Command::Pair {
                first: a.clone(),
                second: b.clone(),
            },
            7,
            2,
            Surface {
                mode,
                bytes: [3, 4, 5],
            },
        )
        .unwrap();
        assert_eq!(vals.len(), 10);
        assert!(matches!(&vals[9].1,protocol::OscValue::Int(v) if *v==224+17+mode as i32));
    }
}
#[test]
fn logical_high_ids_export_valid_fallback_and_never_enter_wire_geometry() {
    let dir = tempfile::tempdir().unwrap();
    let mut w = Workspace::open(dir.path().into()).unwrap();
    let mut m = material();
    m.id = 65535;
    w.project.materials = vec![m.clone()];
    w.project.scene.boxes = vec![Box3::new([0; 3], [1; 3], m.id)];
    let projected = target::project_wire(&w.project.scene, &[m], Some(&catalog())).unwrap();
    let surface = *projected.surfaces.values().next().unwrap();
    assert_eq!(surface.mode, 1);
    assert!(protocol::mf5_values(
        &Command::Voxel {
            value: projected.scene.boxes[0].clone()
        },
        1,
        0,
        surface
    )
    .is_ok());
    assert!(protocol::validate_box(&projected.scene.boxes[0]).is_err());
    let archive = morphforge_app::catalog::export(&w).unwrap();
    let static_catalog = AvatarCatalog::from_bytes(&archive).unwrap();
    assert_eq!(static_catalog.materials.len(), 1);
    assert_eq!(static_catalog.materials[0].id, 1);
}

#[test]
fn pending_unified_catalog_migration_preserves_project_and_old_physical_size() {
    let dir = tempfile::tempdir().unwrap();
    let w = Workspace::open(dir.path().into()).unwrap();
    let project = serde_json::to_value(&w.project).unwrap();
    let old = include_bytes!("../../../materials/mf4/catalog.json");
    std::fs::write(dir.path().join("avatar-catalog.json"), old).unwrap();
    std::fs::write(
        dir.path().join("pending-avatar-catalog.json"),
        include_bytes!("../../../materials/mf6/catalog.json"),
    )
    .unwrap();
    let old_connection = morphforge_app::transport::Connection {
        profile: "mf4".into(),
        port: 9000,
        scale: 0.05,
        enabled: true,
        ..Default::default()
    };
    std::fs::write(
        dir.path().join("connection.json"),
        serde_json::to_vec(&old_connection).unwrap(),
    )
    .unwrap();
    let t = Transport::new(Arc::new(Mutex::new(w)));
    assert_eq!(t.config().profile, "mf6");
    assert_eq!(t.config().port, 9000);
    assert!(!t.config().enabled);
    assert!((t.config().scale - 0.05).abs() < 1e-7);
    assert!(!dir.path().join("pending-avatar-catalog.json").exists());
    assert_eq!(
        std::fs::read(dir.path().join("avatar-catalog.previous.json")).unwrap(),
        old
    );
    assert_eq!(
        serde_json::to_value(&Workspace::open(dir.path().into()).unwrap().project).unwrap(),
        project
    );
    t.inner.lock().unwrap().stop = true;
}
