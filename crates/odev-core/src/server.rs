//! Fresh local Odoo databases run through odev, for screenshots.
//!
//! Databases are named after version + modules and reused (they never hold
//! customer data). Servers keep running between chat turns; which ones run is
//! remembered in a small JSON state file so later processes find them again.

use std::collections::BTreeMap;
use std::fs;
use std::net::{TcpListener, TcpStream};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

/// Longest wait for a server to answer (odev may first update its virtualenv).
const START_TIMEOUT: Duration = Duration::from_secs(600);
/// PostgreSQL's limit on database names.
const MAX_NAME: usize = 63;
/// How much of odev's output is shown when something fails.
const OUTPUT_TAIL: usize = 1500;

/// A running Odoo server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Running {
    pub database: String,
    pub port: u16,
}

impl Running {
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

/// Database name for an Odoo version and module set, e.g. `specs_19_0_sale_stock`.
pub fn database_name(version: &str, modules: &[String], demo_data: bool) -> String {
    let mut mods: Vec<&str> = modules
        .iter()
        .map(|m| m.trim())
        .filter(|m| !m.is_empty())
        .collect();
    mods.sort_unstable();
    mods.dedup();
    if mods.is_empty() {
        mods.push("base");
    }
    // Demo and no-demo data cannot share a database, so they get different names.
    let suffix = if demo_data { "" } else { "_nodemo" };
    let raw = format!("specs_{version}_{}{suffix}", mods.join("_"));
    let name: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    if name.len() <= MAX_NAME {
        return name;
    }
    // Too long: keep a readable prefix and make it unique with a hash of the full name.
    format!("{}_{:016x}", &name[..MAX_NAME - 17], fnv1a(&name))
}

fn fnv1a(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    })
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct State {
    servers: BTreeMap<String, ServerEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ServerEntry {
    port: u16,
    pid: u32,
}

#[derive(Debug)]
pub struct Odev {
    program: PathBuf,
    state_dir: PathBuf,
}

impl Odev {
    /// `program` is the odev executable; `state_dir` keeps the server list and logs.
    pub fn new(program: impl Into<PathBuf>, state_dir: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            state_dir: state_dir.into(),
        }
    }

    /// `~/.local/state/odoo-dev-specs`
    pub fn default_state_dir() -> PathBuf {
        dirs::state_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("odoo-dev-specs")
    }

    pub fn databases(&self) -> Result<Vec<String>> {
        let out = self.run_to_end(&["list", "-1"])?;
        Ok(out
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('['))
            .map(String::from)
            .collect())
    }

    /// Create the database with these modules unless it exists. Returns its name.
    /// `demo_data` controls whether Odoo's demo records are loaded; demo and
    /// no-demo variants are separate databases.
    pub fn ensure_database(
        &self,
        version: &str,
        modules: &[String],
        demo_data: bool,
    ) -> Result<String> {
        let name = database_name(version, modules, demo_data);
        if self.databases()?.contains(&name) {
            return Ok(name);
        }
        let mods = if modules.is_empty() {
            "base".to_string()
        } else {
            modules.join(",")
        };
        self.pull(version);
        let mut args = vec!["create", "-f", "-V", version, &name, "-i", &mods];
        if !demo_data {
            args.push("--without-demo=all");
        }
        args.push("--stop-after-init");
        let created = self
            .run_to_end(&args)
            .with_context(|| format!("Could not create the Odoo {version} database with {mods}."));
        if let Err(e) = created {
            // odev creates the PostgreSQL database before installing modules: remove the
            // half-made one so it is never taken for a ready database.
            let _ = self.run_to_end(&["delete", "-f", &name]);
            return Err(e);
        }
        Ok(name)
    }

    /// Start the database's server (or reuse the running one) and wait until it answers.
    pub fn ensure_running(&self, version: &str, database: &str) -> Result<Running> {
        let mut state = self.load_state();
        if let Some(entry) = state.servers.get(database) {
            if is_alive(entry.pid) && answers(entry.port) {
                return Ok(Running {
                    database: database.into(),
                    port: entry.port,
                });
            }
        }
        self.pull(version);
        let port = free_port()?;
        fs::create_dir_all(&self.state_dir)?;
        let log = fs::File::create(self.log_file(database)).context("creating the server log")?;
        let mut child = Command::new(&self.program)
            .args([
                "run",
                "-f",
                database,
                "--http-port",
                &port.to_string(),
                "--log-level=warn",
            ])
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log)
            // Own process group: stopping it also stops odoo-bin started by odev.
            .process_group(0)
            .spawn()
            .with_context(|| format!("Could not start {}.", self.program.display()))?;
        state.servers.insert(
            database.into(),
            ServerEntry {
                port,
                pid: child.id(),
            },
        );
        self.save_state(&state)?;

        let started = Instant::now();
        while !answers(port) {
            if let Some(status) = child.try_wait()? {
                bail!(
                    "Odoo stopped while starting ({status}):\n{}",
                    self.log_tail(database)
                );
            }
            if started.elapsed() > START_TIMEOUT {
                bail!(
                    "Odoo did not start within {} s:\n{}",
                    START_TIMEOUT.as_secs(),
                    self.log_tail(database)
                );
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        Ok(Running {
            database: database.into(),
            port,
        })
    }

    /// Stop every server started through this state directory.
    pub fn stop_all(&self) -> Result<()> {
        let state = self.load_state();
        for entry in state.servers.values() {
            if let Ok(pid) = libc::pid_t::try_from(entry.pid) {
                // SAFETY: only sends a signal to the process group we created.
                unsafe {
                    libc::killpg(pid, libc::SIGTERM);
                }
            }
        }
        self.save_state(&State::default())
    }

    pub fn log_file(&self, database: &str) -> PathBuf {
        self.state_dir.join(format!("{database}.log"))
    }

    fn state_file(&self) -> PathBuf {
        self.state_dir.join("servers.json")
    }

    fn load_state(&self) -> State {
        fs::read_to_string(self.state_file())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_state(&self, state: &State) -> Result<()> {
        fs::create_dir_all(&self.state_dir)?;
        let tmp = self.state_file().with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(state)?)?;
        fs::rename(&tmp, self.state_file()).context("saving the server list")
    }

    fn log_tail(&self, database: &str) -> String {
        tail(&fs::read_to_string(self.log_file(database)).unwrap_or_default())
    }

    /// Update the Odoo sources of `version`. odev otherwise asks interactively (once a
    /// week) whether to pull, which would fail without a terminal. Failures are ignored:
    /// offline, odev may still start, and if it then asks, that error is reported.
    fn pull(&self, version: &str) {
        let _ = self.run_to_end(&["pull", "-f", "-V", version]);
    }

    /// Run odev to completion without a terminal, so it can never wait for input.
    fn run_to_end(&self, args: &[&str]) -> Result<String> {
        let out = Command::new(&self.program)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .with_context(|| {
                format!(
                    "Could not run {}. Is odev installed?",
                    self.program.display()
                )
            })?;
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            bail!(
                "odev {} failed:\n{}",
                args.join(" "),
                tail(&format!("{stdout}{stderr}"))
            );
        }
        Ok(stdout)
    }
}

fn tail(text: &str) -> String {
    let text = text.trim();
    let start = text
        .char_indices()
        .rev()
        .nth(OUTPUT_TAIL - 1)
        .map_or(0, |(i, _)| i);
    text[start..].to_string()
}

fn is_alive(pid: u32) -> bool {
    // SAFETY: signal 0 only checks that the process exists.
    libc::pid_t::try_from(pid).is_ok_and(|pid| unsafe { libc::kill(pid, 0) } == 0)
}

fn answers(port: u16) -> bool {
    TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_millis(200)).is_ok()
}

fn free_port() -> Result<u16> {
    Ok(TcpListener::bind(("127.0.0.1", 0))?.local_addr()?.port())
}
