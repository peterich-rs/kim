use axum::http::StatusCode;
use axum::routing::post;
use axum::Router;
use kim_sdk::{image_mime_ok, MediaRef, MediaUploader, SdkError, MAX_IMAGE_BYTES};

#[test]
fn rejects_parent_dir_and_relative_path() {
    assert!(kim_sdk::validate_media_path("foo.jpg").is_err());
    assert!(kim_sdk::validate_media_path("/tmp/../etc/passwd").is_err());
    assert!(kim_sdk::validate_media_path("/tmp/a.jpg").is_ok());
}

#[test]
fn mime_allowlist() {
    assert!(image_mime_ok("image/jpeg"));
    assert!(image_mime_ok("image/png"));
    assert!(!image_mime_ok("video/mp4"));
}

#[tokio::test]
async fn payload_too_large() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("big.jpg");
    tokio::fs::write(&path, vec![0u8; (MAX_IMAGE_BYTES + 1) as usize])
        .await
        .expect("write");
    let up = MediaUploader::new("http://127.0.0.1:1").expect("client");
    let err = up
        .upload_image(
            "tok",
            &MediaRef {
                path: path.to_string_lossy().into_owned(),
                mime: "image/jpeg".into(),
                width: 1,
                height: 1,
                byte_size: MAX_IMAGE_BYTES + 1,
            },
        )
        .await
        .expect_err("too large");
    assert!(matches!(err, SdkError::PayloadTooLarge { .. }));
}

#[tokio::test]
async fn unsupported_mime() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a.bin");
    tokio::fs::write(&path, b"xx").await.expect("write");
    let up = MediaUploader::new("http://127.0.0.1:1").expect("client");
    let err = up
        .upload_image(
            "tok",
            &MediaRef {
                path: path.to_string_lossy().into_owned(),
                mime: "video/mp4".into(),
                width: 1,
                height: 1,
                byte_size: 2,
            },
        )
        .await
        .expect_err("mime");
    assert!(matches!(err, SdkError::UnsupportedMedia { .. }));
}

#[tokio::test]
async fn streams_to_worker() {
    let app = Router::new().route("/v1/objects", post(upload));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a.jpg");
    tokio::fs::write(&path, [0xFF, 0xD8, 0xFF, 0xD9])
        .await
        .expect("write");
    let origin = format!("http://{addr}");
    let up = MediaUploader::new(origin).expect("client");
    let url = up
        .upload_image(
            "tok",
            &MediaRef {
                path: path.to_string_lossy().into_owned(),
                mime: "image/jpeg".into(),
                width: 1,
                height: 1,
                byte_size: 4,
            },
        )
        .await
        .expect("upload");
    assert_eq!(url, "https://media.kim.ainexc.com/a.jpg");
}

async fn upload() -> (StatusCode, String) {
    (
        StatusCode::OK,
        r#"{"url":"https://media.kim.ainexc.com/a.jpg"}"#.into(),
    )
}
