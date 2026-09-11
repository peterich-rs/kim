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
        "scripted" => Err(HostError::UnknownProvider("scripted outside tests".into())),
        other => Err(HostError::UnknownProvider(other.to_string())),
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
    fn spec_scripted_is_unknown_outside_tests() {
        let spec = ProviderSpec {
            kind: "scripted".into(),
            base_url: String::new(),
            key_ref: "agent.api_key.goose".into(),
        };
        match build_provider_from_spec(&spec, "sk-dummy") {
            Err(HostError::UnknownProvider(_)) => {}
            Err(_) => panic!("expected UnknownProvider"),
            Ok(_) => panic!("expected UnknownProvider"),
        }
    }
}
