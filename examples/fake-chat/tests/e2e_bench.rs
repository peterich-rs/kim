mod harness;

use std::time::Duration;

use harness::*;
use kimbench::{run_login, BenchOpts};

#[tokio::test]
async fn four_logins_status_zero() {
    let stack = spawn_stack().await;
    let url = ws_url(stack.gw_addr);
    let opts = BenchOpts {
        address: url,
        count: 4,
        threads: 2,
        timeout: Duration::from_secs(5),
        ..BenchOpts::default()
    };
    let stats = run_login(opts).await.expect("bench login");
    assert_eq!(stats.status_counts().get(&0).copied(), Some(4));
    assert!(stats.summary(Duration::from_secs(1)).rps >= 0.0);
    let _ = stack.gw.shutdown().await;
    let _ = stack.chat.shutdown().await;
}
