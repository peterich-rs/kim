use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use async_trait::async_trait;
use goose_agent::operation::{applied, not_applicable, Operation, OperationResult};
use goose_provider_types::conversation::message::Message;
use goose_provider_types::conversation::Conversation;
use rmcp::model::{CallToolResult, ContentBlock, JsonObject, Tool};
use serde_json::{json, Value};

use crate::events::HostEffect;
use crate::ops::unanswered_tool_requests;
use crate::skills::{activate, activation_message, SkillRegistry, SkillResolver};
use crate::HostSession;

pub const ACTIVATE_SKILL: &str = "activate_skill";

/// Progressive disclosure, host-side. The catalog is already in the system
/// prompt; this operation turns one `activate_skill` call into the skill's text
/// as a hidden user message so the next inference can follow it.
///
/// An `Operation` and not a `ToolProvider`: a provider can only answer the
/// call, and appending to the conversation from inside `call` would mutate
/// state the machine owns.
pub struct SkillOp {
    registry: Arc<SkillRegistry>,
    resolver: Arc<SkillResolver>,
    /// `(id, version, path)` already injected this session. A new version of
    /// the same id is a different key, so an update injects again.
    activated: Mutex<HashSet<(String, String, String)>>,
}

impl SkillOp {
    pub fn new(registry: Arc<SkillRegistry>, resolver: Arc<SkillResolver>) -> Self {
        Self {
            registry,
            resolver,
            activated: Mutex::new(HashSet::new()),
        }
    }

    fn mark(&self, key: (String, String, String)) -> bool {
        let mut seen = self
            .activated
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        seen.insert(key)
    }
}

fn schema(value: Value) -> Arc<JsonObject> {
    match value {
        Value::Object(map) => Arc::new(map),
        _ => Arc::new(JsonObject::new()),
    }
}

fn error_result(message: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(message.into())])
}

pub fn activate_skill_tool() -> Tool {
    Tool::new(
        ACTIVATE_SKILL,
        "Load an assigned skill's instructions (or a file under that skill). \
Use when the task matches a listed skill.",
        schema(json!({
            "type": "object",
            "properties": {
                "id": {"type": "string"},
                "path": {
                    "type": "string",
                    "description": "Relative to the skill directory. Default SKILL.md"
                }
            },
            "required": ["id"],
            "additionalProperties": false
        })),
    )
}

