use morphforge_app::{
    minecraft::{formats, imports},
    server::App,
    store::Workspace,
};
use morphforge_core::{Box3, Operation};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
// Java structure NBT with two adjacent stones. Independent byte-level fixture.
fn structure() -> Vec<u8> {
    fn str_(b: &mut Vec<u8>, s: &str) {
        b.extend((s.len() as u16).to_be_bytes());
        b.extend(s.as_bytes());
    }
    fn header(b: &mut Vec<u8>, t: u8, s: &str) {
        b.push(t);
        str_(b, s);
    }
    fn ints(b: &mut Vec<u8>, s: &str, v: &[i32]) {
        header(b, 9, s);
        b.push(3);
        b.extend((v.len() as i32).to_be_bytes());
        for i in v {
            b.extend(i.to_be_bytes());
        }
    }
    let mut b = vec![];
    header(&mut b, 10, "");
    ints(&mut b, "size", &[2, 1, 1]);
    header(&mut b, 9, "palette");
    b.push(10);
    b.extend(1i32.to_be_bytes());
    header(&mut b, 8, "Name");
    str_(&mut b, "minecraft:stone");
    b.push(0);
    header(&mut b, 9, "blocks");
    b.push(10);
    b.extend(2i32.to_be_bytes());
    for x in 0..2 {
        ints(&mut b, "pos", &[x, 0, 0]);
        header(&mut b, 3, "state");
        b.extend(0i32.to_be_bytes());
        b.push(0);
    }
    b.push(0);
    b
}
#[tokio::test]
async fn preview_guards_protection_idempotence_restart_corruption_and_undo() {
    let dir = tempfile::tempdir().unwrap();
    let mut w = Workspace::open(dir.path().into()).unwrap();
    let bytes = structure();
    let source = imports::register(
        &mut w,
        &bytes,
        "two.nbt",
        &formats::parse(&bytes, "two.nbt").unwrap(),
    )
    .unwrap()["source"]["id"]
        .clone();
    let ws = Arc::new(Mutex::new(w));
    let before = serde_json::to_value(&ws.lock().unwrap().project).unwrap();
    let p = imports::preview(ws.clone(), json!({"sourceId":source}), None)
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&ws.lock().unwrap().project).unwrap(),
        before
    );
    let request = json!({"previewId":p["previewId"],"baseRevision":0,"requestId":"import-1"});
    let first = imports::apply(&mut ws.lock().unwrap(), &request, None).unwrap();
    assert_eq!(first["revision"], 1);
    {
        let w = ws.lock().unwrap();
        assert_eq!(w.project.scene.boxes.len(), 1);
        assert_eq!(w.project.scene.boxes[0].size, [8, 4, 4]);
    }
    drop(ws);
    let mut w = Workspace::open(dir.path().into()).unwrap();
    std::fs::remove_file(
        dir.path()
            .join("imports/previews")
            .join(format!("{}.json.gz", p["previewId"].as_str().unwrap())),
    )
    .unwrap();
    assert_eq!(
        imports::apply(&mut w, &request, None).unwrap()["replayed"],
        true
    );
    w.undo(false, Some(0)).unwrap();
    assert!(w.project.scene.boxes.is_empty());
    let ws = Arc::new(Mutex::new(w));
    let p = imports::preview(ws.clone(), json!({"sourceId":source}), None)
        .await
        .unwrap();
    let base = ws.lock().unwrap().project.scene.revision;
    ws.lock()
        .unwrap()
        .apply(
            &[Operation::Box {
                value: Box3::new([20, 0, 0], [1, 1, 1], 1),
            }],
            Some(base),
            None,
            None,
        )
        .unwrap();
    assert!(imports::apply(
        &mut ws.lock().unwrap(),
        &json!({"previewId":p["previewId"],"baseRevision":base}),
        None
    )
    .unwrap_err()
    .to_string()
    .contains("REVISION_CONFLICT"));
    std::fs::write(
        dir.path()
            .join("imports")
            .join(source.as_str().unwrap())
            .join("source.bin"),
        b"broken",
    )
    .unwrap();
    assert!(imports::inspect(dir.path(), &json!({"sourceId":source}))
        .unwrap_err()
        .to_string()
        .contains("MC_SOURCE_CORRUPT"));
}
#[tokio::test]
async fn http_upload_preview_harness_apply_original_and_invalid_upload() {
    let dir = tempfile::tempdir().unwrap();
    let app = App::open(dir.path().into(), Some(dir.path().join("no-key.env"))).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let router = app.router();
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let c = reqwest::Client::new();
    let bytes = structure();
    let part = reqwest::multipart::Part::bytes(bytes.clone()).file_name("two.nbt");
    let response = c
        .post(format!("{base}/api/minecraft/upload"))
        .multipart(reqwest::multipart::Form::new().part("file", part))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    let source = response.json::<Value>().await.unwrap()["source"]["id"].clone();
    let preview = app
        .harness
        .tool(
            "minecraft_import",
            &json!({"action":"preview","sourceId":source}),
            None,
        )
        .await
        .unwrap();
    let scene = c
        .get(format!("{base}{}", preview["sceneUrl"].as_str().unwrap()))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(scene["scene"]["boxes"].as_array().unwrap().len(), 1);
    let result = app
        .harness
        .tool(
            "minecraft_import",
            &json!({"action":"apply","previewId":preview["previewId"],"baseRevision":0}),
            None,
        )
        .await
        .unwrap();
    assert_eq!(result["revision"], 1);
    let original = c
        .get(format!(
            "{base}/api/minecraft/source/{}",
            source.as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert_eq!(original.as_ref(), bytes.as_slice());
    let part = reqwest::multipart::Part::bytes(vec![10, 0, 0, 9]).file_name("broken.nbt");
    assert!(!c
        .post(format!("{base}/api/minecraft/upload"))
        .multipart(reqwest::multipart::Form::new().part("file", part))
        .send()
        .await
        .unwrap()
        .status()
        .is_success());
    assert!(!app.transport.config().enabled);
    server.abort();
}
#[tokio::test]
async fn protected_import_is_rejected_and_slopes_keep_atomic_edit_semantics() {
    let dir = tempfile::tempdir().unwrap();
    let mut w = Workspace::open(dir.path().into()).unwrap();
    morphforge_app::mesh_tools::execute(&mut w,&json!({"action":"wedge","primitive":"voxel","baseRevision":0,"min":[0,0,0],"size":[4,4,4],"direction":"+x","styleId":1,"group":"roof"}),None).unwrap();
    assert_eq!(w.project.scene.boxes[0].shape, 1);
    assert_eq!(w.project.scene.format, "MorphForgeScene/2");
    let slope_id = w.project.scene.boxes[0].id;
    w.apply(
        &[Operation::Transform {
            ids: vec![slope_id],
            offset: [0; 3],
            turns: 1,
            mirror: None,
            copy: false,
        }],
        Some(1),
        None,
        None,
    )
    .unwrap();
    assert_eq!(w.project.scene.boxes[0].shape, 3);
    w.set_protected_groups(vec!["roof".into()]).unwrap();
    let bytes = structure();
    let source = imports::register(
        &mut w,
        &bytes,
        "two.nbt",
        &formats::parse(&bytes, "two.nbt").unwrap(),
    )
    .unwrap()["source"]["id"]
        .clone();
    let ws = Arc::new(Mutex::new(w));
    let p = imports::preview(ws.clone(), json!({"sourceId":source}), None)
        .await
        .unwrap();
    let mut w = ws.lock().unwrap();
    let before = serde_json::to_value(&w.project).unwrap();
    let base = w.project.scene.revision;
    assert!(imports::apply(
        &mut w,
        &json!({"previewId":p["previewId"],"baseRevision":base}),
        None
    )
    .is_err());
    assert_eq!(serde_json::to_value(&w.project).unwrap(), before);
    w.set_protected_groups(vec![]).unwrap();
    let base = w.project.scene.revision;
    w.apply(
        &[Operation::Erase {
            min: [-1, 1, 0],
            size: [1, 1, 1],
        }],
        Some(base),
        None,
        None,
    )
    .unwrap();
    assert!(
        w.project.scene.boxes.is_empty(),
        "partial erase removes the entire slope template"
    );
}
