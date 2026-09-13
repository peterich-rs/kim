use std::sync::Arc;
use std::time::Duration;

use goose_provider_types::base::Provider;
use goose_providers::anthropic::AnthropicProviderBuilder;
use goose_providers::api_client::{ApiClient, AuthMethod};
use goose_providers::openai::{
    ensure_url_scheme, OpenAiProviderBuilder, OPEN_AI_DEFAULT_BASE_PATH,
};
use serde::{Deserialize, Serialize};

use crate::catalog::{self, BuilderKind};
use crate::profile::ProviderSpec;
use crate::HostError;

pub const DEFAULT_OPENAI_HOST: &str = "https://api.openai.com";
pub const DEFAULT_ANTHROPIC_HOST: &str = "https://api.anthropic.com";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderKind {
    OpenAi,
    Anthropic,
}

impl ProviderKind {
    pub fn parse(raw: &str) -> Result<Self, HostError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "openai" | "responses_http" | "live" | "responses" | "scripted" | "" => {
                Ok(Self::OpenAi)
            }
            "anthropic" | "messages" => Ok(Self::Anthropic),
            other => Err(HostError::UnknownProvider(other.to_string())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

impl ProviderConfig {
    pub fn openai(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            kind: ProviderKind::OpenAi,
            base_url: format!("{DEFAULT_OPENAI_HOST}/v1"),
            api_key: api_key.into(),
            model: model.into(),
        }
    }
}

pub fn effective_base_url(spec: &ProviderSpec) -> String {
    let trimmed = spec.base_url.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    catalog::default_base_url(&spec.kind)
        .ok()
        .flatten()
        .unwrap_or("")
        .to_string()
}

pub fn build_provider_from_spec(
    spec: &ProviderSpec,
    api_key: &str,
) -> Result<Arc<dyn Provider>, HostError> {
    if api_key.trim().is_empty() {
        return Err(HostError::MissingApiKey);
    }
    let kind = spec.kind.trim().to_ascii_lowercase();
    if kind == "scripted" {
        #[cfg(test)]
        {
            return Ok(Arc::new(crate::scripted::ScriptedProvider::new(Vec::new())));
        }
        #[cfg(not(test))]
        {
            return Err(HostError::UnknownProvider("scripted".into()));
        }
    }
    tracing::info!(kind = %spec.kind, "builder kind");
    match catalog::builder_kind(&spec.kind)? {
        Some(BuilderKind::Openai) => Ok(Arc::new(build_openai(spec, api_key)?)),
        Some(BuilderKind::Anthropic) => Ok(Arc::new(build_anthropic(spec, api_key)?)),
        None => match kind.as_str() {
            "openai" | "openai_compatible" | "responses_http" | "live" | "responses" | "" => {
                Ok(Arc::new(build_openai(spec, api_key)?))
            }
            "anthropic" | "messages" => Ok(Arc::new(build_anthropic(spec, api_key)?)),
            other => {
                let json = bundled_declarative_json(other)?;
                let cfg = goose_providers::declarative::deserialize_provider_config(json)
                    .map_err(|e| HostError::Failed(e.to_string()))?;
                if !is_mobile_eligible(&cfg) {
                    return Err(HostError::UnknownProvider(format!(
                        "{other} requires process env"
                    )));
                }
                let boxed = goose_providers::declarative::from_json(
                    json,
                    None,
                    SessionKeyResolver {
                        api_key: api_key.to_string(),
                    },
                )
                .map_err(|e| HostError::Failed(e.to_string()))?;
                Ok(Arc::from(boxed))
            }
        },
    }
}

pub struct SessionKeyResolver {
    pub api_key: String,
}

impl goose_providers::declarative::KeyResolver for SessionKeyResolver {
    type Error = std::io::Error;

    fn resolve_key(&self, _name: &str) -> Result<String, Self::Error> {
        if self.api_key.trim().is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "api key missing",
            ));
        }
        Ok(self.api_key.clone())
    }
}

#[derive(Debug, Clone)]
pub struct BundledProviderSummary {
    pub name: String,
    pub display_name: String,
    pub mobile: bool,
}

