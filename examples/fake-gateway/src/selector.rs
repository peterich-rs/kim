use std::collections::HashMap;

use kim_container::Selector;
use kim_naming::DefaultRegistration;
use kim_protocol::pkt::Header;
use kim_protocol::{META_ACCOUNT, META_APP};
use serde::Deserialize;

use crate::slots::build_slots;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RouteBy {
    #[default]
    Account,
    App,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ZoneFile {
    pub id: String,
    #[serde(default = "default_weight")]
    pub weight: u32,
}

fn default_weight() -> u32 {
    100
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct RouteFile {
    #[serde(default)]
    pub route_by: String,
    #[serde(default)]
    pub zones: Vec<ZoneFile>,
    #[serde(default)]
    pub whitelist: HashMap<String, String>,
}

pub struct Route {
    pub route_by: RouteBy,
    pub zones: Vec<ZoneFile>,
    pub whitelist: HashMap<String, String>,
    pub slots: Vec<usize>,
}

impl Route {
    pub fn from_config(file: RouteFile) -> Self {
        let route_by = if file.route_by.eq_ignore_ascii_case("app") {
            RouteBy::App
        } else {
            RouteBy::Account
        };
        let slots = build_slots(&file.zones.iter().map(|z| z.weight).collect::<Vec<_>>());
        Self {
            route_by,
            zones: file.zones,
            whitelist: file.whitelist,
            slots,
        }
    }
}

pub struct RouteSelector {
    route: Route,
}

impl RouteSelector {
    pub fn new(route: Route) -> Self {
        Self { route }
    }
}

fn header_meta(header: &Header, key: &str) -> String {
    header
        .meta
        .iter()
        .find(|m| m.key == key)
        .map(|m| m.value.clone())
        .unwrap_or_default()
}

fn hash_pick(key: &str, srvs: &[DefaultRegistration]) -> Option<String> {
    if srvs.is_empty() {
        return None;
    }
    let i = crc32fast::hash(key.as_bytes()) as usize % srvs.len();
    Some(srvs[i].service_id.clone())
}

fn in_zone(reg: &DefaultRegistration, zone: &str) -> bool {
    reg.meta.get("zone").is_some_and(|z| z == zone)
        || reg.tags.iter().any(|t| t == &format!("zone:{zone}"))
}

impl Selector for RouteSelector {
    fn lookup(&self, header: &Header, srvs: &[DefaultRegistration]) -> Option<String> {
        if srvs.is_empty() {
            return None;
        }
        let app = header_meta(header, META_APP);
        let account = header_meta(header, META_ACCOUNT);
        if app.is_empty() && account.is_empty() {
            return hash_pick(&header.channel_id, srvs);
        }
        let zone_id = if let Some(z) = self.route.whitelist.get(&app) {
            z.clone()
        } else if self.route.slots.is_empty() || self.route.zones.is_empty() {
            return hash_pick(&header.channel_id, srvs);
        } else {
            let key = match self.route.route_by {
                RouteBy::App => &app,
                RouteBy::Account => {
                    if account.is_empty() {
                        &header.channel_id
                    } else {
                        &account
                    }
                }
            };
            let slot = crc32fast::hash(key.as_bytes()) as usize % self.route.slots.len();
            let zi = self.route.slots[slot];
            self.route.zones[zi].id.clone()
        };
        let zone_srvs: Vec<DefaultRegistration> = srvs
            .iter()
            .filter(|r| in_zone(r, &zone_id))
            .cloned()
            .collect();
        let pool = if zone_srvs.is_empty() {
            srvs
        } else {
            &zone_srvs
        };
        let pick_key = if account.is_empty() {
            &header.channel_id
        } else {
            &account
        };
        hash_pick(pick_key, pool)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(id: &str, zone: &str) -> DefaultRegistration {
        let mut meta = HashMap::new();
        meta.insert("zone".into(), zone.into());
        DefaultRegistration {
            service_id: id.into(),
            service_name: "chat".into(),
            protocol: "tcp".into(),
            public_address: "127.0.0.1".into(),
            public_port: 1,
            tags: vec![format!("zone:{zone}")],
            meta,
        }
    }

    #[test]
    fn never_empty_when_adult_exists() {
        let route = Route::from_config(RouteFile {
            route_by: "app".into(),
            zones: vec![ZoneFile {
                id: "z1".into(),
                weight: 100,
            }],
            whitelist: HashMap::new(),
        });
        let sel = RouteSelector::new(route);
        let header = Header {
            channel_id: "c".into(),
            meta: vec![
                kim_protocol::pkt::Meta {
                    key: META_APP.into(),
                    value: "kim".into(),
                },
                kim_protocol::pkt::Meta {
                    key: META_ACCOUNT.into(),
                    value: "alice".into(),
                },
            ],
            ..Header::default()
        };
        let srvs = vec![r("chat-2", "other")];
        assert_eq!(sel.lookup(&header, &srvs).as_deref(), Some("chat-2"));
    }

    #[test]
    fn empty_slots_hash_pick() {
        let route = Route::from_config(RouteFile {
            route_by: "app".into(),
            zones: vec![ZoneFile {
                id: "z1".into(),
                weight: 0,
            }],
            whitelist: HashMap::new(),
        });
        let sel = RouteSelector::new(route);
        let header = Header {
            channel_id: "c".into(),
            meta: vec![kim_protocol::pkt::Meta {
                key: META_APP.into(),
                value: "kim".into(),
            }],
            ..Header::default()
        };
        let srvs = vec![r("a", "z1"), r("b", "z1")];
        assert!(sel.lookup(&header, &srvs).is_some());
    }

    #[test]
    fn whitelist_hits_zone() {
        let mut whitelist = HashMap::new();
        whitelist.insert("kim-gray".into(), "zone_gray".into());
        let route = Route::from_config(RouteFile {
            route_by: "app".into(),
            zones: vec![
                ZoneFile {
                    id: "zone_local".into(),
                    weight: 100,
                },
                ZoneFile {
                    id: "zone_gray".into(),
                    weight: 0,
                },
            ],
            whitelist,
        });
        let sel = RouteSelector::new(route);
        let header = Header {
            channel_id: "c".into(),
            meta: vec![
                kim_protocol::pkt::Meta {
                    key: META_APP.into(),
                    value: "kim-gray".into(),
                },
                kim_protocol::pkt::Meta {
                    key: META_ACCOUNT.into(),
                    value: "alice".into(),
                },
            ],
            ..Header::default()
        };
        let srvs = vec![r("chat-1", "zone_local"), r("chat-2", "zone_gray")];
        assert_eq!(sel.lookup(&header, &srvs).as_deref(), Some("chat-2"));
    }
}
