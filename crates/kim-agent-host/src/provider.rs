use std::sync::Arc;
use std::time::Duration;

use goose_provider_types::base::Provider;
use goose_providers::api_client::{ApiClient, AuthMethod};
use goose_providers::openai::{
    ensure_url_scheme, OpenAiProviderBuilder, OPEN_AI_DEFAULT_BASE_PATH,
};
use serde::{Deserialize, Serialize};

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

pub fn build_provider_from_spec(
    spec: &ProviderSpec,
    api_key: &str,
) -> Result<Arc<dyn Provider>, HostError> {
    if api_key.trim().is_empty() {
        return Err(HostError::MissingApiKey);
    }
    match spec.kind.trim().to_ascii_lowercase().as_str() {
        "openai" | "openai_compatible" | "responses_http" | "live" | "responses" | "" => {
            Ok(Arc::new(build_openai(spec, api_key)?))
        }
        "anthropic" | "messages" => Ok(Arc::new(build_anthropic(spec, api_key)?)),
        "scripted" => Ok(Arc::new(crate::scripted::ScriptedProvider::new(Vec::new()))),
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
    let fallback = fallback_models(spec);
    match build_provider_from_spec(spec, api_key) {
        Ok(provider) => match provider.fetch_supported_models().await {
            Ok(list) if !list.is_empty() => Ok(list),
            _ => Ok(fallback),
        },
        Err(_) => {
            if fallback.is_empty() {
                Err(HostError::UnknownProvider(spec.kind.clone()))
            } else {
                Ok(fallback)
            }
        }
    }
}

fn fallback_models(spec: &ProviderSpec) -> Vec<String> {
    match spec.kind.trim().to_ascii_lowercase().as_str() {
        "openai" | "openai_compatible" | "responses_http" | "live" | "responses" | "" => {
            goose_providers::openai::OPEN_AI_KNOWN_MODELS
                .iter()
                .map(|(name, _)| (*name).to_string())
                .collect()
        }
        "anthropic" | "messages" => FALLBACK_ANTHROPIC
            .iter()
            .map(|name| (*name).to_string())
            .collect(),
        other => bundled_declarative_json(other)
            .ok()
            .and_then(|json| goose_providers::declarative::deserialize_provider_config(json).ok())
            .map(|cfg| cfg.models.into_iter().map(|m| m.name).collect())
            .unwrap_or_default(),
    }
}

pub fn build_openai(
    spec: &ProviderSpec,
    api_key: &str,
) -> Result<goose_providers::openai::OpenAiProvider, HostError> {
    let (host, base_path) = split_openai_url(&spec.base_url)?;
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
    let host = anthropic_host(&spec.base_url)?;
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
    Ok(goose_providers::anthropic::AnthropicProviderBuilder::new(client).build())
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
    Ok((host, OPEN_AI_DEFAULT_BASE_PATH.to_string()))
}

pub fn anthropic_host(raw: &str) -> Result<String, HostError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(DEFAULT_ANTHROPIC_HOST.to_string());
    }
    let url = url::Url::parse(&ensure_url_scheme(trimmed))
        .map_err(|e| HostError::InvalidUrl(e.to_string()))?;
    let host = url[..url::Position::BeforePath].to_string();
    if host.is_empty() {
        Ok(DEFAULT_ANTHROPIC_HOST.to_string())
    } else {
        Ok(host)
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
    fn fallback_models_never_empty_for_openai() {
        let spec = ProviderSpec {
            kind: "openai".into(),
            base_url: String::new(),
            key_ref: "agent.api_key.goose".into(),
        };
        assert!(!fallback_models(&spec).is_empty());
    }
}
