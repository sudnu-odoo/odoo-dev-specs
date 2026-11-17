//! The models each provider offers, shown in the chat's model picker.
//! Kept small and curated: the few models a business user should choose between,
//! with the default first.

use serde::Serialize;

use crate::settings::Provider;

/// One selectable model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    /// Value passed to the CLI's `--model`/`-m` flag.
    pub id: String,
    /// Short human label, e.g. "Sonnet (balanced)".
    pub label: String,
}

fn model(id: &str, label: &str) -> Model {
    Model {
        id: id.into(),
        label: label.into(),
    }
}

/// Models for a provider, default first.
pub fn models(provider: Provider) -> Vec<Model> {
    match provider {
        Provider::Claude => vec![
            model("sonnet", "Sonnet (balanced)"),
            model("opus", "Opus (most capable)"),
            model("haiku", "Haiku (fastest)"),
        ],
        Provider::Gemini => vec![
            model("gemini-2.5-flash", "2.5 Flash (balanced)"),
            model("gemini-2.5-pro", "2.5 Pro (most capable)"),
            model("gemini-3.1-flash-lite", "3.1 Flash Lite (fastest)"),
        ],
    }
}

/// The default model id for a provider (the first in its list).
pub fn default_model(provider: Provider) -> String {
    models(provider)
        .into_iter()
        .next()
        .expect("each provider has models")
        .id
}

/// Whether `id` is one of the provider's models. Guards against a stale choice
/// after the user switched provider.
pub fn is_valid(provider: Provider, id: &str) -> bool {
    models(provider).iter().any(|m| m.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_provider_lists_models_default_first() {
        let claude = models(Provider::Claude);
        assert_eq!(claude.first().unwrap().id, "sonnet");
        assert_eq!(default_model(Provider::Claude), "sonnet");

        let gemini = models(Provider::Gemini);
        assert_eq!(gemini.first().unwrap().id, "gemini-2.5-flash");
        // No "auto": it can pick an unreachable endpoint.
        assert!(!gemini.iter().any(|m| m.id == "auto"));
    }

    #[test]
    fn validity_is_scoped_to_the_provider() {
        assert!(is_valid(Provider::Claude, "opus"));
        assert!(!is_valid(Provider::Claude, "gemini-2.5-pro"));
        assert!(is_valid(Provider::Gemini, "gemini-2.5-pro"));
        assert!(!is_valid(Provider::Gemini, "opus"));
        assert!(!is_valid(Provider::Claude, "nonsense"));
    }
}
