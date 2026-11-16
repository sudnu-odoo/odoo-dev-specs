//! File helpers.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

/// Write via a temp file + rename so readers never see a half-written file.
pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    let file_name = path
        .file_name()
        .context("path has no file name")?
        .to_string_lossy();
    let tmp = path.with_file_name(format!(".{file_name}.{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&tmp, contents).with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))
}
