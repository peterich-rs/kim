use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use goose_agent::operation::{
    applied, not_applicable, yielded, yielded_with, Operation, OperationResult,
};
use goose_provider_types::conversation::message::{ActionRequiredData, Message, MessageContent};
use goose_provider_types::conversation::Conversation;
use goose_provider_types::goose_mode::GooseMode;
use goose_provider_types::permission::Permission;
use rmcp::model::{CallToolResult, ContentBlock, JsonObject};

use crate::events::{HostEffect, PendingYield, YieldKind};
use crate::ops::unanswered_tool_requests;
use crate::profile::{PermissionConfig, PermissionDefault};
use crate::HostSession;

pub struct PermissionOp {
    pub mode: GooseMode,
    pub config: PermissionConfig,
}

fn force_ask(name: &str) -> bool {
    name == "bash" || name.contains("__")
}

impl PermissionConfig {
    pub fn default_for(name: &str, mode: GooseMode) -> PermissionDefault {
        match mode {
            GooseMode::Auto => PermissionDefault::AlwaysAllow,
            GooseMode::Approve | GooseMode::Chat => PermissionDefault::AskBefore,
            GooseMode::SmartApprove => match name {
                "send_message" | "read_clipboard" | "write_file" | "bash" => {
                    PermissionDefault::AskBefore
                }
                n if n.contains("__") => PermissionDefault::AskBefore,
                _ => PermissionDefault::AlwaysAllow,
            },
        }
    }

    pub fn resolve(&self, name: &str, mode: GooseMode) -> PermissionDefault {
        if force_ask(name) {
            if self.tools.get(name) == Some(&PermissionDefault::NeverAllow) {
                return PermissionDefault::NeverAllow;
            }
            return PermissionDefault::AskBefore;
        }
        self.tools
            .get(name)
            .copied()
            .unwrap_or_else(|| Self::default_for(name, mode))
    }
}

pub fn parse_permission(raw: &str) -> Result<Permission, String> {
    match raw.trim() {
        "always_allow" => Ok(Permission::AlwaysAllow),
        "allow_once" => Ok(Permission::AllowOnce),
        "deny_once" => Ok(Permission::DenyOnce),
        "always_deny" => Ok(Permission::AlwaysDeny),
        "cancel" => Ok(Permission::Cancel),
        other => Err(format!("unknown permission {other}")),
    }
}

struct ConfirmationMaps<'a> {
    responses: HashMap<&'a str, &'a Permission>,
    asked: HashMap<&'a str, bool>,
    always_allow: Vec<&'a str>,
    always_deny: Vec<&'a str>,
}

fn confirmation_maps(conversation: &Conversation) -> ConfirmationMaps<'_> {
    let mut responses = HashMap::new();
    let mut asked = HashMap::new();
    let mut id_to_name: HashMap<&str, &str> = HashMap::new();
    for message in conversation.messages() {
        for block in &message.content {
            let Some(ar) = block.as_action_required() else {
                continue;
            };
            match &ar.data {
                ActionRequiredData::ToolConfirmation { id, tool_name, .. } => {
                    asked.insert(id.as_str(), true);
                    id_to_name.insert(id.as_str(), tool_name.as_str());
                }
                ActionRequiredData::ToolConfirmationResponse { id, permission } => {
                    responses.insert(id.as_str(), permission);
                }
                _ => {}
            }
        }
    }
    let mut always_allow = Vec::new();
    let mut always_deny = Vec::new();
    for (id, perm) in &responses {
        let Some(name) = id_to_name.get(id) else {
            continue;
        };
        match perm {
            Permission::AlwaysAllow => always_allow.push(*name),
            Permission::AlwaysDeny => always_deny.push(*name),
            _ => {}
        }
    }
    ConfirmationMaps {
        responses,
        asked,
        always_allow,
        always_deny,
    }
}

fn policy_for(
    config: &PermissionConfig,
    mode: GooseMode,
    name: &str,
    maps: &ConfirmationMaps<'_>,
) -> PermissionDefault {
    let configured = config.resolve(name, mode);
    if configured != PermissionDefault::AskBefore {
        return configured;
    }
    if maps.always_deny.contains(&name) {
        return PermissionDefault::NeverAllow;
    }
    if maps.always_allow.contains(&name) {
        return PermissionDefault::AlwaysAllow;
    }
    configured
}

fn deny_response(id: String) -> Message {
    Message::user().with_tool_response(
        id,
        Ok(CallToolResult::error(vec![ContentBlock::text(
            "user-rejected",
        )])),
    )
}

fn arguments_of(call: &rmcp::model::CallToolRequestParams) -> JsonObject {
    call.arguments.clone().unwrap_or_default()
}

