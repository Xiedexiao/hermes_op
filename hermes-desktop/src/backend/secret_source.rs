//! Pure helper functions for provider secret source resolution.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderSecretSourceInputs<'a> {
    pub provider: &'a str,
    pub runtime_api_key_ref: Option<&'a str>,
    pub config_api_key: Option<&'a str>,
}

impl<'a> ProviderSecretSourceInputs<'a> {
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
pub enum SecretSourceKind {
    NotRequired,
    RuntimeApiKeyRef,
    ProviderEnv,
    ConfigApiKey,
}

impl SecretSourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotRequired => "not_required",
            Self::RuntimeApiKeyRef => "runtime_api_key_ref",
            Self::ProviderEnv => "provider_env",
            Self::ConfigApiKey => "config_api_key",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretSourceDescriptor {
    pub kind: SecretSourceKind,
    pub label: &'static str,
    pub env_var: Option<String>,
    pub configured: bool,
    pub available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSecretSourceResolution {
    pub requires_secret: bool,
    pub selected_source_index: Option<usize>,
    pub sources: Vec<SecretSourceDescriptor>,
}

impl ProviderSecretSourceResolution {
    pub fn primary_source(&self) -> Option<&SecretSourceDescriptor> {
        self.sources.first()
    }

    pub fn selected_source(&self) -> Option<&SecretSourceDescriptor> {
        self.selected_source_index
            .and_then(|index| self.sources.get(index))
    }
}

pub fn provider_requires_secret(provider: &str) -> bool {
    !matches!(normalized_provider_key(provider).as_str(), "ollama")
}

pub fn provider_secret_env_var(provider: &str) -> Option<&'static str> {
    match normalized_provider_key(provider).as_str() {
        "openai" => Some("OPENAI_API_KEY"),
        "anthropic" => Some("ANTHROPIC_API_KEY"),
        "deepseek" => Some("DEEPSEEK_API_KEY"),
        "openrouter" => Some("OPENROUTER_API_KEY"),
        _ => None,
    }
}

pub fn resolve_provider_secret_sources<F>(
    inputs: ProviderSecretSourceInputs<'_>,
    mut env_var_has_value: F,
) -> ProviderSecretSourceResolution
where
    F: FnMut(&str) -> bool,
{
    if !provider_requires_secret(inputs.provider) {
        return ProviderSecretSourceResolution {
            requires_secret: false,
            selected_source_index: Some(0),
            sources: vec![SecretSourceDescriptor {
                kind: SecretSourceKind::NotRequired,
                label: "No API key required",
                env_var: None,
                configured: true,
                available: true,
            }],
        };
    }

    let runtime_api_key_ref = normalized_non_empty(inputs.runtime_api_key_ref);
    let config_api_key = normalized_non_empty(inputs.config_api_key);
    let provider_env_var = provider_secret_env_var(inputs.provider);

    let mut sources = Vec::new();

    if let Some(reference) = runtime_api_key_ref {
        sources.push(SecretSourceDescriptor {
            kind: SecretSourceKind::RuntimeApiKeyRef,
            label: "Runtime API key reference",
            env_var: Some(reference.to_string()),
            configured: true,
            available: env_var_has_value(reference),
        });
    }

    if let Some(env_var) = provider_env_var {
        sources.push(SecretSourceDescriptor {
            kind: SecretSourceKind::ProviderEnv,
            label: "Provider environment variable",
            env_var: Some(env_var.to_string()),
            configured: true,
            available: env_var_has_value(env_var),
        });
    }

    sources.push(SecretSourceDescriptor {
        kind: SecretSourceKind::ConfigApiKey,
        label: "Config API key",
        env_var: None,
        configured: config_api_key.is_some(),
        available: config_api_key.is_some(),
    });

    let selected_source_index = sources.iter().position(|source| source.available);

    ProviderSecretSourceResolution {
        requires_secret: true,
        selected_source_index,
        sources,
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
        inputs: ProviderSecretSourceInputs<'_>,
        available_env_vars: &[&str],
    ) -> ProviderSecretSourceResolution {
        resolve_provider_secret_sources(inputs, |name| {
            available_env_vars.iter().any(|item| item == &name)
        })
    }

    #[test]
    fn not_required_provider_resolves_to_single_selected_source() {
        let resolution =
            resolve_with_env(ProviderSecretSourceInputs::new(" Ollama ", None, None), &[]);

        assert!(!resolution.requires_secret);
        assert_eq!(resolution.selected_source_index, Some(0));
        assert_eq!(resolution.sources.len(), 1);

        let source = resolution
            .selected_source()
            .expect("not-required provider should select a source");
        assert_eq!(source.kind, SecretSourceKind::NotRequired);
        assert_eq!(source.kind.as_str(), "not_required");
        assert_eq!(source.label, "No API key required");
        assert_eq!(source.env_var, None);
        assert!(source.configured);
        assert!(source.available);
    }

    #[test]
    fn runtime_api_key_ref_has_highest_priority_when_present_in_env() {
        let resolution = resolve_with_env(
            ProviderSecretSourceInputs::new("openrouter", Some(" OPENROUTER_API_KEY "), None),
            &["OPENROUTER_API_KEY", "OPENAI_API_KEY"],
        );

        assert!(resolution.requires_secret);
        assert_eq!(resolution.selected_source_index, Some(0));
        assert_eq!(resolution.sources.len(), 3);

        let source = resolution
            .selected_source()
            .expect("runtime ref should be selected");
        assert_eq!(source.kind, SecretSourceKind::RuntimeApiKeyRef);
        assert_eq!(source.kind.as_str(), "runtime_api_key_ref");
        assert_eq!(source.label, "Runtime API key reference");
        assert_eq!(source.env_var.as_deref(), Some("OPENROUTER_API_KEY"));
        assert!(source.configured);
        assert!(source.available);
    }

    #[test]
    fn provider_env_source_is_selected_when_runtime_ref_is_absent() {
        let resolution = resolve_with_env(
            ProviderSecretSourceInputs::new(" anthropic ", None, None),
            &["ANTHROPIC_API_KEY"],
        );

        assert_eq!(resolution.selected_source_index, Some(0));
        assert_eq!(resolution.sources.len(), 2);

        let source = resolution
            .selected_source()
            .expect("provider env should be selected");
        assert_eq!(source.kind, SecretSourceKind::ProviderEnv);
        assert_eq!(source.kind.as_str(), "provider_env");
        assert_eq!(source.label, "Provider environment variable");
        assert_eq!(source.env_var.as_deref(), Some("ANTHROPIC_API_KEY"));
        assert!(source.configured);
        assert!(source.available);
    }

    #[test]
    fn config_api_key_fallback_is_selected_when_env_sources_are_unavailable() {
        let resolution = resolve_with_env(
            ProviderSecretSourceInputs::new(
                "deepseek",
                Some("DEEPSEEK_API_KEY"),
                Some(" config-secret "),
            ),
            &[],
        );

        assert_eq!(resolution.selected_source_index, Some(2));
        assert_eq!(resolution.sources.len(), 3);

        let runtime = &resolution.sources[0];
        assert_eq!(runtime.kind, SecretSourceKind::RuntimeApiKeyRef);
        assert!(runtime.configured);
        assert!(!runtime.available);

        let source = resolution
            .selected_source()
            .expect("config api key should be selected");
        assert_eq!(source.kind, SecretSourceKind::ConfigApiKey);
        assert_eq!(source.kind.as_str(), "config_api_key");
        assert_eq!(source.label, "Config API key");
        assert_eq!(source.env_var, None);
        assert!(source.configured);
        assert!(source.available);
    }

    #[test]
    fn unresolved_runtime_ref_remains_primary_when_nothing_else_is_available() {
        let resolution = resolve_with_env(
            ProviderSecretSourceInputs::new("openai", Some(" OPENAI_API_KEY "), None),
            &[],
        );

        assert!(resolution.requires_secret);
        assert_eq!(resolution.selected_source_index, None);
        assert_eq!(resolution.sources.len(), 3);

        let primary = resolution
            .primary_source()
            .expect("runtime ref should be the primary source");
        assert_eq!(primary.kind, SecretSourceKind::RuntimeApiKeyRef);
        assert_eq!(primary.env_var.as_deref(), Some("OPENAI_API_KEY"));
        assert!(primary.configured);
        assert!(!primary.available);

        let provider_env = &resolution.sources[1];
        assert_eq!(provider_env.kind, SecretSourceKind::ProviderEnv);
        assert_eq!(provider_env.env_var.as_deref(), Some("OPENAI_API_KEY"));
        assert!(provider_env.configured);
        assert!(!provider_env.available);

        let config = &resolution.sources[2];
        assert_eq!(config.kind, SecretSourceKind::ConfigApiKey);
        assert!(!config.configured);
        assert!(!config.available);
    }

    #[test]
    fn provider_helpers_normalize_supported_names() {
        assert!(provider_requires_secret("openai"));
        assert!(!provider_requires_secret("Ollama"));
        assert_eq!(provider_secret_env_var(" OPENAI "), Some("OPENAI_API_KEY"));
        assert_eq!(
            provider_secret_env_var("anthropic"),
            Some("ANTHROPIC_API_KEY")
        );
        assert_eq!(
            provider_secret_env_var("deepseek"),
            Some("DEEPSEEK_API_KEY")
        );
        assert_eq!(
            provider_secret_env_var("openrouter"),
            Some("OPENROUTER_API_KEY")
        );
        assert_eq!(provider_secret_env_var("custom"), None);
    }
}
