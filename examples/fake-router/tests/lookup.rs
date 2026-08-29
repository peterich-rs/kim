use std::net::IpAddr;
use std::sync::Arc;

use fake_router::{Idc, Lookup, LookupError, Mapping, Region, StaticIpRegion};
use kim_naming::{DefaultRegistration, StaticNaming};

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

#[tokio::test]
async fn loopback_json_shape() {
    let naming = Arc::new(StaticNaming::from_slice(vec![
        reg(
            "wg-1",
            "wgateway",
            "ws",
            8001,
            "local",
            "ws://127.0.0.1:8001/",
        ),
        reg("tg-1", "tgateway", "tcp", 8003, "local", "127.0.0.1:8003"),
    ]));
    let lookup = Lookup {
        naming,
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
    };
    let resp = lookup
        .lookup(IpAddr::from([127, 0, 0, 1]), "tok")
        .await
        .expect("lookup");
    assert!(resp.ws.contains("8001"), "{}", resp.ws);
    assert!(resp.tcp.contains("8003"), "{}", resp.tcp);
    assert_eq!(resp.location, "default");
}

#[tokio::test]
async fn empty_wgateway_is_no_gateway() {
    let naming = Arc::new(StaticNaming::from_slice(vec![]));
    let lookup = Lookup {
        naming,
        geo: Arc::new(StaticIpRegion {
            default_location: "default".into(),
            map: Default::default(),
        }),
        default_region: "local".into(),
        mapping: vec![],
        regions: vec![Region {
            id: "local".into(),
            idcs: vec![Idc {
                id: "local".into(),
                weight: 100,
            }],
        }],
    };
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
    };
    let resp = lookup
        .lookup(IpAddr::from([10, 0, 0, 8]), "tok")
        .await
        .expect("hk");
    assert_eq!(resp.location, "hk");
    assert!(resp.ws.contains("hk"), "{}", resp.ws);
}
