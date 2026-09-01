use std::net::IpAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use kim_naming::{DefaultRegistration, StaticNaming};
use kim_protocol::{generate, generate_with_jti, DEMO_DEFAULT_SECRET};
use router::{app, hash_key, AppState, Idc, Lookup, LookupError, Mapping, Region, StaticIpRegion};
use tower::ServiceExt;

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn jwt_secret() -> String {
    DEMO_DEFAULT_SECRET.to_string()
}

fn reg(
    id: &str,
    name: &str,
    proto: &str,
    port: u16,
    idc: &str,
    domain: &str,
) -> DefaultRegistration {
    let mut meta = std::collections::HashMap::new();
    meta.insert("domain".into(), domain.into());
    DefaultRegistration {
        service_id: id.into(),
        service_name: name.into(),
        protocol: proto.into(),
        public_address: "127.0.0.1".into(),
        public_port: port,
        tags: vec![format!("IDC:{idc}")],
        meta,
    }
}

fn local_lookup(regs: Vec<DefaultRegistration>) -> Lookup {
    Lookup {
        naming: Arc::new(StaticNaming::from_slice(regs)),
        geo: Arc::new(StaticIpRegion {
            default_location: "default".into(),
            map: Default::default(),
        }),
        default_region: "local".into(),
        mapping: vec![Mapping {
            country: "default".into(),
            region: "local".into(),
        }],
        regions: vec![Region {
            id: "local".into(),
            idcs: vec![Idc {
                id: "local".into(),
                weight: 100,
            }],
        }],
        jwt_secret: jwt_secret(),
    }
}

fn router_app(lookup: Lookup) -> axum::Router {
    app(
        AppState {
            lookup: Arc::new(lookup),
        },
        prometheus::Registry::new(),
    )
}

#[tokio::test]
async fn loopback_json_shape() {
    let lookup = local_lookup(vec![
        reg(
            "wg-1",
            "wgateway",
            "ws",
            8001,
            "local",
            "ws://127.0.0.1:8001/",
        ),
        reg("tg-1", "tgateway", "tcp", 8003, "local", "127.0.0.1:8003"),
    ]);
    let resp = lookup
        .lookup(IpAddr::from([127, 0, 0, 1]), "")
        .await
        .expect("lookup");
    assert!(resp.ws.contains("8001"), "{}", resp.ws);
    assert!(resp.tcp.contains("8003"), "{}", resp.tcp);
    assert_eq!(resp.location, "default");
}

#[tokio::test]
async fn empty_wgateway_is_no_gateway() {
    let lookup = local_lookup(vec![]);
    let err = lookup
        .lookup(IpAddr::from([127, 0, 0, 1]), "")
        .await
        .unwrap_err();
    assert!(matches!(err, LookupError::NoGateway));
}

#[tokio::test]
async fn ip_map_hk() {
    let mut map = std::collections::HashMap::new();
    map.insert(IpAddr::from([10, 0, 0, 8]), "hk".into());
    let naming = Arc::new(StaticNaming::from_slice(vec![
        reg("wg-1", "wgateway", "ws", 8001, "hk", "ws://hk:8001/"),
        reg(
            "wg-local",
            "wgateway",
            "ws",
            8001,
            "local",
            "ws://127.0.0.1:8001/",
        ),
    ]));
    let lookup = Lookup {
        naming,
        geo: Arc::new(StaticIpRegion {
            default_location: "default".into(),
            map,
        }),
        default_region: "local".into(),
        mapping: vec![
            Mapping {
                country: "default".into(),
                region: "local".into(),
            },
            Mapping {
                country: "hk".into(),
                region: "hk".into(),
            },
        ],
        regions: vec![
            Region {
                id: "local".into(),
                idcs: vec![Idc {
                    id: "local".into(),
                    weight: 100,
                }],
            },
            Region {
                id: "hk".into(),
                idcs: vec![Idc {
                    id: "hk".into(),
                    weight: 100,
                }],
            },
        ],
        jwt_secret: jwt_secret(),
    };
    let resp = lookup
        .lookup(IpAddr::from([10, 0, 0, 8]), "")
        .await
        .expect("hk");
    assert_eq!(resp.location, "hk");
    assert!(resp.ws.contains("hk"), "{}", resp.ws);
}

#[test]
fn hash_key_uses_acc_jti_not_compact_jwt() {
    let ip = IpAddr::from([127, 0, 0, 1]);
    let exp = now_ts() + 3600;
    let t1 = generate_with_jti(DEMO_DEFAULT_SECRET, "alice", "kim", exp, "reuse-jti").unwrap();
    let t2 = generate_with_jti(DEMO_DEFAULT_SECRET, "alice", "kim", exp + 60, "reuse-jti").unwrap();
    assert_ne!(t1, t2, "renew changes compact JWT");
    let k1 = hash_key(ip, &t1, DEMO_DEFAULT_SECRET).expect("t1");
    let k2 = hash_key(ip, &t2, DEMO_DEFAULT_SECRET).expect("t2");
    assert_eq!(k1, "alice:reuse-jti");
    assert_eq!(k1, k2);
    assert_ne!(k1, t1);
    assert_ne!(k1, t2);
}

