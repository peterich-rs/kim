use std::time::Duration;

use kim_metrics::KimMetrics;

#[tokio::test]
async fn serve_and_scrape() {
    let m = KimMetrics::new("wg-1", "wgateway").expect("metrics");
    m.on_channel_open();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    drop(listener);
    let registry = m.registry();
    tokio::spawn(async move {
        let _ = kim_metrics::serve(addr, registry).await;
    });
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    let body = loop {
        match reqwest::get(format!("http://{addr}/metrics")).await {
            Ok(resp) => break resp.text().await.expect("text"),
            Err(_) if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            Err(e) => panic!("scrape failed: {e}"),
        }
    };
    assert!(body.contains("kim_channel_total"), "metrics body: {body}");
}

#[tokio::test]
async fn router_oneshot_health() {
    let m = KimMetrics::new("chat-1", "chat").expect("metrics");
    let app = kim_metrics::router(m.registry());
    let req = axum::http::Request::builder()
        .uri("/health")
        .body(axum::body::Body::empty())
        .expect("req");
    let resp = tower::ServiceExt::oneshot(app, req).await.expect("oneshot");
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
}
