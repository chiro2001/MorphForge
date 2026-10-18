use morphforge_app::{
    capture::CaptureBroker, harness::Harness, provider::Provider, store::Workspace,
    transport::Transport,
};
use serde_json::json;
use std::sync::{Arc, Mutex};
#[tokio::test]
async fn large_catalog_is_bounded_searchable_and_paginated_for_the_model() {
    let root = tempfile::tempdir().unwrap();
    let mut w = Workspace::open(root.path().into()).unwrap();
    let base = w.project.materials[0].clone();
    let materials = (0..1000)
        .map(|i| {
            let mut m = base.clone();
            m.id = i + 1;
            m.uid = format!("test:block_{i}");
            m.name = format!("fixture {i}");
            m
        })
        .collect();
    w.update_materials(materials).unwrap();
    let workspace = Arc::new(Mutex::new(w));
    let t = Transport::new(workspace.clone());
    let provider = Provider::new_for_loopback_test(root.path(), "http://127.0.0.1:1").unwrap();
    let h = Harness::new(workspace, provider, CaptureBroker::new(), t.clone());
    let context = h.context();
    assert_eq!(context["styles"].as_array().unwrap().len(), 128);
    assert_eq!(context["catalog"]["total"], 1000);
    assert_eq!(context["catalog"]["nextOffset"], 128);
    let found = h
        .tool("get_context", &json!({"styleQuery":"test:block_999"}), None)
        .await
        .unwrap();
    assert_eq!(found["styles"].as_array().unwrap().len(), 1);
    assert_eq!(found["styles"][0]["id"], 1000);
    let page = h
        .tool(
            "get_context",
            &json!({"styleOffset":128,"styleLimit":8}),
            None,
        )
        .await
        .unwrap();
    assert_eq!(page["styles"].as_array().unwrap().len(), 8);
    assert_eq!(page["styles"][0]["id"], 129);
    t.inner.lock().unwrap().stop = true;
}
