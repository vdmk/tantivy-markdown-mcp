use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use fs4::fs_std::FileExt;
use sha2::{Digest, Sha256};

const MAX_INDEX_DIR_NAME_BYTES: usize = 240;

/// Returns the persistent index directory for a watched path.
pub fn index_dir(watch_path: &Path) -> Result<PathBuf> {
    Ok(data_home(std::env::var_os("XDG_DATA_HOME"))?
        .join("tantivy-markdown-mcp")
        .join(index_dir_name(watch_path)))
}

/// Opens and acquires the shared or exclusive application data lock.
pub fn data_lock(shared: bool) -> Result<File> {
    let data_home = data_home(std::env::var_os("XDG_DATA_HOME"))?;
    std::fs::create_dir_all(&data_home)?;
    let lock_path = data_home.join("tantivy-markdown-mcp.lock");
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .with_context(|| format!("cannot open data lock {}", lock_path.display()))?;

    if shared {
        lock.lock_shared()?;
    } else if !lock.try_lock_exclusive()? {
        anyhow::bail!("tantivy-markdown-mcp is currently running");
    }
    Ok(lock)
}

/// Resolves the application data directory from `XDG_DATA_HOME` or the home directory.
pub fn data_home(xdg_data_home: Option<std::ffi::OsString>) -> Result<PathBuf> {
    if let Some(path) = xdg_data_home
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        return Ok(path);
    }

    Ok(home::home_dir()
        .context("cannot determine home directory")?
        .join(".local/share"))
}

fn index_dir_name(watch_path: &Path) -> String {
    let root = watch_path.to_string_lossy();
    let hash = Sha256::digest(root.as_bytes());
    let suffix = format!("-{hash:x}");
    let readable = root.replace('/', "_");
    let readable_limit = MAX_INDEX_DIR_NAME_BYTES - suffix.len();
    format!("{}{suffix}", truncate_to_bytes(&readable, readable_limit))
}

fn truncate_to_bytes(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }

    let end = value
        .char_indices()
        .take_while(|(index, _)| *index <= max_bytes)
        .map(|(index, _)| index)
        .last()
        .unwrap_or(0);
    &value[..end]
}
