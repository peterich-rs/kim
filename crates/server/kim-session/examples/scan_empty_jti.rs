//! Count `login:loc:v2:*` locations whose `jti` is empty.
//!
//! Exit 0 iff empty_jti, invalid, and wrong_type are all 0. Requires
//! `--features redis` and `REDIS_URL` (or a single argv URL).

#[tokio::main]
async fn main() {
    let url = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("REDIS_URL").ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let Some(url) = url else {
        eprintln!("REDIS_URL or argv[1] required");
        std::process::exit(2);
    };
    let store = match kim_session::RedisSessionStore::open(&url).await {
        Ok(s) => s,
        Err(err) => {
            eprintln!("open redis: {err}");
            std::process::exit(2);
        }
    };
    match store.count_empty_jti_locations().await {
        Ok(scan) => {
            println!(
                "empty_jti={} invalid={} wrong_type={} scanned={}",
                scan.empty_jti, scan.invalid, scan.wrong_type, scan.scanned
            );
            std::process::exit(kim_session::empty_jti_gate_code(scan));
        }
        Err(err) => {
            eprintln!("scan: {err}");
            std::process::exit(2);
        }
    }
}
