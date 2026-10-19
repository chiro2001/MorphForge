use morphforge_app::server::App;
use serde_json::{json, Value};
use std::io::Cursor;
#[tokio::test]
async fn reference_http_upload_list_image_and_remove_are_local_only() {
    let dir = tempfile::tempdir().unwrap();
    let app = App::open(dir.path().into(), None).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app.router()).await.unwrap();
    });
    let client = reqwest::Client::new();
    let library: Value = client
        .get(format!("{base}/api/references"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(library["images"].as_array().unwrap().len() >= 8);
    let mut png = Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(24, 16)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(png.into_inner()).file_name("uploaded-test.png"),
    );
    let response = client
        .post(format!("{base}/api/references/upload"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    let uploaded: Value = response.json().await.unwrap();
    let id = uploaded["id"].as_str().unwrap();
    for url in [
        uploaded["url"].as_str().unwrap(),
        uploaded["previewUrl"].as_str().unwrap(),
    ] {
        let r = client.get(format!("{base}{url}")).send().await.unwrap();
        assert!(r.status().is_success());
        assert!(r.bytes().await.unwrap().len() > 20);
    }
    assert!(client
        .post(format!("{base}/api/references/remove"))
        .json(&json!({"id":id}))
        .send()
        .await
        .unwrap()
        .status()
        .is_success());
    assert_eq!(
        client
            .get(format!("{base}/api/references/{id}/image"))
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::NOT_FOUND
    );
    let bad = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(b"not png".to_vec()).file_name("bad.png"),
    );
    assert_eq!(
        client
            .post(format!("{base}/api/references/upload"))
            .multipart(bad)
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::BAD_REQUEST
    );
    server.abort();
}
