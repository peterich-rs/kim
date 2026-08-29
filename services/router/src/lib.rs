mod lookup;
mod slots;

pub use lookup::{Idc, IpRegion, Lookup, LookupError, LookupResp, Mapping, Region, StaticIpRegion};
pub use slots::build_slots;

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use axum::{Json, Router};
use kim_naming::{open_naming, DefaultRegistration};
use serde::Deserialize;

#[derive(Clone)]
pub struct AppState {
    pub lookup: Arc<Lookup>,
}

#[derive(Deserialize)]
struct LookupQuery {
    token: Option<String>,
    ip: Option<String>,
}

pub fn app(state: AppState, registry: prometheus::Registry) -> Router {
    let api = Router::new()
        .route("/api/lookup", get(lookup_q))
        .route("/api/lookup/{token}", get(lookup_path))
        .with_state(state);
    api.merge(kim_metrics::router(registry))
}

async fn lookup_q(
    State(st): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<LookupQuery>,
) -> Result<Json<LookupResp>, StatusCode> {
    handle(&st, headers, q.token, q.ip).await
}

async fn lookup_path(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(token): Path<String>,
    Query(q): Query<LookupQuery>,
) -> Result<Json<LookupResp>, StatusCode> {
    handle(&st, headers, Some(token), q.ip).await
}

async fn handle(
    st: &AppState,
    headers: HeaderMap,
    token: Option<String>,
    ip_q: Option<String>,
) -> Result<Json<LookupResp>, StatusCode> {
    let token = token.or_else(|| bearer(&headers)).unwrap_or_default();
    let ip = ip_q
        .and_then(|s| s.parse::<IpAddr>().ok())
        .or_else(|| xff(&headers))
        .unwrap_or(IpAddr::from([127, 0, 0, 1]));
    match st.lookup.lookup(ip, &token).await {
        Ok(resp) => Ok(Json(resp)),
        Err(LookupError::NoGateway) => Err(StatusCode::SERVICE_UNAVAILABLE),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

fn bearer(h: &HeaderMap) -> Option<String> {
    let v = h.get(axum::http::header::AUTHORIZATION)?.to_str().ok()?;
    v.strip_prefix("Bearer ").map(str::to_string)
}

fn xff(h: &HeaderMap) -> Option<IpAddr> {
    let v = h.get("x-forwarded-for")?.to_str().ok()?;
    v.split(',').next()?.trim().parse().ok()
}

#[derive(Deserialize)]
struct File {
    #[serde(rename = "self")]
    this: SelfSection,
    #[serde(default)]
    ip_map: Vec<IpRow>,
    #[serde(default)]
    mapping: Vec<MapRow>,
    #[serde(default)]
    regions: Vec<RegionRow>,
    #[serde(default)]
    services: Vec<SvcRow>,
}

#[derive(Deserialize)]
struct SelfSection {
    listen: String,
    default_location: String,
    default_region: String,
    #[serde(default)]
    consul_url: String,
}

#[derive(Deserialize)]
struct IpRow {
    ip: String,
    country: String,
}

#[derive(Deserialize)]
struct MapRow {
    country: String,
    region: String,
}

#[derive(Deserialize)]
struct RegionRow {
    id: String,
    #[serde(default)]
    idcs: Vec<IdcRow>,
}

#[derive(Deserialize)]
struct IdcRow {
    id: String,
    #[serde(default = "w100")]
    weight: u32,
}

fn w100() -> u32 {
    100
}

#[derive(Deserialize)]
struct SvcRow {
    service_id: String,
    service_name: String,
    protocol: String,
    public_address: String,
    public_port: u16,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    domain: String,
}

pub fn load(path: &std::path::Path) -> Result<(String, AppState), Box<dyn std::error::Error>> {
    let cfg: File = toml::from_str(&std::fs::read_to_string(path)?)?;
    let mut map = HashMap::new();
    for row in cfg.ip_map {
        if let Ok(ip) = row.ip.parse::<IpAddr>() {
            map.insert(ip, row.country);
        }
    }
    let geo = Arc::new(StaticIpRegion {
        default_location: cfg.this.default_location,
        map,
    });
    let regs: Vec<DefaultRegistration> = cfg
        .services
        .into_iter()
        .map(|s| {
            let mut meta = HashMap::new();
            if !s.domain.is_empty() {
                meta.insert("domain".into(), s.domain);
            }
            DefaultRegistration {
                service_id: s.service_id,
                service_name: s.service_name,
                protocol: s.protocol,
                public_address: s.public_address,
                public_port: s.public_port,
                tags: s.tags,
                meta,
            }
        })
        .collect();
    let consul = std::env::var("CONSUL_HTTP_ADDR")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            let t = cfg.this.consul_url.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        });
    let naming = if consul.is_some() {
        open_naming(consul.as_deref(), vec![])?
    } else {
        open_naming(None, regs)?
    };
    let lookup = Lookup {
        naming,
        geo,
        default_region: cfg.this.default_region,
        mapping: cfg
            .mapping
            .into_iter()
            .map(|m| Mapping {
                country: m.country,
                region: m.region,
            })
            .collect(),
        regions: cfg
            .regions
            .into_iter()
            .map(|r| Region {
                id: r.id,
                idcs: r
                    .idcs
                    .into_iter()
                    .map(|i| Idc {
                        id: i.id,
                        weight: i.weight,
                    })
                    .collect(),
            })
            .collect(),
    };
    Ok((
        cfg.this.listen,
        AppState {
            lookup: Arc::new(lookup),
        },
    ))
}

pub fn app_from_state(state: AppState) -> Result<Router, Box<dyn std::error::Error>> {
    let metrics = kim_metrics::KimMetrics::new("router-1", "router")?;
    Ok(app(state, metrics.registry()))
}