pub fn bundled_provider_summaries() -> Vec<BundledProviderSummary> {
    let mut out = Vec::new();
    for (_, json) in goose_providers::declarative::fixed_provider_config_entries() {
        let Ok(cfg) = goose_providers::declarative::deserialize_provider_config(json) else {
            continue;
        };
        let mobile = is_mobile_eligible(&cfg);
        out.push(BundledProviderSummary {
            name: cfg.name,
            display_name: cfg.display_name,
            mobile,
        });
    }
    out
}

pub fn bundled_declarative_json(name: &str) -> Result<&'static str, HostError> {
    let want = name.trim().to_ascii_lowercase();
    for (_, json) in goose_providers::declarative::fixed_provider_config_entries() {
        let Ok(cfg) = goose_providers::declarative::deserialize_provider_config(json) else {
            continue;
        };
        if cfg.name.eq_ignore_ascii_case(&want) {
            return Ok(json);
        }
    }
    Err(HostError::UnknownProvider(name.to_string()))
}

fn is_mobile_eligible(cfg: &goose_providers::declarative::DeclarativeProviderConfig) -> bool {
    if cfg.base_url.contains("${") {
        return false;
    }
    if let Some(vars) = &cfg.env_vars {
        if vars.iter().any(|v| v.required) {
            return false;
        }
    }
    true
}

const FALLBACK_ANTHROPIC: &[&str] = &[
    "claude-sonnet-4-5",
    "claude-opus-4-5",
    "claude-haiku-4-5",
    "claude-sonnet-4-6",
    "claude-opus-4-6",
];

pub async fn fetch_models(spec: &ProviderSpec, api_key: &str) -> Result<Vec<String>, HostError> {
    if api_key.trim().is_empty() {
        return Err(HostError::MissingApiKey);
    }
    let provider = build_provider_from_spec(spec, api_key)?;
    let list = provider
        .fetch_supported_models()
        .await
        .map_err(|e| HostError::Failed(e.to_string()))?;
    let list: Vec<String> = list
        .into_iter()
        .filter(|id| is_concrete_model_id(id))
        .collect();
    if list.is_empty() {
        return Err(HostError::Failed("empty model list".into()));
    }
    Ok(list)
}

/// Gateways often advertise routing aliases (`gpt-*`) on `/v1/models`.
/// Those are not a single chat model id.
pub(crate) fn is_concrete_model_id(id: &str) -> bool {
    let id = id.trim();
    !id.is_empty() && !id.contains('*') && !id.contains('?')
}

pub(crate) fn fallback_models(spec: &ProviderSpec) -> Vec<String> {
    let kind = catalog::normalize_vendor_id(&spec.kind);
    if let Ok(ids) = catalog::catalog_model_ids(&kind) {
        if !ids.is_empty() {
            return ids;
        }
    }
    if let Ok(Some(name)) = catalog::goose_fallback_name(&kind) {
        if let Some(models) = bundled_models(name) {
            return models;
        }
    }
    match kind.as_str() {
        "openai" | "openai_compatible" => goose_providers::openai::OPEN_AI_KNOWN_MODELS
            .iter()
            .map(|(name, _)| (*name).to_string())
            .collect(),
        "anthropic" => FALLBACK_ANTHROPIC
            .iter()
            .map(|name| (*name).to_string())
            .collect(),
        _ => Vec::new(),
    }
}

fn bundled_models(name: &str) -> Option<Vec<String>> {
    bundled_declarative_json(name)
        .ok()
        .and_then(|json| goose_providers::declarative::deserialize_provider_config(json).ok())
        .map(|cfg| cfg.models.into_iter().map(|m| m.name).collect())
}

pub fn build_openai(
    spec: &ProviderSpec,
    api_key: &str,
) -> Result<goose_providers::openai::OpenAiProvider, HostError> {
    let (host, base_path) = split_openai_url(&effective_base_url(spec))?;
    let client = ApiClient::with_timeout_and_tls(
        host,
        AuthMethod::BearerToken(api_key.to_string()),
        Duration::from_secs(120),
        None,
    )
    .map_err(|e| HostError::Failed(e.to_string()))?;
    Ok(OpenAiProviderBuilder::new(client)
        .base_path(base_path)
        .supports_streaming(true)
        .build())
}

