//! Pure helper functions for provider auth resolution.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderAuthInputs<'a> {
    pub provider: &'a str,
    pub runtime_api_key_ref: Option<&'a str>,
    pub config_api_key: Option<&'a str>,
}

impl<'a> ProviderAuthInputs<'a> {
    pub fn new(
        provider: &'a str,
        runtime_api_key_ref: Option<&'a str>,
        config_api_key: Option<&'a str>,
    ) -> Self {
        Self {
            provider,
            runtime_api_key_ref,
            config_api_key,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderAuthKind {
    NotRequired,
    RuntimeApiKeyRef,
    ProviderEnv,
    ConfigApiKey,
    None,
}

impl ProviderAuthKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotRequired => "not_required",
            Self::RuntimeApiKeyRef => "runtime_api_key_ref",
            Self::ProviderEnv => "provider_env",
            Self::ConfigApiKey => "config_api_key",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderAuthResolution {
    pub kind: ProviderAuthKind,
    pub label: &'static str,
    pub env_var: Option<String>,
    pub available: bool,
}

pub fn provider_requires_api_key(provider: &str) -> bool {
    !matches!(normalized_provider_key(provider).as_str(), "ollama")
}

pub fn provider_api_key_env(provider: &str) -> Option<&'static str> {
    match normalized_provider_key(provider).as_str() {
        "openai" => Some("OPENAI_API_KEY"),
        "anthropic" => Some("ANTHROPIC_API_KEY"),
        "deepseek" => Some("DEEPSEEK_API_KEY"),
        "openrouter" => Some("OPENROUTER_API_KEY"),
        _ => None,
    }
}

pub fn resolve_provider_auth<F>(
    inputs: ProviderAuthInputs<'_>,
    mut env_var_has_value: F,
) -> ProviderAuthResolution
where
    F: FnMut(&str) -> bool,
{
    let runtime_api_key_ref = normalized_non_empty(inputs.runtime_api_key_ref);
    let config_api_key = normalized_non_empty(inputs.config_api_key);

    if !provider_requires_api_key(inputs.provider) {
        return ProviderAuthResolution {
            kind: ProviderAuthKind::NotRequired,
            label: "No API key required",
            env_var: None,
            available: true,
        };
    }

    if let Some(reference) = runtime_api_key_ref
        && env_var_has_value(reference)
    {
        return ProviderAuthResolution {
            kind: ProviderAuthKind::RuntimeApiKeyRef,
            label: "Runtime API key reference",
            env_var: Some(reference.to_string()),
            available: true,
        };
    }

    if let Some(env_var) = provider_api_key_env(inputs.provider)
        && env_var_has_value(env_var)
    {
        return ProviderAuthResolution {
            kind: ProviderAuthKind::ProviderEnv,
            label: "Provider environment variable",
            env_var: Some(env_var.to_string()),
            available: true,
        };
    }

    if config_api_key.is_some() {
        return ProviderAuthResolution {
            kind: ProviderAuthKind::ConfigApiKey,
            label: "Config API key",
            env_var: provider_api_key_env(inputs.provider).map(str::to_string),
            available: true,
        };
    }

    ProviderAuthResolution {
        kind: if runtime_api_key_ref.is_some() {
            ProviderAuthKind::RuntimeApiKeyRef
        } else {
            ProviderAuthKind::None
        },
        label: if runtime_api_key_ref.is_some() {
            "Runtime API key reference"
        } else {
            "No auth source configured"
        },
        env_var: runtime_api_key_ref
            .map(str::to_string)
            .or_else(|| provider_api_key_env(inputs.provider).map(str::to_string)),
        available: false,
    }
}

fn normalized_non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn normalized_provider_key(provider: &str) -> String {
    provider.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve_with_env(
        inputs: ProviderAuthInputs<'_>,
        available_env_vars: &[&str],
    ) -> ProviderAuthResolution {
        resolve_provider_auth(inputs, |name| {
            available_env_vars.iter().any(|item| item == &name)
        })
    }

    #[test]
    fn ollama_is_ready_without_any_api_key() {
        let resolution = resolve_with_env(ProviderAuthInputs::new(" Ollama ", None, None), &[]);

        assert_eq!(resolution.kind, ProviderAuthKind::NotRequired);
        assert_eq!(resolution.kind.as_str(), "not_required");
        assert_eq!(resolution.label, "No API key required");
        assert_eq!(resolution.env_var, None);
        assert!(resolution.available);
    }

    #[test]
    fn runtime_api_key_ref_wins_when_its_env_var_exists() {
        let resolution = resolve_with_env(
            ProviderAuthInputs::new("openrouter", Some(" OPENROUTER_API_KEY "), None),
            &["OPENROUTER_API_KEY", "OPENAI_API_KEY"],
        );

        assert_eq!(resolution.kind, ProviderAuthKind::RuntimeApiKeyRef);
        assert_eq!(resolution.kind.as_str(), "runtime_api_key_ref");
        assert_eq!(resolution.label, "Runtime API key reference");
        assert_eq!(resolution.env_var.as_deref(), Some("OPENROUTER_API_KEY"));
        assert!(resolution.available);
    }

    #[test]
    fn provider_env_is_used_when_runtime_ref_is_absent() {
        let resolution = resolve_with_env(
            ProviderAuthInputs::new(" anthropic ", None, None),
            &["ANTHROPIC_API_KEY"],
        );

        assert_eq!(resolution.kind, ProviderAuthKind::ProviderEnv);
        assert_eq!(resolution.kind.as_str(), "provider_env");
        assert_eq!(resolution.label, "Provider environment variable");
        assert_eq!(resolution.env_var.as_deref(), Some("ANTHROPIC_API_KEY"));
        assert!(resolution.available);
    }

    #[test]
    fn config_api_key_falls_back_when_no_env_var_is_available() {
        let resolution = resolve_with_env(
            ProviderAuthInputs::new(
                "deepseek",
                Some("DEEPSEEK_API_KEY"),
                Some(" config-secret "),
            ),
            &[],
        );

        assert_eq!(resolution.kind, ProviderAuthKind::ConfigApiKey);
        assert_eq!(resolution.kind.as_str(), "config_api_key");
        assert_eq!(resolution.label, "Config API key");
        assert_eq!(resolution.env_var.as_deref(), Some("DEEPSEEK_API_KEY"));
        assert!(resolution.available);
    }

    #[test]
    fn missing_runtime_api_key_ref_reports_that_reference_as_unavailable() {
        let resolution = resolve_with_env(
            ProviderAuthInputs::new("openrouter", Some(" OPENROUTER_API_KEY "), None),
            &[],
        );

        assert_eq!(resolution.kind, ProviderAuthKind::RuntimeApiKeyRef);
        assert_eq!(resolution.label, "Runtime API key reference");
        assert_eq!(resolution.env_var.as_deref(), Some("OPENROUTER_API_KEY"));
        assert!(!resolution.available);
    }

    #[test]
    fn missing_provider_auth_uses_provider_env_hint_when_available() {
        let resolution = resolve_with_env(ProviderAuthInputs::new("openai", None, None), &[]);

        assert_eq!(resolution.kind, ProviderAuthKind::None);
        assert_eq!(resolution.kind.as_str(), "none");
        assert_eq!(resolution.label, "No auth source configured");
        assert_eq!(resolution.env_var.as_deref(), Some("OPENAI_API_KEY"));
        assert!(!resolution.available);
    }

    #[test]
    fn unknown_provider_without_auth_source_has_no_env_hint() {
        let resolution = resolve_with_env(
            ProviderAuthInputs::new("custom-provider", None, Some("   ")),
            &[],
        );

        assert_eq!(resolution.kind, ProviderAuthKind::None);
        assert_eq!(resolution.env_var, None);
        assert!(!resolution.available);
    }

    #[test]
    fn provider_helpers_match_supported_providers() {
        assert!(provider_requires_api_key("openai"));
        assert!(!provider_requires_api_key("ollama"));
        assert_eq!(provider_api_key_env("OPENAI"), Some("OPENAI_API_KEY"));
        assert_eq!(provider_api_key_env("anthropic"), Some("ANTHROPIC_API_KEY"));
        assert_eq!(provider_api_key_env("deepseek"), Some("DEEPSEEK_API_KEY"));
        assert_eq!(
            provider_api_key_env("openrouter"),
            Some("OPENROUTER_API_KEY")
        );
        assert_eq!(provider_api_key_env("custom"), None);
    }
}
