//! Fresh local Odoo databases for screenshots, run with `odoo-bin` directly.
//!
//! Everything lives in one runtime folder, independent of developer tools:
//!
//! ```text
//! <root>/sources/<version>/odoo   Odoo source (shallow clone of the public repo)
//! <root>/venvs/<version>          Python virtualenv with Odoo's requirements
//! <root>/data                     Odoo data dir (filestores)
//! <root>/state                    running servers and their logs
//! ```
//!
//! Databases are named after version + modules (`specs_19_0_sale`) and reused;
//! they never hold customer data. Only `specs_*` databases are ever listed or deleted.
//! Servers keep running between chat turns and are found again via a state file.

use std::collections::BTreeMap;
use std::fs;
use std::net::{TcpListener, TcpStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};

use crate::setup::adjust_requirements;

mod databases;
pub use databases::{session_database, set_session_database, DatabaseInfo, NewDatabase};

/// Public community repository; enterprise is configured separately.
pub const ODOO_GIT_URL: &str = "https://github.com/odoo/odoo.git";
/// Prefix of every database the runtime owns.
pub const DB_PREFIX: &str = "specs_";
/// Longest wait for a server to answer.
const START_TIMEOUT: Duration = Duration::from_secs(600);
/// PostgreSQL's limit on database names.
const MAX_NAME: usize = 63;
/// How much of a tool's output is shown when something fails.
const OUTPUT_TAIL: usize = 1500;
/// Written once a virtualenv has all its requirements installed.
/// Marks a complete Python environment. Versioned: environments built before
/// Python ran isolated (and might lack packages) carry the old name and are rebuilt.
pub const VENV_READY: &str = ".requirements-installed-v2";

/// Modules Odoo needs at startup; checked after installing, so a broken environment
/// fails here with a clear message instead of when the server starts.
const CORE_IMPORTS: &str = "import babel, lxml, psycopg2, werkzeug, PIL";

/// Variables through which the launching shell would change what Python sees.
const PYTHON_ENV: [&str; 5] = [
    "PYTHONPATH",
    "PYTHONHOME",
    "PYTHONUSERBASE",
    "PYTHONSTARTUP",
    "VIRTUAL_ENV",
];

/// A command whose Python sees only its own environment, whoever launched the app:
/// no inherited search paths and no user site-packages.
fn isolated(program: &Path) -> Command {
    let mut cmd = Command::new(program);
    for var in PYTHON_ENV {
        cmd.env_remove(var);
    }
    cmd.env("PYTHONNOUSERSITE", "1");
    cmd
}

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

/// External programs the runtime calls (swappable in tests).
#[derive(Debug, Clone)]
pub struct Tools {
    pub git: PathBuf,
    pub python: PathBuf,
    pub psql: PathBuf,
    pub dropdb: PathBuf,
}

impl Default for Tools {
    /// The programs found on `PATH`.
    fn default() -> Self {
        Self {
            git: "git".into(),
            python: "python3".into(),
            psql: "psql".into(),
            dropdb: "dropdb".into(),
        }
    }
}

/// odoo-bin flags that load (or skip) demo data. Odoo 19 turned demo data off by
/// default and added `--with-demo`; older versions load it unless told not to.
pub fn demo_flags(version: &str, demo_data: bool) -> Vec<String> {
    if !demo_data {
        return vec!["--without-demo=all".into()];
    }
    let major: u32 = version
        .trim_start_matches("saas-")
        .split('.')
        .next()
        .and_then(|m| m.parse().ok())
        .unwrap_or(0);
    if major >= 19 {
        vec!["--with-demo".into()]
    } else {
        vec![]
    }
}

