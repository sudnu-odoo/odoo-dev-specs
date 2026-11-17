//! Command-line options of `specs-mcp`.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};

const USAGE: &str =
    "usage: specs-mcp --session-dir <folder> [--runtime-dir <folder>] [--no-demo-data]";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub session_dir: PathBuf,
    /// Local Odoo runtime folder; screenshots are enabled when set.
    pub runtime_dir: Option<PathBuf>,
    /// Load Odoo demo data into screenshot databases.
    pub demo_data: bool,
}

pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Options> {
    let (mut session_dir, mut runtime_dir, mut demo_data) = (None, None, true);
    let mut args = args.into_iter();
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--session-dir" => session_dir = Some(args.next().context(USAGE)?.into()),
            "--runtime-dir" => runtime_dir = Some(args.next().context(USAGE)?.into()),
            "--no-demo-data" => demo_data = false,
            _ => bail!("unknown option {flag}\n{USAGE}"),
        }
    }
    Ok(Options {
        session_dir: session_dir.context(USAGE)?,
        runtime_dir,
        demo_data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Options> {
        parse_args(args.iter().map(|a| a.to_string()))
    }

    #[test]
    fn session_dir_alone() {
        assert_eq!(
            parse(&["--session-dir", "/s"]).unwrap(),
            Options {
                session_dir: "/s".into(),
                runtime_dir: None,
                demo_data: true
            }
        );
    }

    #[test]
    fn all_options_in_any_order() {
        assert_eq!(
            parse(&[
                "--no-demo-data",
                "--runtime-dir",
                "/rt",
                "--session-dir",
                "/s"
            ])
            .unwrap(),
            Options {
                session_dir: "/s".into(),
                runtime_dir: Some("/rt".into()),
                demo_data: false
            }
        );
    }

    #[test]
    fn mistakes_are_refused() {
        assert!(parse(&[]).is_err(), "session dir is required");
        assert!(parse(&["--session-dir"]).is_err(), "missing value");
        assert!(
            parse(&["--session-dir", "/s", "--odev", "/x"]).is_err(),
            "unknown option"
        );
    }
}