#[test]
fn hash_key_legacy_acc_without_jti() {
    let ip = IpAddr::from([10, 0, 0, 1]);
    let token = generate(DEMO_DEFAULT_SECRET, "bob", "kim", now_ts() + 3600).unwrap();
    let key = hash_key(ip, &token, DEMO_DEFAULT_SECRET).expect("legacy");
    assert!(key.starts_with("bob"), "{key}");
    assert_ne!(key, token);
}

#[test]
fn hash_key_rejects_invalid_and_expired() {
    let ip = IpAddr::from([127, 0, 0, 1]);
    assert!(matches!(
        hash_key(ip, "not-a-jwt", DEMO_DEFAULT_SECRET),
        Err(LookupError::Unauthorized)
    ));
    let expired = generate_with_jti(DEMO_DEFAULT_SECRET, "alice", "kim", 1, "dead").unwrap();
    assert!(matches!(
        hash_key(ip, &expired, DEMO_DEFAULT_SECRET),
        Err(LookupError::Unauthorized)
    ));
}

#[test]
fn empty_token_hashes_ip() {
    let ip = IpAddr::from([10, 1, 2, 3]);
    assert_eq!(
        hash_key(ip, "", DEMO_DEFAULT_SECRET).unwrap(),
        ip.to_string()
    );
}

async fn body_json(resp: axum::http::Response<Body>) -> serde_json::Value {
    let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
        .await
        .expect("body");
    serde_json::from_slice(&bytes).expect("json")
}

#[tokio::test]
async fn path_token_is_gone() {
    let lookup = local_lookup(vec![reg(
        "wg-1",
        "wgateway",
        "ws",
        8001,
        "local",
        "ws://127.0.0.1:8001/",
    )]);
    let token = generate_with_jti(
        DEMO_DEFAULT_SECRET,
        "alice",
        "kim",
        now_ts() + 3600,
        "reuse-jti",
    )
    .unwrap();
    let app = router_app(lookup);
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/lookup/{token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        resp.status() == StatusCode::NOT_FOUND || resp.status() == StatusCode::GONE,
        "{}",
        resp.status()
    );
}

#[tokio::test]
async fn authorization_lookup_hashes_acc_not_raw_token() {
    let lookup = local_lookup(vec![
        reg("wg-a", "wgateway", "ws", 8001, "local", "ws://gw-a:8001/"),
        reg("wg-b", "wgateway", "ws", 8002, "local", "ws://gw-b:8002/"),
        reg("tg-1", "tgateway", "tcp", 8003, "local", "127.0.0.1:8003"),
    ]);
    let exp = now_ts() + 3600;
    let t1 = generate_with_jti(DEMO_DEFAULT_SECRET, "alice", "kim", exp, "reuse-jti").unwrap();
    let t2 = generate_with_jti(DEMO_DEFAULT_SECRET, "alice", "kim", exp + 90, "reuse-jti").unwrap();
    assert_ne!(t1, t2);

    let app = router_app(lookup);
    let r1 = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/lookup")
                .header("authorization", format!("Bearer {t1}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r1.status(), StatusCode::OK);
    let j1 = body_json(r1).await;

    let r2 = app
        .oneshot(
            Request::builder()
                .uri("/api/lookup")
                .header("authorization", format!("Bearer {t2}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r2.status(), StatusCode::OK);
    let j2 = body_json(r2).await;
    assert_eq!(j1["ws"], j2["ws"]);
    assert_eq!(j1["tcp"], j2["tcp"]);

    let ip = IpAddr::from([127, 0, 0, 1]);
    assert_eq!(
        hash_key(ip, &t1, DEMO_DEFAULT_SECRET).unwrap(),
        "alice:reuse-jti"
    );
}

#[tokio::test]
async fn invalid_bearer_is_401() {
    let lookup = local_lookup(vec![reg(
        "wg-1",
        "wgateway",
        "ws",
        8001,
        "local",
        "ws://127.0.0.1:8001/",
    )]);
    let app = router_app(lookup);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/lookup")
                .header("authorization", "Bearer not-a-jwt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn expired_bearer_is_401() {
    let lookup = local_lookup(vec![reg(
        "wg-1",
        "wgateway",
        "ws",
        8001,
        "local",
        "ws://127.0.0.1:8001/",
    )]);
    let expired = generate_with_jti(DEMO_DEFAULT_SECRET, "alice", "kim", 1, "dead").unwrap();
    let app = router_app(lookup);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/lookup")
                .header("authorization", format!("Bearer {expired}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