/// Database name for an Odoo version and module set, e.g. `specs_19_0_sale_stock`.
pub fn database_name(
    version: &str,
    modules: &[String],
    demo_data: bool,
    enterprise: bool,
) -> String {
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
    // ("_demo": databases made before demo data was really loaded on Odoo 19 had no
    // suffix and are empty, so they are never reused.)
    let suffix = if demo_data { "_demo" } else { "_nodemo" };
    // Enterprise installs more modules (e.g. web_enterprise): its own databases.
    let edition = if enterprise { "_ee" } else { "" };
    let raw = format!("{DB_PREFIX}{version}_{}{edition}{suffix}", mods.join("_"));
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

/// An Odoo version as a branch name: `17.0`, `18.0`, `19.0`, `saas-18.2`…
fn validate_version(version: &str) -> Result<()> {
    let number = version.strip_prefix("saas-").unwrap_or(version);
    let ok = number.split_once('.').is_some_and(|(major, minor)| {
        !major.is_empty()
            && !minor.is_empty()
            && major.chars().all(|c| c.is_ascii_digit())
            && minor.chars().all(|c| c.is_ascii_digit())
    });
    ensure!(ok, "\"{version}\" is not an Odoo version (use e.g. 19.0).");
    Ok(())
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

#[derive(Debug, Clone)]
pub struct Runtime {
    root: PathBuf,
    tools: Tools,
    odoo_git_url: String,
    /// Optional enterprise source folder per version.
    enterprise: BTreeMap<String, PathBuf>,
}

impl Runtime {
    pub fn new(root: impl Into<PathBuf>, tools: Tools) -> Self {
        Self {
            root: root.into(),
            tools,
            odoo_git_url: ODOO_GIT_URL.into(),
            enterprise: BTreeMap::new(),
        }
    }

    /// `~/.local/share/odoo-dev-specs/runtime`
    pub fn default_root() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("odoo-dev-specs")
            .join("runtime")
    }

    pub fn with_odoo_git_url(mut self, url: impl Into<String>) -> Self {
        self.odoo_git_url = url.into();
        self
    }

    /// Add an enterprise source folder to the addons path of `version`.
    pub fn with_enterprise(mut self, version: &str, dir: impl Into<PathBuf>) -> Self {
        self.enterprise.insert(version.into(), dir.into());
        self
    }

    pub fn tools(&self) -> &Tools {
        &self.tools
    }

    /// Enterprise source of `version`: the one configured, or an `enterprise` folder
    /// (or link) next to the community source, `sources/<version>/enterprise`.
    pub fn enterprise_dir(&self, version: &str) -> Option<PathBuf> {
        if let Some(dir) = self.enterprise.get(version) {
            return Some(dir.clone());
        }
        let beside = self.root.join("sources").join(version).join("enterprise");
        beside.is_dir().then_some(beside)
    }

    pub fn source_dir(&self, version: &str) -> PathBuf {
        self.root.join("sources").join(version).join("odoo")
    }

    pub fn data_dir(&self) -> PathBuf {
        self.root.join("data")
    }

    pub fn venv_dir(&self, version: &str) -> PathBuf {
        self.root.join("venvs").join(version)
    }

    fn state_dir(&self) -> PathBuf {
        self.root.join("state")
    }

    pub fn log_file(&self, database: &str) -> PathBuf {
        self.state_dir().join(format!("{database}.log"))
    }

    /// Source folders already downloaded, for the assistant to read.
    pub fn source_dirs(&self) -> Vec<PathBuf> {
        let mut dirs: Vec<PathBuf> = fs::read_dir(self.root.join("sources"))
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().join("odoo").join("odoo-bin").is_file())
            .flat_map(|e| {
                let version = e.file_name().to_string_lossy().into_owned();
                std::iter::once(e.path().join("odoo")).chain(self.enterprise_dir(&version))
            })
            .collect();
        dirs.sort();
        dirs
    }

    /// Download the Odoo source of `version` unless it is already there.
    pub fn ensure_source(&self, version: &str) -> Result<PathBuf> {
        validate_version(version)?;
        let dir = self.source_dir(version);
        if dir.join("odoo-bin").is_file() {
            return Ok(dir);
        }
        // A previous attempt may have stopped halfway.
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.parent().expect("source dir has a parent"))?;
        let target = dir.display().to_string();
        let cloned = self.run_to_end(
            &self.tools.git,
            &[
                "clone",
                "--depth",
                "1",
                "--branch",
                version,
                &self.odoo_git_url,
                &target,
            ],
        );
        if let Err(e) = cloned {
            let _ = fs::remove_dir_all(&dir);
            return Err(e.context(format!("Could not download the Odoo {version} source.")));
        }
        Ok(dir)
    }

    /// Create the virtualenv of `version` with Odoo's requirements unless it exists.
    /// Returns its Python interpreter.
    pub fn ensure_venv(&self, version: &str) -> Result<PathBuf> {
        let source = self.ensure_source(version)?;
        let venv = self.venv_dir(version);
        let python = venv.join("bin").join("python");
        if venv.join(VENV_READY).is_file() {
            return Ok(python);
        }
        let _ = fs::remove_dir_all(&venv);
        fs::create_dir_all(venv.parent().expect("venv dir has a parent"))?;
        let venv_arg = venv.display().to_string();
        self.run_to_end(&self.tools.python, &["-m", "venv", &venv_arg])
            .with_context(|| {
                format!("Could not create the Python environment for Odoo {version}.")
            })?;

        let requirements = fs::read_to_string(source.join("requirements.txt"))
            .context("The Odoo source has no requirements.txt.")?;
        let adjusted = venv.join("requirements.txt");
        fs::write(&adjusted, adjust_requirements(&requirements))?;
        let adjusted_arg = adjusted.display().to_string();
        self.run_to_end(
            &python,
            &["-m", "pip", "install", "--quiet", "-r", &adjusted_arg],
        )
        .with_context(|| format!("Could not install the Python packages for Odoo {version}."))?;
        self.run_to_end(&python, &["-c", CORE_IMPORTS])
            .with_context(|| format!("The Python packages for Odoo {version} are incomplete."))?;
        fs::write(venv.join(VENV_READY), "")?;
        Ok(python)
    }

    /// The runtime's own databases (`specs_*`).
    pub fn databases(&self) -> Result<Vec<String>> {
        let out = self.run_to_end(
            &self.tools.psql,
            &[
                "-d",
                "postgres",
                "-At",
                "-c",
                "SELECT datname FROM pg_database",
            ],
        )?;
        Ok(out
            .lines()
            .map(str::trim)
            .filter(|n| n.starts_with(DB_PREFIX))
            .map(String::from)
            .collect())
    }

    /// Create the database with these modules unless it exists. Returns its name.
    /// `demo_data` loads Odoo's demo records; demo and no-demo are separate databases.
    pub fn ensure_database(
        &self,
        version: &str,
        modules: &[String],
        demo_data: bool,
    ) -> Result<String> {
        validate_version(version)?;
        // Several assistants (sessions, providers) can ask at the same moment: one
        // at a time per version sets up the source, Python environment and database,
        // so none sees a half-installed environment or a database still being built.
        let _setup = self.setup_lock(version)?;
        let enterprise = self.enterprise_dir(version);
        let name = database_name(version, modules, demo_data, enterprise.is_some());
        if !self.databases()?.contains(&name) {
            self.install(version, &name, modules, demo_data)?;
        }
        self.remember_auto(&name, version, modules, demo_data, enterprise.is_some())?;
        Ok(name)
    }

    /// Drop one of the runtime's databases and its files, and forget it.
    pub fn delete_database(&self, name: &str) -> Result<()> {
        self.drop_database(name)?;
        self.forget(name)
    }

    /// Start the database's server (or reuse the running one) and wait until it answers.
    pub fn ensure_running(&self, version: &str, database: &str) -> Result<Running> {
        validate_version(version)?;
        if let Some(running) = self.already_running(database) {
            return Ok(running);
        }
        // Another process may be starting the same server: wait, then look again.
        let _setup = self.setup_lock(version)?;
        if let Some(running) = self.already_running(database) {
            return Ok(running);
        }
        let mut state = self.load_state();
        let python = self.ensure_venv(version)?;
        let port = free_port()?;
        let mut args = self.odoo_args(version, database);
        args.extend(["--http-port".into(), port.to_string()]);

        fs::create_dir_all(self.state_dir())?;
        let log = fs::File::create(self.log_file(database)).context("creating the server log")?;
        let mut child = isolated(&python)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log)
            // Own process group, so stopping it also stops Odoo's worker processes.
            .process_group(0)
            .spawn()
            .with_context(|| format!("Could not start Odoo {version}."))?;
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

    /// Stop every server started from this runtime folder.
    pub fn stop_all(&self) -> Result<()> {
        for entry in self.load_state().servers.values() {
            if let Ok(pid) = libc::pid_t::try_from(entry.pid) {
                // SAFETY: only sends a signal to the process group we created.
                unsafe {
                    libc::killpg(pid, libc::SIGTERM);
                }
            }
        }
        self.save_state(&State::default())
    }

    /// Common `odoo-bin` arguments: program, database, addons path, data dir.
    fn odoo_args(&self, version: &str, database: &str) -> Vec<String> {
        let source = self.source_dir(version);
        let mut addons: Vec<String> = Vec::new();
        if let Some(enterprise) = self.enterprise_dir(version) {
            addons.push(enterprise.display().to_string());
        }
        addons.push(source.join("addons").display().to_string());
        vec![
            source.join("odoo-bin").display().to_string(),
            "-d".into(),
            database.into(),
            "--addons-path".into(),
            addons.join(","),
            "--data-dir".into(),
            self.data_dir().display().to_string(),
            "--log-level=warn".into(),
        ]
    }

    /// The database's server, if it is recorded, alive and answering.
    fn already_running(&self, database: &str) -> Option<Running> {
        let state = self.load_state();
        let entry = state.servers.get(database)?;
        (is_alive(entry.pid) && answers(entry.port)).then(|| Running {
            database: database.into(),
            port: entry.port,
        })
    }

    /// Exclusive lock on setting up one Odoo version, shared by every process using
    /// this runtime folder. Released when the returned file is dropped.
    fn setup_lock(&self, version: &str) -> Result<fs::File> {
        fs::create_dir_all(self.state_dir())?;
        let path = self.state_dir().join(format!("setup-{version}.lock"));
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .with_context(|| format!("opening {}", path.display()))?;
        file.lock()
            .with_context(|| format!("waiting for {}", path.display()))?;
        Ok(file)
    }

    fn state_file(&self) -> PathBuf {
        self.state_dir().join("servers.json")
    }

    fn load_state(&self) -> State {
        fs::read_to_string(self.state_file())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_state(&self, state: &State) -> Result<()> {
        fs::create_dir_all(self.state_dir())?;
        let tmp = self.state_file().with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(state)?)?;
        fs::rename(&tmp, self.state_file()).context("saving the server list")
    }

    fn log_tail(&self, database: &str) -> String {
        tail(&fs::read_to_string(self.log_file(database)).unwrap_or_default())
    }

    /// Run a program to completion without a terminal, so it can never wait for input.
    fn run_to_end(&self, program: &Path, args: &[&str]) -> Result<String> {
        let out = isolated(program)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .with_context(|| format!("Could not run {}. Is it installed?", program.display()))?;
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            bail!(
                "{} failed:\n{}",
                program.display(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_names_are_stable_and_readable() {
        let a = database_name(
            "19.0",
            &["stock".into(), "sale_management".into()],
            true,
            false,
        );
        let b = database_name(
            "19.0",
            &["sale_management".into(), "stock".into(), "stock".into()],
            true,
            false,
        );
        assert_eq!(a, b, "order and duplicates do not matter");
        assert_eq!(a, "specs_19_0_sale_management_stock_demo");
        assert_eq!(
            database_name("18.0", &[], true, false),
            "specs_18_0_base_demo"
        );
        assert_eq!(
            database_name("18.0", &[], false, false),
            "specs_18_0_base_nodemo"
        );
        assert_eq!(
            database_name("19.0", &[], true, true),
            "specs_19_0_base_ee_demo"
        );
        let many: Vec<String> = (0..20).map(|i| format!("module_number_{i}")).collect();
        let long = database_name("19.0", &many, true, false);
        assert!(long.len() <= MAX_NAME, "PostgreSQL limit: {long}");
        assert_ne!(long, database_name("19.0", &many[..19], true, false));
    }

    #[test]
    fn demo_data_flags_follow_the_odoo_version() {
        // Odoo 19 stopped loading demo data by default: it must be asked for.
        assert_eq!(demo_flags("19.0", true), ["--with-demo"]);
        assert_eq!(demo_flags("saas-19.2", true), ["--with-demo"]);
        // Before 19 demo data was the default.
        assert!(demo_flags("18.0", true).is_empty());
        assert!(demo_flags("17.0", true).is_empty());
        for v in ["17.0", "18.0", "19.0"] {
            assert_eq!(demo_flags(v, false), ["--without-demo=all"]);
        }
    }

    #[test]
    fn versions_must_look_like_odoo_branches() {
        for ok in ["17.0", "19.0", "saas-18.2"] {
            assert!(validate_version(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "19",
            "19.",
            "v19.0",
            "--upload-pack=x",
            "19.0;x",
            "../19.0",
        ] {
            assert!(validate_version(bad).is_err(), "{bad}");
        }
    }
}