pub fn build_anthropic(
    spec: &ProviderSpec,
    api_key: &str,
) -> Result<goose_providers::anthropic::AnthropicProvider, HostError> {
    let host = anthropic_host(&effective_base_url(spec))?;
    let client = ApiClient::with_timeout_and_tls(
        host,
        AuthMethod::ApiKey {
            header_name: "x-api-key".into(),
            key: api_key.to_string(),
        },
        Duration::from_secs(120),
        None,
    )
    .map_err(|e| HostError::Failed(e.to_string()))?;
    let mut builder = AnthropicProviderBuilder::new(client);
    if catalog::normalize_vendor_id(&spec.kind) == "minimax" {
        builder = builder.name("minimax").skip_canonical_filtering(true);
    }
    Ok(builder.build())
}

pub fn split_openai_url(raw: &str) -> Result<(String, String), HostError> {
    let fallback = (
        DEFAULT_OPENAI_HOST.to_string(),
        OPEN_AI_DEFAULT_BASE_PATH.to_string(),
    );
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(fallback);
    }
    let url = url::Url::parse(&ensure_url_scheme(trimmed))
        .map_err(|e| HostError::InvalidUrl(e.to_string()))?;
    let host = url[..url::Position::BeforePath].to_string();
    if host.is_empty() {
        return Ok(fallback);
    }
    let path = url.path().trim_matches('/');
    let lower = path.to_ascii_lowercase();
    let base_path = if lower.contains("chat/completions") || lower.contains("responses") {
        path.to_string()
    } else if path.is_empty() {
        "v1/chat/completions".to_string()
    } else {
        format!("{path}/chat/completions")
    };
    Ok((host, base_path))
}

