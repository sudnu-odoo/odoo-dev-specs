//! Databases the user manages from the app: a catalog with each database's friendly
//! name and details (next to the servers' state), and the actions on them: create,
//! rename, duplicate, reset, stop, delete and installing missing apps.
//!
//! A database is only ever made fresh or copied from another local one: there is no
//! way in to restore a client's backup, so customer data never reaches the assistant.

use std::time::{SystemTime, UNIX_EPOCH};

use super::*;

const CATALOG: &str = "databases.json";
/// In a session folder: the database the session's screenshots use.
const SESSION_FILE: &str = "odoo.json";
/// Longest wait for a stopped server to let go of its database.
const STOP_TIMEOUT: Duration = Duration::from_secs(10);

/// One database, as the app shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseInfo {
    /// PostgreSQL name, always `specs_*`.
    pub name: String,
    /// The name people see, e.g. "Sales demo".
    pub label: String,
    pub version: String,
    /// Apps asked for (technical names).
    pub modules: Vec<String>,
    pub demo_data: bool,
    pub enterprise: bool,
    /// Made by the assistant for screenshots rather than by a person.
    pub auto: bool,
    /// Unix time in milliseconds.
    pub created_at: u64,
    pub last_used: u64,
    #[serde(default, skip_deserializing)]
    pub size_bytes: u64,
    #[serde(default, skip_deserializing)]
    pub running: bool,
}

/// What the "New database" form asks for.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewDatabase {
    pub label: String,
    pub version: String,
    pub modules: Vec<String>,
    pub demo_data: bool,
    /// Run with the enterprise source (only where this computer has it).
    #[serde(default)]
    pub enterprise: bool,
}

type Catalog = BTreeMap<String, DatabaseInfo>;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn clean_modules(modules: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for m in modules.iter().map(|m| m.trim()).filter(|m| !m.is_empty()) {
        if !out.iter().any(|o| o == m) {
            out.push(m.to_string());
        }
    }
    out
}

/// Databases made for screenshots end in `_demo` when they have demo data; those
/// without a suffix were made before demo data loaded on Odoo 19 and are empty.
fn demo_from_name(name: &str) -> bool {
    name.ends_with("_demo")
}

/// "specs_19_0_sale_demo" -> "19.0", for databases the catalog never saw.
fn version_from_name(name: &str) -> String {
    let mut parts = name.trim_start_matches(DB_PREFIX).split('_');
    match (parts.next(), parts.next()) {
        (Some(major), Some(minor))
            if major.chars().all(|c| c.is_ascii_digit())
                && minor.chars().all(|c| c.is_ascii_digit()) =>
        {
            format!("{major}.{minor}")
        }
        _ => String::new(),
    }
}

/// The database a session's screenshots use; `None` means Auto.
pub fn session_database(session_dir: &Path) -> Option<String> {
    let text = fs::read_to_string(session_dir.join(SESSION_FILE)).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value["database"].as_str().map(String::from)
}

/// Choose the session's database (`None` = Auto).
pub fn set_session_database(session_dir: &Path, database: Option<&str>) -> Result<()> {
    let path = session_dir.join(SESSION_FILE);
    match database {
        None => match fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        },
        Some(name) => {
            let tmp = path.with_extension("json.tmp");
            fs::write(&tmp, serde_json::json!({ "database": name }).to_string())?;
            fs::rename(&tmp, &path).context("saving the session's database")
        }
    }
}

impl Runtime {
    /// Every `specs_*` database with its details, the user's own first, then the
    /// most recently used.
    pub fn list_databases(&self) -> Result<Vec<DatabaseInfo>> {
        let catalog = self.load_catalog();
        let mut list: Vec<DatabaseInfo> = self
            .database_sizes()?
            .into_iter()
            .map(|(name, size)| {
                let mut info = catalog
                    .get(&name)
                    .cloned()
                    .unwrap_or_else(|| self.discover(&name));
                // Older app versions wrote the demo-data flag wrong; the name says it.
                if info.auto && !name.starts_with(&format!("{DB_PREFIX}db_")) {
                    info.demo_data = demo_from_name(&name);
                }
                info.size_bytes = size;
                info.running = self.already_running(&name).is_some();
                info
            })
            .collect();
        list.sort_by(|a, b| a.auto.cmp(&b.auto).then(b.last_used.cmp(&a.last_used)));
        Ok(list)
    }

    /// Details of one database.
    pub fn database_info(&self, name: &str) -> Result<DatabaseInfo> {
        ensure!(
            self.databases()?.iter().any(|n| n == name),
            "The database {name} does not exist any more."
        );
        Ok(self
            .load_catalog()
            .get(name)
            .cloned()
            .unwrap_or_else(|| self.unknown(name)))
    }

