//! Whether each AI provider is ready (installed and logged in), and how to log in.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::settings::Provider;

/// Readiness of one provider, shown in Settings and used to lock its models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub provider: Provider,
    /// Display name, e.g. "Antigravity".
    pub name: String,
    pub installed: bool,
    pub logged_in: bool,
    /// The CLI, when installed.
    #[serde(skip)]
    pub program: Option<PathBuf>,
}

impl ProviderStatus {
    pub fn ready(&self) -> bool {
        self.installed && self.logged_in
    }
}

/// Terminals we can open a login in, in order of preference.
pub const TERMINALS: [&str; 7] = [
    "gnome-terminal",
    "ptyxis",
    "kgx",
    "konsole",
    "xfce4-terminal",
    "x-terminal-emulator",
    "xterm",
];

/// Whether `claude auth status --json` reports a login.
pub fn claude_logged_in(status_json: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(status_json)
        .ok()
        .and_then(|v| v["loggedIn"].as_bool())
        .unwrap_or(false)
}

/// The command that logs in to a provider. Antigravity has no login subcommand:
/// its first interactive run asks the user to sign in.
pub fn login_command(provider: Provider, program: &Path) -> Vec<String> {
    let program = program.display().to_string();
    match provider {
        Provider::Claude => vec![program, "auth".into(), "login".into()],
        Provider::Antigravity => vec![program],
    }
}

/// Arguments that make `terminal` run `command` in a new window, which stays open
/// afterwards so the user can read the result. The command is passed as separate
/// arguments to `sh` (never spliced into the script), so paths need no quoting.
pub fn terminal_args(terminal: &Path, command: &[String]) -> Vec<String> {
    let script = r#""$@"; echo; printf 'Done. You can close this window and go back to Odoo Dev Specs.'; read _"#;
    let mut run = vec!["sh".to_string(), "-c".into(), script.into(), "sh".into()];
    run.extend(command.iter().cloned());
    let name = terminal.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let flag = match name {
        "gnome-terminal" | "ptyxis" | "kgx" => "--",
        "xfce4-terminal" => "-x",
        _ => "-e",
    };
    std::iter::once(flag.to_string()).chain(run).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_status_is_read_from_its_json() {
        assert!(claude_logged_in(
            r#"{"loggedIn": true, "authMethod": "claude.ai"}"#
        ));
        assert!(!claude_logged_in(r#"{"loggedIn": false}"#));
        assert!(!claude_logged_in("not json"));
    }

    #[test]
    fn login_commands() {
        assert_eq!(
            login_command(Provider::Claude, Path::new("/bin/claude")),
            ["/bin/claude", "auth", "login"]
        );
        assert_eq!(
            login_command(Provider::Antigravity, Path::new("/bin/agy")),
            ["/bin/agy"]
        );
    }

    #[test]
    fn gnome_terminal_runs_the_command_after_a_double_dash_and_stays_open() {
        let a = terminal_args(
            Path::new("/usr/bin/gnome-terminal"),
            &["/home/u/my bin/agy".to_string()],
        );
        assert_eq!(a[..3], ["--", "sh", "-c"]);
        assert!(a[3].contains("read _"));
        // The path with a space is one argument, not part of the script.
        assert_eq!(a.last().unwrap(), "/home/u/my bin/agy");
    }

    #[test]
    fn other_terminals_use_their_own_flag() {
        let cmd = ["x".to_string()];
        assert_eq!(terminal_args(Path::new("/usr/bin/xterm"), &cmd)[0], "-e");
        assert_eq!(
            terminal_args(Path::new("/usr/bin/xfce4-terminal"), &cmd)[0],
            "-x"
        );
    }

    #[test]
    fn ready_means_installed_and_logged_in() {
        let mut s = ProviderStatus {
            provider: Provider::Claude,
            name: "Claude".into(),
            installed: true,
            logged_in: false,
            program: None,
        };
        assert!(!s.ready());
        s.logged_in = true;
        assert!(s.ready());
    }
}
