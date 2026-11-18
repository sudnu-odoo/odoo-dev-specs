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

/// Every provider, in the order the picker lists them.
pub const PROVIDERS: [Provider; 2] = [Provider::Claude, Provider::Antigravity];

impl Provider {
    /// Stored id, as in a session's `provider` field ("claude", "antigravity").
    pub fn id(self) -> &'static str {
        match self {
            Provider::Claude => "claude",
            Provider::Antigravity => "antigravity",
        }
    }

    pub fn from_id(id: &str) -> Option<Provider> {
        PROVIDERS.into_iter().find(|p| p.id() == id)
    }

    /// Name shown in the picker.
    pub fn display_name(self) -> &'static str {
        match self {
            Provider::Claude => "Claude",
            Provider::Antigravity => "Antigravity",
        }
    }
}

/// Models for a provider, default first. Antigravity ids are those of `agy models`.
pub fn models(provider: Provider) -> Vec<Model> {
    let name = provider.display_name();
    let list: &[(&str, &str)] = match provider {
        Provider::Claude => &[
            ("sonnet", "Sonnet (balanced)"),
            ("opus", "Opus (most capable)"),
            ("haiku", "Haiku (fastest)"),
        ],
        Provider::Antigravity => &[
            ("gemini-3.8-flash-high", "Gemini 3.8 Flash (High)"),
            ("gemini-3.8-flash-medium", "Gemini 3.8 Flash (Medium)"),
            ("gemini-3.8-flash-low", "Gemini 3.8 Flash (Low)"),
            ("claude-sonnet-4-6", "Claude Sonnet 4.6 (Thinking)"),
            ("claude-opus-4-6-thinking", "Claude Opus 4.6 (Thinking)"),
            ("gpt-oss-120b-medium", "GPT-OSS 120B (Medium)"),
        ],
    };
    list.iter()
        .map(|(id, label)| model(name, id, label))
        .collect()
}

/// The models of every provider for one picker. Providers that are not `available`
/// (CLI missing or not logged in) are listed too, but locked.
pub fn all_models(available: impl Fn(Provider) -> bool) -> Vec<Model> {
    PROVIDERS
        .into_iter()
        .flat_map(|provider| {
            let locked = !available(provider);
            models(provider)
                .into_iter()
                .map(move |m| Model { locked, ..m })
        })
        .collect()
}

/// Reasoning effort levels both CLIs accept for `--effort`, lowest first.
pub const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

/// Keep only an effort level the CLIs understand.
pub fn valid_effort(effort: Option<String>) -> Option<String> {
    effort.filter(|e| EFFORTS.contains(&e.as_str()))
}

/// Which provider answers a message. A session belongs to the provider of its first
/// message (one AI per conversation: another one could not resume it), so once it
/// has started, a model of another provider is refused. Before that, the chosen
/// model decides, else `fallback`.
pub fn session_provider(
    session_provider: &str,
    started: bool,
    chosen: Option<Provider>,
    fallback: Provider,
) -> Result<Provider, String> {
    if !started {
        return Ok(chosen.unwrap_or(fallback));
    }
    let own = Provider::from_id(session_provider).unwrap_or(fallback);
    match chosen {
        Some(other) if other != own => Err(format!(
            "This session uses {}. Start a new session to use {}.",
            own.display_name(),
            other.display_name()
        )),
        _ => Ok(own),
    }
}

/// Which provider offers this model id, if any.
pub fn provider_of(id: &str) -> Option<Provider> {
    PROVIDERS.into_iter().find(|p| is_valid(*p, id))
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
    fn all_models_lists_every_provider_and_locks_the_unavailable_ones() {
        let all = all_models(|p| p == Provider::Claude);
        let first = all.first().unwrap();
        assert_eq!((first.id.as_str(), first.locked), ("sonnet", false));
        let agy: Vec<_> = all.iter().filter(|m| m.provider == "Antigravity").collect();
        assert_eq!(agy.len(), 6);
        assert!(agy.iter().all(|m| m.locked));
        let mut ids: Vec<_> = all.iter().map(|m| m.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(
            ids.len(),
            all.len(),
            "model ids must be unique across providers"
        );
    }

    #[test]
    fn only_known_effort_levels_are_kept() {
        assert_eq!(valid_effort(Some("high".into())).as_deref(), Some("high"));
        assert_eq!(valid_effort(Some("turbo".into())), None);
        assert_eq!(valid_effort(None), None);
    }

    #[test]
    fn a_new_session_takes_the_provider_of_the_chosen_model() {
        let p = session_provider(
            "claude",
            false,
            Some(Provider::Antigravity),
            Provider::Claude,
        );
        assert_eq!(p, Ok(Provider::Antigravity));
        assert_eq!(
            session_provider("claude", false, None, Provider::Claude),
            Ok(Provider::Claude)
        );
    }

    #[test]
    fn a_started_session_stays_with_its_provider() {
        assert_eq!(
            session_provider("antigravity", true, None, Provider::Claude),
            Ok(Provider::Antigravity)
        );
        assert_eq!(
            session_provider(
                "antigravity",
                true,
                Some(Provider::Antigravity),
                Provider::Claude
            ),
            Ok(Provider::Antigravity)
        );
        let err = session_provider(
            "claude",
            true,
            Some(Provider::Antigravity),
            Provider::Claude,
        )
        .unwrap_err();
        assert!(err.contains("This session uses Claude"), "{err}");
    }

    #[test]
    fn provider_ids_round_trip() {
        for p in PROVIDERS {
            assert_eq!(Provider::from_id(p.id()), Some(p));
        }
        assert_eq!(Provider::from_id("nope"), None);
    }

    #[test]
    fn a_model_id_identifies_its_provider() {
        assert_eq!(provider_of("opus"), Some(Provider::Claude));
        assert_eq!(
            provider_of("gemini-3.8-flash-high"),
            Some(Provider::Antigravity)
        );
        assert_eq!(provider_of("nonsense"), None);
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