    /// A new database with a friendly name, made fresh with the chosen apps.
    pub fn create_database(&self, new: &NewDatabase) -> Result<DatabaseInfo> {
        validate_version(&new.version)?;
        let label = new.label.trim();
        ensure!(!label.is_empty(), "Give the database a name.");
        ensure!(
            !new.enterprise || self.enterprise_available(&new.version),
            "Enterprise is not available for Odoo {} on this computer.",
            new.version
        );
        let _setup = self.setup_lock(&new.version)?;
        let name = self.numbered_name(&new.version)?;
        let modules = clean_modules(&new.modules);
        self.install(&new.version, &name, &modules, new.demo_data, new.enterprise)?;
        let info = DatabaseInfo {
            name: name.clone(),
            label: label.into(),
            version: new.version.clone(),
            modules,
            demo_data: new.demo_data,
            enterprise: new.enterprise,
            auto: false,
            created_at: now_ms(),
            last_used: now_ms(),
            size_bytes: 0,
            running: false,
        };
        self.edit_catalog(|c| {
            c.insert(name, info.clone());
            Ok(())
        })?;
        Ok(info)
    }

    pub fn rename_database(&self, name: &str, label: &str) -> Result<()> {
        let label = label.trim();
        ensure!(!label.is_empty(), "Give the database a name.");
        let mut info = self.database_info(name)?;
        info.label = label.into();
        self.edit_catalog(|c| {
            c.insert(name.into(), info);
            Ok(())
        })
    }

    /// A copy of the database and its files under a new name, e.g. before an experiment.
    pub fn duplicate_database(&self, name: &str, label: &str) -> Result<DatabaseInfo> {
        let label = label.trim();
        ensure!(!label.is_empty(), "Give the copy a name.");
        let source = self.database_info(name)?;
        let _setup = self.setup_lock(&source.version)?;
        // PostgreSQL copies only a database nobody is connected to.
        self.stop_database(name)?;
        let copy = self.numbered_name(&source.version)?;
        self.run_to_end(
            &self.tools.psql,
            &[
                "-d",
                "postgres",
                "-At",
                "-c",
                &format!("CREATE DATABASE \"{copy}\" TEMPLATE \"{name}\""),
            ],
        )
        .context("Could not copy the database.")?;
        let files = self.data_dir().join("filestore");
        if files.join(name).is_dir() {
            copy_dir(&files.join(name), &files.join(&copy))
                .context("Could not copy the database's files.")?;
        }
        let info = DatabaseInfo {
            name: copy.clone(),
            label: label.into(),
            auto: false,
            created_at: now_ms(),
            last_used: now_ms(),
            ..source
        };
        self.edit_catalog(|c| {
            c.insert(copy, info.clone());
            Ok(())
        })?;
        Ok(info)
    }

    /// Back to a fresh database with the same name and apps; everything done in it is lost.
    pub fn reset_database(&self, name: &str) -> Result<()> {
        let info = self.database_info(name)?;
        let _setup = self.setup_lock(&info.version)?;
        self.drop_database(name)?;
        let modules = clean_modules(&info.modules);
        self.install(
            &info.version,
            name,
            &modules,
            info.demo_data,
            info.enterprise,
        )?;
        self.edit_catalog(|c| {
            c.insert(
                name.into(),
                DatabaseInfo {
                    created_at: now_ms(),
                    ..info
                },
            );
            Ok(())
        })
    }

