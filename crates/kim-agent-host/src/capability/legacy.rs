//! Legacy `ToolSet` + `extensions` ↔ `CapabilityRef` projection.
//! The projection itself lives in `kim-agent-model`. These wrappers keep the host module path.

use kim_agent_model::{CapabilityRef, ExtensionSpec, ToolSet};

pub fn from_legacy(tools: &ToolSet, extensions: &[ExtensionSpec]) -> Vec<CapabilityRef> {
    CapabilityRef::from_legacy(tools, extensions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kim_agent_model::extensions_from_capabilities;

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
            env: Default::default(),
        };
        let caps = from_legacy(&ToolSet::default(), std::slice::from_ref(&ext));
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
