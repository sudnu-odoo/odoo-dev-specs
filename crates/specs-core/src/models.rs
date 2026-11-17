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
    /// Provider name, used to group the picker.
    pub provider: String,
    /// The provider is not logged in, so the model cannot be chosen yet.
    pub locked: bool,
}

fn model(provider: &str, id: &str, label: &str) -> Model {
    Model {
        id: id.into(),
        label: label.into(),
        provider: provider.into(),
        locked: false,
    }
}

/// Models for a provider, default first.
pub fn models(provider: Provider) -> Vec<Model> {
    match provider {
        Provider::Claude => vec![
            model("Claude", "sonnet", "Sonnet (balanced)"),
            model("Claude", "opus", "Opus (most capable)"),
            model("Claude", "haiku", "Haiku (fastest)"),
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
    }

    #[test]
    fn models_carry_their_provider_and_start_unlocked() {
        let m = &models(Provider::Claude)[0];
        assert_eq!(m.provider, "Claude");
        assert!(!m.locked);
        let json = serde_json::to_value(m).unwrap();
        assert_eq!(json["provider"], "Claude");
        assert_eq!(json["locked"], false);
    }

    #[test]
    fn validity_is_scoped_to_the_provider() {
        assert!(is_valid(Provider::Claude, "opus"));
        assert!(!is_valid(Provider::Claude, "nonsense"));
    }
}