    /// Stop the database's server, if it runs, and wait until it has let go.
    pub fn stop_database(&self, name: &str) -> Result<()> {
        let mut state = self.load_state();
        let Some(entry) = state.servers.remove(name) else {
            return Ok(());
        };
        if let Ok(pid) = libc::pid_t::try_from(entry.pid) {
            // SAFETY: only sends a signal to the process group we created.
            unsafe {
                libc::killpg(pid, libc::SIGTERM);
            }
        }
        self.save_state(&state)?;
        let started = Instant::now();
        while (is_alive(entry.pid) || answers(entry.port)) && started.elapsed() < STOP_TIMEOUT {
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(())
    }

    /// Install the apps the database does not have yet (stops its server first).
    pub fn install_missing(&self, name: &str, modules: &[String]) -> Result<()> {
        let info = self.database_info(name)?;
        let installed = self.installed_modules(name)?;
        let missing: Vec<String> = clean_modules(modules)
            .into_iter()
            .filter(|m| !installed.contains(m))
            .collect();
        if missing.is_empty() {
            return Ok(());
        }
        let _setup = self.setup_lock(&info.version)?;
        self.stop_database(name)?;
        let python = self.ensure_venv(&info.version)?;
        let mut args = self.odoo_args(&info.version, name, info.enterprise);
        args.extend(["-i".into(), missing.join(",")]);
        args.extend(demo_flags(&info.version, info.demo_data));
        args.push("--stop-after-init".into());
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        self.run_to_end(&python, &args).with_context(|| {
            format!(
                "Could not install {} in {}.",
                missing.join(", "),
                info.label
            )
        })?;
        let mut modules = info.modules.clone();
        modules.extend(missing);
        self.edit_catalog(|c| {
            c.insert(
                name.into(),
                DatabaseInfo {
                    modules: clean_modules(&modules),
                    ..info
                },
            );
            Ok(())
        })
    }

    /// The database for a session that has none yet: the most recently used one of
    /// the version, with the apps it lacks installed; or, when the version has none,
    /// a new numbered one ("db-19-1") with those apps.
    pub fn database_for_version(
        &self,
        version: &str,
        modules: &[String],
        demo_data: bool,
    ) -> Result<String> {
        validate_version(version)?;
        let existing = self
            .list_databases()?
            .into_iter()
            .filter(|d| d.version == version && d.demo_data == demo_data)
            .max_by(|a, b| {
                (a.last_used, a.created_at, &a.name).cmp(&(b.last_used, b.created_at, &b.name))
            });
        if let Some(db) = existing {
            self.install_missing(&db.name, modules)?;
            return Ok(db.name);
        }
        let made = self.create_database(&NewDatabase {
            label: self.suggested_label(version)?,
            version: version.into(),
            modules: modules.to_vec(),
            demo_data,
            // Like the assistant's databases always were: enterprise where it is set up.
            enterprise: self.enterprise_available(version),
        })?;
        Ok(made.name)
    }

    /// Note that the database was just used (for the list's order).
    pub fn touch_database(&self, name: &str) -> Result<()> {
        let fallback = self.unknown(name);
        self.edit_catalog(|c| {
            c.entry(name.into()).or_insert(fallback).last_used = now_ms();
            Ok(())
        })
    }

    /// Record a database made for screenshots (no-op when already known).
    pub(super) fn remember_auto(
        &self,
        name: &str,
        version: &str,
        modules: &[String],
        demo_data: bool,
        enterprise: bool,
    ) -> Result<()> {
        let modules = clean_modules(modules);
        let label = if modules.is_empty() {
            format!("Auto {version}")
        } else {
            format!("Auto {version}: {}", modules.join(", "))
        };
        self.edit_catalog(|c| {
            c.entry(name.into()).or_insert(DatabaseInfo {
                name: name.into(),
                label,
                version: version.into(),
                modules,
                demo_data,
                enterprise,
                auto: true,
                created_at: now_ms(),
                last_used: now_ms(),
                size_bytes: 0,
                running: false,
            });
            Ok(())
        })
    }

    pub(super) fn forget(&self, name: &str) -> Result<()> {
        self.edit_catalog(|c| {
            c.remove(name);
            Ok(())
        })
    }

    /// Run `odoo-bin -i` to make the database; a half-made one is removed on failure.
    pub(super) fn install(
        &self,
        version: &str,
        name: &str,
        modules: &[String],
        demo_data: bool,
        enterprise: bool,
    ) -> Result<()> {
        let python = self.ensure_venv(version)?;
        let mods = if modules.is_empty() {
            "base".to_string()
        } else {
            modules.join(",")
        };
        let mut args = self.odoo_args(version, name, enterprise);
        args.extend(["-i".into(), mods.clone()]);
        args.extend(demo_flags(version, demo_data));
        args.push("--stop-after-init".into());
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        if let Err(e) = self.run_to_end(&python, &args) {
            // odoo-bin creates the PostgreSQL database before installing modules:
            // remove the half-made one so it is never taken for a ready database.
            let _ = self.drop_database(name);
            return Err(e.context(format!(
                "Could not create the Odoo {version} database with {mods}."
            )));
        }
        Ok(())
    }

    /// Drop the PostgreSQL database and its files (catalog untouched).
    pub(super) fn drop_database(&self, name: &str) -> Result<()> {
        ensure!(
            name.starts_with(DB_PREFIX),
            "Only databases made by this app can be deleted."
        );
        self.stop_database(name)?;
        self.run_to_end(&self.tools.dropdb, &["--if-exists", name])?;
        let _ = fs::remove_dir_all(self.data_dir().join("filestore").join(name));
        Ok(())
    }

    /// A database the catalog never saw (made by an older app version): its apps are
    /// read once from the database and remembered, so it can be told apart and reset.
    fn discover(&self, name: &str) -> DatabaseInfo {
        let mut info = self.unknown(name);
        if let Ok(apps) = self.installed_apps(name) {
            if !apps.is_empty() {
                info.label = format!("{}: {}", info.label, apps.join(", "));
            }
            info.modules = apps;
            let known = info.clone();
            let _ = self.edit_catalog(|c| {
                c.entry(name.into()).or_insert(known);
                Ok(())
            });
        }
        info
    }

    fn unknown(&self, name: &str) -> DatabaseInfo {
        let version = version_from_name(name);
        DatabaseInfo {
            name: name.into(),
            label: format!("Auto {version}").trim().to_string(),
            enterprise: name.contains("_ee_"),
            demo_data: demo_from_name(name),
            version,
            modules: Vec::new(),
            auto: true,
            created_at: 0,
            last_used: 0,
            size_bytes: 0,
            running: false,
        }
    }

    fn database_sizes(&self) -> Result<Vec<(String, u64)>> {
        let out = self.run_to_end(
            &self.tools.psql,
            &[
                "-d",
                "postgres",
                "-At",
                "-c",
                "SELECT datname, pg_database_size(datname) FROM pg_database",
            ],
        )?;
        Ok(out
            .lines()
            .filter_map(|line| {
                let mut cols = line.trim().split('|');
                let name = cols.next()?.trim();
                let size = cols.next().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
                name.starts_with(DB_PREFIX)
                    .then(|| (name.to_string(), size))
            })
            .collect())
    }

    /// The installed modules that are apps (Sales, Inventory…), not technical ones.
    fn installed_apps(&self, name: &str) -> Result<Vec<String>> {
        let out = self.run_to_end(
            &self.tools.psql,
            &[
                "-d",
                name,
                "-At",
                "-c",
                "SELECT name FROM ir_module_module WHERE state = 'installed' AND application ORDER BY name",
            ],
        )?;
        Ok(out
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect())
    }

    fn installed_modules(&self, name: &str) -> Result<Vec<String>> {
        let out = self.run_to_end(
            &self.tools.psql,
            &[
                "-d",
                name,
                "-At",
                "-c",
                "SELECT name FROM ir_module_module WHERE state = 'installed'",
            ],
        )?;
        Ok(out
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect())
    }

    /// Whether this computer has the enterprise source for the version.
    pub fn enterprise_available(&self, version: &str) -> bool {
        self.enterprise_dir(version).is_some()
    }

    /// Whether the database runs with enterprise: as recorded, or for databases the
    /// catalog never saw, as their name says.
    pub(super) fn runs_enterprise(&self, database: &str) -> bool {
        self.load_catalog()
            .get(database)
            .map_or(database.contains("_ee_"), |info| info.enterprise)
    }

    /// The name the "New database" form suggests: "db-19-1", "db-19-2"…
    pub fn suggested_label(&self, version: &str) -> Result<String> {
        validate_version(version)?;
        let short = version.strip_suffix(".0").unwrap_or(version);
        Ok(format!("db-{short}-{}", self.next_number(version)?))
    }

    /// `specs_db_19_<n>`: numbered per version, after the highest number in use.
    fn numbered_name(&self, version: &str) -> Result<String> {
        Ok(format!(
            "{}{}",
            number_prefix(version),
            self.next_number(version)?
        ))
    }

    fn next_number(&self, version: &str) -> Result<u32> {
        let prefix = number_prefix(version);
        let highest = self
            .databases()?
            .into_iter()
            .chain(self.load_catalog().into_keys())
            .filter_map(|n| n.strip_prefix(&prefix)?.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        Ok(highest + 1)
    }

    fn catalog_path(&self) -> PathBuf {
        self.state_dir().join(CATALOG)
    }

    fn load_catalog(&self) -> Catalog {
        fs::read_to_string(self.catalog_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Change the catalog under a lock shared by the app and the screenshot tool.
    fn edit_catalog<T>(&self, change: impl FnOnce(&mut Catalog) -> Result<T>) -> Result<T> {
        fs::create_dir_all(self.state_dir())?;
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.state_dir().join("catalog.lock"))?;
        lock.lock().context("waiting for the database list")?;
        let mut catalog = self.load_catalog();
        let out = change(&mut catalog)?;
        let tmp = self.catalog_path().with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(&catalog)?)?;
        fs::rename(&tmp, self.catalog_path()).context("saving the database list")?;
        Ok(out)
    }
}

/// "19.0" -> "specs_db_19_", "saas-18.2" -> "specs_db_saas_18_2_".
fn number_prefix(version: &str) -> String {
    let short = version.strip_suffix(".0").unwrap_or(version);
    let short: String = short
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("{DB_PREFIX}db_{short}_")
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
