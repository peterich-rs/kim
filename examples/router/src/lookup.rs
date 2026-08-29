use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use kim_naming::Naming;
use serde::Serialize;

use crate::slots::build_slots;

#[derive(Serialize, Clone, Debug)]
pub struct LookupResp {
    pub utc: i64,
    pub location: String,
    pub ws: String,
    pub tcp: String,
}

pub trait IpRegion: Send + Sync {
    fn country(&self, ip: IpAddr) -> String;
}

pub struct StaticIpRegion {
    pub default_location: String,
    pub map: HashMap<IpAddr, String>,
}

impl IpRegion for StaticIpRegion {
    fn country(&self, ip: IpAddr) -> String {
        if ip.is_loopback() {
            return self.default_location.clone();
        }
        self.map
            .get(&ip)
            .cloned()
            .unwrap_or_else(|| self.default_location.clone())
    }
}

#[derive(Clone)]
pub struct Mapping {
    pub country: String,
    pub region: String,
}

#[derive(Clone)]
pub struct Idc {
    pub id: String,
    pub weight: u32,
}

#[derive(Clone)]
pub struct Region {
    pub id: String,
    pub idcs: Vec<Idc>,
}

pub struct Lookup {
    pub naming: Arc<dyn Naming>,
    pub geo: Arc<dyn IpRegion>,
    pub default_region: String,
    pub mapping: Vec<Mapping>,
    pub regions: Vec<Region>,
}

impl Lookup {
    pub async fn lookup(&self, ip: IpAddr, token: &str) -> Result<LookupResp, LookupError> {
        let location = self.geo.country(ip);
        let region = self
            .mapping
            .iter()
            .find(|m| m.country == location)
            .map(|m| m.region.as_str())
            .unwrap_or(self.default_region.as_str());
        let Some(reg) = self.regions.iter().find(|r| r.id == region) else {
            return Err(LookupError::BadConfig);
        };
        let hash_key = if token.is_empty() {
            ip.to_string()
        } else {
            token.to_string()
        };
        let idc = pick_idc(reg, &hash_key).ok_or(LookupError::BadConfig)?;
        match self.pick_gateways(&idc, &hash_key).await {
            Ok(pair) => Ok(LookupResp {
                utc: now_ts(),
                location,
                ws: pair.0,
                tcp: pair.1,
            }),
            Err(LookupError::NoGateway) => {
                let fallback = self
                    .regions
                    .iter()
                    .find(|r| r.id == self.default_region)
                    .and_then(|r| r.idcs.first().map(|i| i.id.clone()));
                let Some(fidc) = fallback else {
                    return Err(LookupError::NoGateway);
                };
                let (ws, tcp) = self.pick_gateways(&fidc, &hash_key).await?;
                Ok(LookupResp {
                    utc: now_ts(),
                    location,
                    ws,
                    tcp,
                })
            }
            Err(e) => Err(e),
        }
    }

    async fn pick_gateways(
        &self,
        idc: &str,
        hash_key: &str,
    ) -> Result<(String, String), LookupError> {
        let tag = format!("IDC:{idc}");
        let ws_list = self
            .naming
            .find("wgateway", &[tag.as_str()])
            .await
            .map_err(|e| LookupError::Other(e.to_string()))?;
        if ws_list.is_empty() {
            return Err(LookupError::NoGateway);
        }
        let tcp_list = self
            .naming
            .find("tgateway", &[tag.as_str()])
            .await
            .map_err(|e| LookupError::Other(e.to_string()))?;
        let ws = pick_one(&ws_list, hash_key);
        let tcp = if tcp_list.is_empty() {
            String::new()
        } else {
            pick_one(&tcp_list, hash_key)
        };
        Ok((ws, tcp))
    }
}

fn pick_idc(region: &Region, key: &str) -> Option<String> {
    let slots = build_slots(&region.idcs.iter().map(|i| i.weight).collect::<Vec<_>>());
    if slots.is_empty() {
        return region.idcs.first().map(|i| i.id.clone());
    }
    let i = crc32fast::hash(key.as_bytes()) as usize % slots.len();
    Some(region.idcs[slots[i]].id.clone())
}

fn pick_one(list: &[kim_naming::DefaultRegistration], key: &str) -> String {
    let i = crc32fast::hash(key.as_bytes()) as usize % list.len();
    let r = &list[i];
    if let Some(d) = r.meta.get("domain") {
        return d.clone();
    }
    if r.protocol == "ws" {
        format!("ws://{}:{}/", r.public_address, r.public_port)
    } else {
        format!("{}:{}", r.public_address, r.public_port)
    }
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[derive(Debug, thiserror::Error)]
pub enum LookupError {
    #[error("no gateway")]
    NoGateway,
    #[error("bad config")]
    BadConfig,
    #[error("{0}")]
    Other(String),
}
