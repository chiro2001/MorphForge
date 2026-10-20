use axum::{routing::get, Json, Router};
use morphforge_app::{avatar_discovery, server::App, target::AvatarCatalog};
use serde_json::{json, Value};
fn tree(c: &AvatarCatalog) -> Value {
    let hash = c.identity_hash();
    let mut content = serde_json::Map::new();
    for (name, value) in [
        ("MF_InfoVersion", 7),
        ("MF_InfoFeatures", 7),
        ("MF_InfoBuild0", 17),
        ("MF_InfoBuild1", 3),
        ("MF_InfoBuild2", 1),
    ] {
        content.insert(name.into(), json!({"VALUE":[value]}));
    }
    for i in 0..32 {
        let v = u8::from_str_radix(&hash[i * 2..i * 2 + 2], 16).unwrap();
        content.insert(format!("MF_Catalog{i:02}"), json!({"VALUE":[v]}));
    }
    json!({"CONTENTS":content})
}
#[tokio::test]
async fn query_identifies_full_hash_and_applies_only_known_catalog_without_udp() {
    let root = tempfile::tempdir().unwrap();
    let app = App::open(root.path().into(), Some(root.path().join("no-key"))).unwrap();
    let c =
        AvatarCatalog::from_bytes(include_bytes!("../../../materials/mf6/catalog.json")).unwrap();
    let expected = c.identity_hash();
    avatar_discovery::cache(root.path(), &c).unwrap();
    let response = tree(&c);
    let router = Router::new()
        .route(
            "/",
            get(|| async { Json(json!({"OSC_TRANSPORT":"UDP","EXTENSIONS":{"VALUE":true}})) }),
        )
        .route(
            "/avatar/parameters",
            get(move || {
                let v = response.clone();
                async move { Json(v) }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let query_task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let router = app.router();
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let r: Value = reqwest::Client::new()
        .post(format!("{base}/api/vrc/discover"))
        .json(&json!({"port":port}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(r["info"]["catalogHash"], expected);
    assert_eq!(r["catalogKnown"], true);
    assert_eq!(r["applied"], true);
    assert_eq!(r["info"]["executionAck"], false);
    assert!(!app.transport.config().enabled);
    assert_eq!(app.transport.inner.lock().unwrap().sent_count, 0);
    let selected = app
        .transport
        .inner
        .lock()
        .unwrap()
        .avatar_catalog
        .clone()
        .unwrap();
    assert_eq!(selected.identity_hash(), expected);
    assert!(selected.capabilities.contains(&"mf6-region8/1".into()));
    // Cache serialization changes whitespace/metadata but never the binary identity.
    assert_eq!(
        avatar_discovery::lookup(root.path(), &expected)
            .unwrap()
            .unwrap()
            .identity_hash(),
        expected
    );
    let mut unknown = selected.clone();
    unknown.materials[0].color[0] = 0.123;
    assert_ne!(unknown.identity_hash(), expected);
    assert!(
        avatar_discovery::lookup(root.path(), &unknown.identity_hash())
            .unwrap()
            .is_none()
    );
    let mut invalid = tree(&c);
    invalid["CONTENTS"]["MF_Catalog31"]["VALUE"][0] = json!(256);
    assert!(avatar_discovery::parse(&invalid).is_err());
    let client = reqwest::Client::new();
    let before = app.workspace.lock().unwrap().project.materials.len();
    let missing = client
        .post(format!("{base}/api/vrc/materials"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert!(missing.status().is_client_error());
    assert_eq!(
        app.workspace.lock().unwrap().project.materials.len(),
        before,
        "missing image import is atomic"
    );
    let mut plain = morphforge_core::materials::builtins().remove(0);
    plain.uid = "fixture:avatar-gold".into();
    plain.color = [1., 0.7, 0.1];
    plain.metallic = 0.5;
    let c=AvatarCatalog::from_bytes(&serde_json::to_vec(&json!({"format":"MorphForgeMaterialCatalog/1","materials":[plain],"capabilities":["mf6-unified/1","mf5-runtime-materials/1"]})).unwrap()).unwrap();
    app.transport.set_catalog(Some(c), root.path()).unwrap();
    let added: Value = client
        .post(format!("{base}/api/vrc/materials"))
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(added["materialsAdded"], 1);
    assert_eq!(added["geometryChanged"], false);
    assert!(app.workspace.lock().unwrap().project.scene.boxes.is_empty());
    let again: Value = client
        .post(format!("{base}/api/vrc/materials"))
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(again["materialsAdded"], 0);
    app.transport.inner.lock().unwrap().stop = true;
    task.abort();
    query_task.abort();
}

#[test]
fn texture_build_options_participate_in_identity_and_survive_cache() {
    let bytes = include_bytes!("../../../materials/mf6/catalog.json");
    let mut source: Value = serde_json::from_slice(bytes).unwrap();
    let plain = AvatarCatalog::from_bytes(bytes).unwrap();
    source["textureOptions"] = json!({"cellPixels":16,"layeredOnly":true});
    let compact = AvatarCatalog::from_bytes(&serde_json::to_vec(&source).unwrap()).unwrap();
    assert_eq!(compact.texture_options.as_ref().unwrap()["cellPixels"], 16);
    assert_ne!(plain.identity_hash(), compact.identity_hash());
    let root = tempfile::tempdir().unwrap();
    avatar_discovery::cache(root.path(), &compact).unwrap();
    assert_eq!(
        avatar_discovery::lookup(root.path(), &compact.identity_hash())
            .unwrap()
            .unwrap()
            .identity_hash(),
        compact.identity_hash()
    );
}