pub fn anthropic_host(raw: &str) -> Result<String, HostError> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Ok(DEFAULT_ANTHROPIC_HOST.to_string());
    }
    let url = url::Url::parse(&ensure_url_scheme(trimmed))
        .map_err(|e| HostError::InvalidUrl(e.to_string()))?;
    let origin = url[..url::Position::BeforePath].to_string();
    if origin.is_empty() {
        return Ok(DEFAULT_ANTHROPIC_HOST.to_string());
    }
    let path = url.path().trim_end_matches('/');
    if path.is_empty() || path == "/" {
        Ok(origin)
    } else {
        Ok(format!("{origin}{path}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use goose_providers::declarative::KeyResolver;

    #[test]
    fn provider_kind_aliases() {
        assert_eq!(ProviderKind::parse("openai").unwrap(), ProviderKind::OpenAi);
        assert_eq!(
            ProviderKind::parse("anthropic").unwrap(),
            ProviderKind::Anthropic
        );
        assert_eq!(
            ProviderKind::parse("responses_http").unwrap(),
            ProviderKind::OpenAi
        );
        assert_eq!(
            ProviderKind::parse("scripted").unwrap(),
            ProviderKind::OpenAi
        );
        assert!(ProviderKind::parse("unknown").is_err());
    }

    #[test]
    fn spec_scripted_builds() {
        let spec = ProviderSpec {
            kind: "scripted".into(),
            base_url: String::new(),
            key_ref: "agent.api_key.goose".into(),
        };
        assert!(build_provider_from_spec(&spec, "sk-dummy").is_ok());
    }

    #[test]
    fn session_key_resolver_empty_fails() {
        let resolver = SessionKeyResolver {
            api_key: String::new(),
        };
        assert!(resolver.resolve_key("OPENAI_API_KEY").is_err());
    }

    #[test]
    fn bundled_unknown_name_is_unknown_provider() {
        match bundled_declarative_json("not-a-real-provider") {
            Err(HostError::UnknownProvider(_)) => {}
            Err(_) => panic!("expected UnknownProvider"),
            Ok(_) => panic!("expected UnknownProvider"),
        }
    }

    #[test]
    fn wildcard_model_ids_are_not_concrete() {
        assert!(is_concrete_model_id("gpt-4o"));
        assert!(is_concrete_model_id("composer-2.5"));
        assert!(!is_concrete_model_id("gpt-*"));
        assert!(!is_concrete_model_id("claude-*"));
        assert!(!is_concrete_model_id("codex-*"));
        assert!(!is_concrete_model_id(""));
    }

    #[tokio::test]
    async fn fetch_models_empty_key_is_missing_api_key() {
        let spec = ProviderSpec {
            kind: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            key_ref: String::new(),
        };
        let err = fetch_models(&spec, "").await.unwrap_err();
        assert!(matches!(err, HostError::MissingApiKey), "{err:?}");
    }

    #[test]
    fn fallback_models_never_empty_for_openai() {
        let spec = ProviderSpec {
            kind: "openai".into(),
            base_url: String::new(),
            key_ref: "agent.api_key.goose".into(),
        };
        assert!(!fallback_models(&spec).is_empty());
    }

    #[test]
    fn fallback_models_deepseek_is_catalog_not_gpt4o() {
        let spec = ProviderSpec {
            kind: "deepseek".into(),
            base_url: String::new(),
            key_ref: String::new(),
        };
        let models = fallback_models(&spec);
        assert!(models.contains(&"deepseek-flash".into()), "{models:?}");
        assert!(!models.iter().any(|m| m.starts_with("gpt-")), "{models:?}");
    }

    #[test]
    fn fallback_models_groq_is_not_gpt4o() {
        let spec = ProviderSpec {
            kind: "groq".into(),
            base_url: String::new(),
            key_ref: String::new(),
        };
        let models = fallback_models(&spec);
        assert!(!models.is_empty(), "{models:?}");
        assert!(!models.iter().any(|m| m == "gpt-4o"), "{models:?}");
        assert!(
            models
                .iter()
                .any(|m| m.contains("llama") || m.contains("groq") || m.contains('/')),
            "{models:?}"
        );
    }

    #[test]
    fn split_openai_url_goldens() {
        let cases = [
            (
                "https://api.deepseek.com",
                "https://api.deepseek.com",
                "v1/chat/completions",
                "v1/models",
            ),
            (
                "https://api.moonshot.cn/v1",
                "https://api.moonshot.cn",
                "v1/chat/completions",
                "v1/models",
            ),
            (
                "https://dashscope.aliyuncs.com/compatible-mode/v1",
                "https://dashscope.aliyuncs.com",
                "compatible-mode/v1/chat/completions",
                "compatible-mode/v1/models",
            ),
            (
                "https://open.bigmodel.cn/api/paas/v4",
                "https://open.bigmodel.cn",
                "api/paas/v4/chat/completions",
                "api/paas/v4/models",
            ),
            (
                "https://openrouter.ai/api/v1",
                "https://openrouter.ai",
                "api/v1/chat/completions",
                "api/v1/models",
            ),
            (
                "https://api.x.ai/v1",
                "https://api.x.ai",
                "v1/chat/completions",
                "v1/models",
            ),
            (
                "https://api.groq.com/openai/v1",
                "https://api.groq.com",
                "openai/v1/chat/completions",
                "openai/v1/models",
            ),
            (
                "https://api.groq.com/openai/v1/chat/completions",
                "https://api.groq.com",
                "openai/v1/chat/completions",
                "openai/v1/models",
            ),
        ];
        for (input, host, chat, models) in cases {
            let (got_host, got_path) = split_openai_url(input).unwrap();
            assert_eq!(got_host, host, "host for {input}");
            assert_eq!(got_path, chat, "chat path for {input}");
            assert!(
                got_path.contains("chat/completions") || got_path.contains("responses"),
                "chat path must be a full completions path: {got_path}"
            );
            assert_eq!(
                goose_map_base_path(&got_path, "models", "v1/models"),
                models,
                "models prefix for {input}"
            );
        }
    }

    /// Mirrors goose-providers 0.1.0-alpha.9 `OpenAiProvider::map_base_path`.
    fn goose_map_base_path(base_path: &str, target: &str, fallback: &str) -> String {
        let normalized = if let Some(path) = base_path.strip_prefix('/') {
            format!("/{}", path.trim_end_matches('/'))
        } else {
            base_path.trim_end_matches('/').to_string()
        };
        if normalized.ends_with(target) || normalized.contains(&format!("/{target}")) {
            return normalized;
        }
        let lower = normalized.to_ascii_lowercase();
        if lower.contains("chat/completions") {
            return normalized.replacen("chat/completions", target, 1);
        }
        if lower.ends_with("responses") || lower.contains("/responses") {
            return normalized.replacen("responses", target, 1);
        }
        if normalized.starts_with('/') {
            format!("/{}", fallback.trim_start_matches('/'))
        } else {
            fallback.to_string()
        }
    }

    fn dummy_spec(kind: &str, base_url: &str) -> ProviderSpec {
        ProviderSpec {
            kind: kind.into(),
            base_url: base_url.into(),
            key_ref: "agent.api_key.goose".into(),
        }
    }

    #[test]
    fn build_catalog_vendor_uses_derived_host_and_path() {
        let spec = dummy_spec("qwen", "");
        let (host, path) = split_openai_url(&effective_base_url(&spec)).unwrap();
        assert_eq!(host, "https://dashscope.aliyuncs.com");
        assert_eq!(path, "compatible-mode/v1/chat/completions");
        let provider = build_openai(&spec, "sk-dummy").unwrap();
        let json = serde_json::to_value(&provider).unwrap();
        assert_eq!(json["base_path"], "compatible-mode/v1/chat/completions");
        assert!(build_provider_from_spec(&spec, "sk-dummy").is_ok());
    }

    #[test]
    fn build_deepseek_without_env() {
        let spec = dummy_spec("deepseek", "");
        let (host, path) = split_openai_url(&effective_base_url(&spec)).unwrap();
        assert_eq!(host, "https://api.deepseek.com");
        assert_eq!(path, "v1/chat/completions");
        let provider = build_openai(&spec, "sk-dummy").unwrap();
        let json = serde_json::to_value(&provider).unwrap();
        assert_eq!(json["base_path"], "v1/chat/completions");
        let dbg = format!("{provider:?}");
        assert!(dbg.contains("api.deepseek.com"), "{dbg}");
        assert!(build_provider_from_spec(&spec, "sk-dummy").is_ok());
    }

    #[test]
    fn build_xai_uses_v1_chat_completions() {
        let spec = dummy_spec("xai", "");
        let (host, path) = split_openai_url(&effective_base_url(&spec)).unwrap();
        assert_eq!(host, "https://api.x.ai");
        assert_eq!(path, "v1/chat/completions");
        assert_eq!(
            goose_map_base_path(&path, "models", "v1/models"),
            "v1/models"
        );
        let provider = build_openai(&spec, "sk-dummy").unwrap();
        let json = serde_json::to_value(&provider).unwrap();
        assert_eq!(json["base_path"], "v1/chat/completions");
        assert!(build_provider_from_spec(&spec, "sk-dummy").is_ok());
        let via_grok = dummy_spec("grok", "");
        assert_eq!(
            split_openai_url(&effective_base_url(&via_grok)).unwrap().0,
            "https://api.x.ai"
        );
    }

    #[test]
    fn build_groq_keeps_openai_prefix() {
        let spec = dummy_spec("groq", "");
        let (host, path) = split_openai_url(&effective_base_url(&spec)).unwrap();
        assert_eq!(host, "https://api.groq.com");
        assert_eq!(path, "openai/v1/chat/completions");
        assert_eq!(
            goose_map_base_path(&path, "models", "v1/models"),
            "openai/v1/models"
        );
        let provider = build_openai(&spec, "sk-dummy").unwrap();
        let json = serde_json::to_value(&provider).unwrap();
        assert_eq!(json["base_path"], "openai/v1/chat/completions");
        assert!(build_provider_from_spec(&spec, "sk-dummy").is_ok());
    }

    #[test]
    fn build_minimax_uses_full_anthropic_path_and_name() {
        let spec = dummy_spec("minimax", "");
        let host = anthropic_host(&effective_base_url(&spec)).unwrap();
        assert_eq!(host, "https://api.minimaxi.com/anthropic");
        let provider = build_anthropic(&spec, "sk-dummy").unwrap();
        assert_eq!(provider.get_name(), "minimax");
        assert!(build_provider_from_spec(&spec, "sk-dummy").is_ok());
    }

    #[test]
    fn moonshot_zhipu_do_not_require_process_env() {
        for kind in ["moonshot", "zhipu"] {
            let spec = dummy_spec(kind, "");
            assert!(
                build_provider_from_spec(&spec, "sk-dummy").is_ok(),
                "{kind} should build from catalog URL"
            );
        }
    }
}
