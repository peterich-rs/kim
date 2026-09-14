//! Legacy `ToolSet` + `extensions` ↔ `CapabilityRef` projection.

use serde_json::json;

use crate::capability::CapabilityRef;
use crate::profile::{ExtensionSpec, ToolSet};

/// Map legacy bool tools + MCP extensions into capability refs.
pub fn from_legacy(tools: &ToolSet, extensions: &[ExtensionSpec]) -> Vec<CapabilityRef> {
    let mut caps = Vec::new();

    let im = [
        ("send_message", "im.send_message"),
        ("search_contacts", "im.search_contacts"),
        ("search_messages", "im.search_messages"),
        ("get_conversation_context", "im.get_conversation_context"),
        ("read_clipboard", "im.read_clipboard"),
        ("list_profiles", "im.list_profiles"),
    ];
    for (flag, kind) in im {
        let on = match flag {
            "send_message" => tools.send_message,
            "search_contacts" => tools.search_contacts,
            "search_messages" => tools.search_messages,
            "get_conversation_context" => tools.get_conversation_context,
            "read_clipboard" => tools.read_clipboard,
            "list_profiles" => tools.list_profiles,
            _ => false,
        };
        if on {
            caps.push(CapabilityRef {
                kind: kind.into(),
                id: String::new(),
                params: json!({}),
                enabled: true,
            });
        }
    }

    if tools.fs || tools.fs_write {
        caps.push(CapabilityRef {
            kind: "fs".into(),
            id: String::new(),
            params: json!({ "writable": tools.fs_write }),
            enabled: true,
        });
    }

    if tools.bash {
        caps.push(CapabilityRef {
            kind: "bash".into(),
            id: String::new(),
            params: json!({}),
            enabled: true,
        });
    }

    if tools.subagent {
        caps.push(CapabilityRef {
            kind: "subagent".into(),
            id: String::new(),
            params: json!({}),
            enabled: true,
        });
    }

    for ext in extensions {
        let transport = if ext.transport.trim().is_empty() {
            "stdio"
        } else {
            ext.transport.trim()
        };
        caps.push(CapabilityRef {
            kind: "mcp".into(),
            id: format!("mcp:{}", ext.name),
            params: json!({
                "name": ext.name,
                "command": ext.command,
                "transport": transport,
                "url": ext.url,
            }),
            enabled: true,
        });
    }

    caps
}

/// MCP capability refs → `extensions` rows (empty when no MCP caps).
pub(crate) fn extensions_from_capabilities(caps: &[CapabilityRef]) -> Vec<ExtensionSpec> {
    let mut out = Vec::new();
    for cap in caps.iter().filter(|c| c.enabled && c.kind == "mcp") {
        let name = cap
            .params
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if name.is_empty() {
            continue;
        }
        let command = cap
            .params
            .get("command")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let transport = cap
            .params
            .get("transport")
            .and_then(|v| v.as_str())
            .unwrap_or("stdio")
            .trim();
        let url = cap
            .params
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        out.push(ExtensionSpec {
            name: name.to_string(),
            transport: if transport.is_empty() {
                "stdio".into()
            } else {
                transport.to_string()
            },
            command,
            url,
        });
    }
    out
}

impl ToolSet {
    /// Project capabilities back to the legacy bool surface.
    pub fn from_capabilities(caps: &[CapabilityRef]) -> Self {
        let mut tools = ToolSet::default();
        for cap in caps.iter().filter(|c| c.enabled) {
            match cap.kind.as_str() {
                "im.send_message" => tools.send_message = true,
                "im.search_contacts" => tools.search_contacts = true,
                "im.search_messages" => tools.search_messages = true,
                "im.get_conversation_context" => tools.get_conversation_context = true,
                "im.read_clipboard" => tools.read_clipboard = true,
                "im.list_profiles" => tools.list_profiles = true,
                "fs" => {
                    tools.fs = true;
                    if cap
                        .params
                        .get("writable")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                    {
                        tools.fs_write = true;
                    }
                }
                "bash" => tools.bash = true,
                "subagent" => tools.subagent = true,
                _ => {}
            }
        }
        tools
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_default_create_tools() {
        let tools = ToolSet {
            send_message: true,
            read_clipboard: true,
            ..ToolSet::default()
        };
        let caps = from_legacy(&tools, &[]);
        let back = ToolSet::from_capabilities(&caps);
        assert_eq!(back, tools);
    }

    #[test]
    fn fs_write_implies_fs_projection() {
        let tools = ToolSet {
            fs_write: true,
            ..ToolSet::default()
        };
        let caps = from_legacy(&tools, &[]);
        assert_eq!(caps.len(), 1);
        assert_eq!(caps[0].kind, "fs");
        let back = ToolSet::from_capabilities(&caps);
        assert!(back.fs);
        assert!(back.fs_write);
    }

    #[test]
    fn mcp_round_trip_and_clear() {
        let ext = ExtensionSpec {
            name: "gh".into(),
            transport: "stdio".into(),
            command: vec!["uvx".into(), "mcp".into()],
            url: String::new(),
        };
        let caps = from_legacy(&ToolSet::default(), &[ext.clone()]);
        assert_eq!(extensions_from_capabilities(&caps), vec![ext]);
        assert!(extensions_from_capabilities(&[]).is_empty());
        let im_only = from_legacy(
            &ToolSet {
                send_message: true,
                ..ToolSet::default()
            },
            &[],
        );
        assert!(extensions_from_capabilities(&im_only).is_empty());
    }
}