pub fn unanswered_confirmations(conversation: &Conversation) -> Vec<PendingYield> {
    let maps = confirmation_maps(conversation);
    let mut out = Vec::new();
    for message in conversation.messages() {
        for block in &message.content {
            let Some(ar) = block.as_action_required() else {
                continue;
            };
            let ActionRequiredData::ToolConfirmation {
                id,
                tool_name,
                arguments,
                prompt,
            } = &ar.data
            else {
                continue;
            };
            if maps.responses.contains_key(id.as_str()) {
                continue;
            }
            let arguments_json = serde_json::to_string(arguments).unwrap_or_else(|_| "{}".into());
            out.push(PendingYield {
                call_id: id.clone(),
                name: tool_name.clone(),
                arguments_json,
                kind: YieldKind::ActionRequired,
                prompt: prompt.clone().unwrap_or_default(),
            });
        }
    }
    out
}

pub fn confirmation_response_message(call_id: &str, permission: Permission) -> Message {
    Message::user()
        .with_content(MessageContent::action_required_tool_confirmation_response(
            call_id.to_string(),
            permission,
        ))
        // Must not be user-visible: a visible user message would become kickoff.
        .with_visibility(false, false)
}

#[async_trait]
impl Operation<HostSession, HostEffect> for PermissionOp {
    fn name(&self) -> &'static str {
        "permission"
    }

    async fn run(
        &self,
        _session: &HostSession,
        conversation: &Conversation,
        _emit: &goose_agent::operation::Emitter,
    ) -> Result<OperationResult<HostEffect>> {
        let pending = unanswered_tool_requests(conversation);
        if pending.is_empty() {
            return not_applicable();
        }
        let maps = confirmation_maps(conversation);
        let mut deny_messages = Vec::new();
        let mut ask_blocks: Vec<(String, String, JsonObject)> = Vec::new();
        let mut need_yield = false;

        for req in pending {
            let Ok(call) = req.tool_call.as_ref() else {
                continue;
            };
            let name = call.name.as_ref();
            if let Some(perm) = maps.responses.get(req.id.as_str()) {
                match perm {
                    Permission::AllowOnce | Permission::AlwaysAllow => continue,
                    Permission::DenyOnce | Permission::AlwaysDeny | Permission::Cancel => {
                        deny_messages.push(deny_response(req.id));
                    }
                }
                continue;
            }
            match policy_for(&self.config, self.mode, name, &maps) {
                PermissionDefault::AlwaysAllow => {}
                PermissionDefault::NeverAllow => {
                    deny_messages.push(deny_response(req.id));
                }
                PermissionDefault::AskBefore => {
                    need_yield = true;
                    if !maps.asked.contains_key(req.id.as_str()) {
                        ask_blocks.push((req.id, name.to_string(), arguments_of(call)));
                    }
                }
            }
        }

        if !deny_messages.is_empty() && !need_yield {
            return applied(deny_messages.into_iter().map(HostEffect::from));
        }
        if need_yield {
            let mut effects: Vec<HostEffect> =
                deny_messages.into_iter().map(HostEffect::from).collect();
            if !ask_blocks.is_empty() {
                let mut message = Message::assistant();
                for (id, name, arguments) in ask_blocks {
                    let prompt = format!("Allow {name}?");
                    message = message.with_action_required(id, name, arguments, Some(prompt));
                }
                message = message.with_visibility(true, false);
                effects.push(HostEffect::from(message));
                return yielded_with(effects);
            }
            if effects.is_empty() {
                return yielded();
            }
            return yielded_with(effects);
        }
        not_applicable()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use goose_provider_types::goose_mode::GooseMode;

    #[test]
    fn smart_approve_write_tools_ask() {
        assert_eq!(
            PermissionConfig::default_for("send_message", GooseMode::SmartApprove),
            PermissionDefault::AskBefore
        );
        assert_eq!(
            PermissionConfig::default_for("read_clipboard", GooseMode::SmartApprove),
            PermissionDefault::AskBefore
        );
        assert_eq!(
            PermissionConfig::default_for("search_contacts", GooseMode::SmartApprove),
            PermissionDefault::AlwaysAllow
        );
        assert_eq!(
            PermissionConfig::default_for("read_file", GooseMode::SmartApprove),
            PermissionDefault::AlwaysAllow
        );
    }

    #[test]
    fn bash_and_mcp_never_auto() {
        let cfg = PermissionConfig::default();
        assert_eq!(
            cfg.resolve("bash", GooseMode::Auto),
            PermissionDefault::AskBefore
        );
        assert_eq!(
            cfg.resolve("ext__tool", GooseMode::Auto),
            PermissionDefault::AskBefore
        );
    }
}