#[async_trait]
impl Operation<HostSession, HostEffect> for SkillOp {
    fn name(&self) -> &'static str {
        "skills"
    }

    async fn inference_tools(&self, _session: &HostSession) -> Result<Vec<Tool>> {
        Ok(vec![activate_skill_tool()])
    }

    async fn run(
        &self,
        _session: &HostSession,
        conversation: &Conversation,
        _emit: &goose_agent::operation::Emitter,
    ) -> Result<OperationResult<HostEffect>> {
        let pending: Vec<(String, JsonObject)> = unanswered_tool_requests(conversation)
            .into_iter()
            .filter_map(|request| {
                let call = request.tool_call.as_ref().ok()?;
                if call.name.as_ref() != ACTIVATE_SKILL {
                    return None;
                }
                Some((
                    request.id.clone(),
                    call.arguments.clone().unwrap_or_default(),
                ))
            })
            .collect();
        if pending.is_empty() {
            return not_applicable();
        }

        let mut response = Message::user();
        let mut bodies = Vec::new();
        for (call_id, arguments) in pending {
            let id = arguments
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_string();
            if id.is_empty() {
                response.add_tool_response_with_metadata(
                    call_id,
                    Ok(error_result("activate_skill requires an id")),
                    None,
                );
                continue;
            }
            let path = arguments.get("path").and_then(Value::as_str);
            match activate(&self.registry, &self.resolver, &id, path) {
                Ok(activation) => {
                    let fresh = self.mark((
                        activation.id.clone(),
                        activation.version.clone(),
                        activation.path.clone(),
                    ));
                    if fresh {
                        bodies.push(activation_message(&activation));
                    }
                    tracing::info!(
                        skill_id = %activation.id,
                        version = %activation.version,
                        ok = true,
                        already = !fresh,
                        "activate_skill"
                    );
                    response.add_tool_response_with_metadata(
                        call_id,
                        Ok(CallToolResult::structured(json!({
                            "ok": true,
                            "already": !fresh,
                            "id": activation.id,
                            "version": activation.version,
                            "path": activation.path,
                            "truncated": activation.truncated,
                        }))),
                        None,
                    );
                }
                Err(err) => {
                    tracing::warn!(skill_id = %id, ok = false, error = %err, "activate_skill");
                    response.add_tool_response_with_metadata(
                        call_id,
                        Ok(error_result(err.to_string())),
                        None,
                    );
                }
            }
        }

        let mut effects = vec![HostEffect::from(response)];
        for body in bodies {
            // Hidden from the user and not user-visible, so it cannot become
            // the kickoff message of the turn.
            effects.push(HostEffect::from(
                Message::user().with_text(body).with_visibility(false, true),
            ));
        }
        applied(effects)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::{build_registry, RegistryScan, SkillRef};
    use goose_agent::operation::Emitter;
    use std::path::Path;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    fn emit() -> Emitter {
        let (tx, _rx) = mpsc::channel(8);
        Emitter::new(tx, CancellationToken::new())
    }

    fn session() -> HostSession {
        HostSession {
            id: "s".into(),
            conversation: Conversation::empty(),
        }
    }

    fn call(call_id: &str, id: &str, path: Option<&str>) -> Message {
        let mut arguments = JsonObject::new();
        arguments.insert("id".into(), json!(id));
        if let Some(path) = path {
            arguments.insert("path".into(), json!(path));
        }
        let mut params = rmcp::model::CallToolRequestParams::new(ACTIVATE_SKILL);
        params.arguments = Some(arguments);
        Message::assistant().with_tool_request(call_id, Ok(params))
    }

    fn turn(request: Message) -> Conversation {
        Conversation::new_unvalidated(vec![Message::user().with_text("go"), request])
    }

    fn messages(result: OperationResult<HostEffect>) -> Vec<Message> {
        let OperationResult::Applied(step) = result else {
            panic!("expected the operation to apply");
        };
        step.effects
            .into_iter()
            .filter_map(|effect| match effect {
                HostEffect::Conversation(
                    goose_agent::operation::ConversationEffect::AppendMessage(message),
                ) => Some(message),
                _ => None,
            })
            .collect()
    }

    fn write_portable(root: &Path, id: &str, body: &str) {
        let dir = root.join(id);
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {id}\ndescription: d\nversion: 1\n---\n\n{body}\n"),
        )
        .expect("write");
    }

    fn portable_op(user_root: &Path) -> SkillOp {
        let resolver = Arc::new(SkillResolver::new());
        let registry = build_registry(
            &[],
            &[],
            &RegistryScan {
                user_root: &user_root.to_string_lossy(),
                project_root: user_root,
                enabled: true,
            },
            &resolver,
        );
        SkillOp::new(Arc::new(registry), resolver)
    }

    #[tokio::test]
    async fn no_activate_request_is_not_applicable() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_portable(dir.path(), "git-commit", "commit body");
        let op = portable_op(dir.path());
        let conversation = Conversation::new_unvalidated(vec![Message::user().with_text("go")]);
        let result = op.run(&session(), &conversation, &emit()).await.unwrap();
        assert!(matches!(result, OperationResult::NotApplicable));
    }

    #[tokio::test]
    async fn activate_appends_hidden_body_and_tool_response() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_portable(dir.path(), "git-commit", "commit body");
        let op = portable_op(dir.path());
        let tools = op.inference_tools(&session()).await.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name.as_ref(), ACTIVATE_SKILL);

        let out = messages(
            op.run(&session(), &turn(call("c1", "git-commit", None)), &emit())
                .await
                .unwrap(),
        );
        assert_eq!(out.len(), 2, "{out:?}");
        assert!(out[0].is_tool_response());
        assert!(!out[0].get_tool_response_ids().is_empty());

        let hidden = &out[1];
        assert!(!hidden.metadata.user_visible, "body must stay hidden");
        assert!(hidden.metadata.agent_visible, "model must see the body");
        assert!(!hidden.is_user_visible(), "hidden body cannot be a kickoff");
        assert_eq!(
            hidden.as_concat_text(),
            "skill: git-commit@1\n\ncommit body"
        );
    }

    #[tokio::test]
    async fn second_activation_reports_already_without_a_new_body() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_portable(dir.path(), "git-commit", "commit body");
        let op = portable_op(dir.path());
        let conversation = turn(call("c1", "git-commit", None));
        assert_eq!(
            messages(op.run(&session(), &conversation, &emit()).await.unwrap()).len(),
            2
        );

        let again = messages(
            op.run(&session(), &turn(call("c2", "git-commit", None)), &emit())
                .await
                .unwrap(),
        );
        assert_eq!(again.len(), 1, "no second body: {again:?}");
        let text = again[0]
            .content
            .iter()
            .filter_map(|block| block.as_tool_response_text())
            .collect::<String>();
        assert!(text.contains("\"already\":true"), "{text}");
    }

    #[tokio::test]
    async fn new_app_version_injects_again() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache = dir.path().join("cache").join("kim-im");
        let first = cache.join("1");
        std::fs::create_dir_all(&first).expect("mkdir");
        std::fs::write(
            first.join("SKILL.md"),
            "---\nname: kim-im\ndescription: d\nversion: 1\n---\n\nversion one\n",
        )
        .expect("write");

        let resolver = Arc::new(SkillResolver::with_cache(dir.path().join("cache")));
        let registry = build_registry(
            &[SkillRef {
                id: "kim-im".into(),
                ..SkillRef::default()
            }],
            &[],
            &RegistryScan {
                user_root: "",
                project_root: dir.path(),
                enabled: false,
            },
            &resolver,
        );
        let op = SkillOp::new(Arc::new(registry), resolver);

        let out = messages(
            op.run(&session(), &turn(call("c1", "kim-im", None)), &emit())
                .await
                .unwrap(),
        );
        assert_eq!(out[1].as_concat_text(), "skill: kim-im@1\n\nversion one");

        let second = cache.join("2");
        std::fs::create_dir_all(&second).expect("mkdir");
        std::fs::write(
            second.join("SKILL.md"),
            "---\nname: kim-im\ndescription: d\nversion: 2\n---\n\nversion two\n",
        )
        .expect("write");

        let updated = messages(
            op.run(&session(), &turn(call("c2", "kim-im", None)), &emit())
                .await
                .unwrap(),
        );
        assert_eq!(updated.len(), 2, "an update must inject again");
        assert_eq!(
            updated[1].as_concat_text(),
            "skill: kim-im@2\n\nversion two"
        );
    }

    #[tokio::test]
    async fn id_outside_the_catalog_answers_with_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_portable(dir.path(), "git-commit", "commit body");
        let op = portable_op(dir.path());
        let out = messages(
            op.run(&session(), &turn(call("c1", "kim-memory", None)), &emit())
                .await
                .unwrap(),
        );
        assert_eq!(out.len(), 1, "no body for an unlisted id: {out:?}");
        let errored = out[0].content.iter().any(|block| match block {
            goose_provider_types::conversation::message::MessageContent::ToolResponse(response) => {
                response
                    .tool_result
                    .as_ref()
                    .is_ok_and(|result| result.is_error == Some(true))
            }
            _ => false,
        });
        assert!(errored, "{out:?}");
    }

    #[tokio::test]
    async fn escaping_path_answers_with_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_portable(dir.path(), "git-commit", "commit body");
        std::fs::write(dir.path().join("secret.txt"), "nope").expect("write");
        let op = portable_op(dir.path());
        let out = messages(
            op.run(
                &session(),
                &turn(call("c1", "git-commit", Some("../secret.txt"))),
                &emit(),
            )
            .await
            .unwrap(),
        );
        assert_eq!(out.len(), 1, "{out:?}");
        let text = out[0]
            .content
            .iter()
            .filter_map(|block| block.as_tool_response_text())
            .collect::<String>();
        assert!(text.contains("escapes"), "{text}");
    }
}
