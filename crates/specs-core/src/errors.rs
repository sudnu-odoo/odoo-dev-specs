//! The assistants' failures, said plainly: what happened and what to do.

/// The message to show for an assistant's failure; unknown ones stay as they are.
pub fn friendly(message: &str) -> String {
    let lower = message.to_lowercase();
    if lower.contains("quota reached") || lower.contains("resource_exhausted") {
        let when = resets_in(message)
            .map(|wait| format!(" It comes back in about {wait}."))
            .unwrap_or_default();
        return format!(
            "This assistant's usage quota is used up for now.{when} Meanwhile, start a new \
session with another assistant (pick its model in the menu)."
        );
    }
    if lower.contains("no capacity available") || lower.contains("code 503") {
        return "The assistant's servers are busy right now. Try again in a minute, or pick \
another model."
            .into();
    }
    if lower.contains("usage limit")
        || lower.contains("hit your limit")
        || lower.contains("rate limit")
    {
        return "Your plan's usage limit is reached for now. Try again later, or start a new \
session with another assistant."
            .into();
    }
    if lower.contains("not logged in")
        || lower.contains("please run /login")
        || lower.contains("invalid api key")
    {
        return "The assistant is logged out. Log in again from the model menu, then resend \
your message."
            .into();
    }
    message.to_string()
}

/// "Resets in 115h11m8s" -> "4 days and 19 hours".
fn resets_in(message: &str) -> Option<String> {
    let rest = &message[message.find("Resets in ")? + "Resets in ".len()..];
    let hours: u64 = rest.split('h').next()?.trim().parse().ok()?;
    Some(match (hours / 24, hours % 24) {
        (0, 0) => "less than an hour".into(),
        (0, h) => format!("{h} hour{}", if h == 1 { "" } else { "s" }),
        (d, 0) => format!("{d} day{}", if d == 1 { "" } else { "s" }),
        (d, h) => format!(
            "{d} day{} and {h} hour{}",
            if d == 1 { "" } else { "s" },
            if h == 1 { "" } else { "s" }
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_used_up_quota_says_when_it_comes_back_and_what_to_do() {
        let raw = "Individual quota reached. Please upgrade your subscription to increase your \
limits. Resets in 115h11m8s.";
        let said = friendly(raw);
        assert!(said.contains("quota is used up"), "{said}");
        assert!(said.contains("about 4 days and 19 hours"), "{said}");
        assert!(said.contains("another assistant"), "{said}");
    }

    #[test]
    fn busy_servers_limits_and_logouts_are_explained() {
        assert!(friendly(
            "API error (attempt 1): UNAVAILABLE (code 503): No capacity available for model gemini-3.8-flash-high"
        )
        .contains("busy right now"));
        assert!(
            friendly("Claude AI usage limit reached|1764000000").contains("usage limit is reached")
        );
        assert!(friendly("Invalid API key · Please run /login").contains("logged out"));
    }

    #[test]
    fn anything_else_is_shown_as_it_is() {
        assert_eq!(
            friendly("The diagram file is missing."),
            "The diagram file is missing."
        );
    }
}
