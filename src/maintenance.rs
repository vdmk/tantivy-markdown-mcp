use std::path::Path;

use crate::utils::data_home;
use anyhow::{Context, Result};

/// Removes every stored project index.
pub fn clear_all_indexes() -> Result<()> {
    let _data_lock = crate::utils::data_lock(false)?;
    let indexes_dir = data_home(std::env::var_os("XDG_DATA_HOME"))?.join("tantivy-markdown-mcp");
    if indexes_dir.exists() {
        std::fs::remove_dir_all(&indexes_dir)
            .with_context(|| format!("cannot clear indexes {}", indexes_dir.display()))?;
        println!("Cleared all indexes: {}", indexes_dir.display());
    } else {
        println!("All indexes already clear: {}", indexes_dir.display());
    }
    Ok(())
}
/// Removes an existing project index after verifying its canonical location.
///
/// Missing paths are ignored.
pub fn clear_index(index_dir: &Path) -> Result<()> {
    let indexes_dir = data_home(std::env::var_os("XDG_DATA_HOME"))?.join("tantivy-markdown-mcp");
    let _data_lock = crate::utils::data_lock(false)?;
    match std::fs::symlink_metadata(index_dir) {
        Ok(_) => {
            let canonical_indexes_dir = indexes_dir.canonicalize().with_context(|| {
                format!("cannot resolve index directory {}", indexes_dir.display())
            })?;
            let canonical_index_dir = index_dir
                .canonicalize()
                .with_context(|| format!("cannot resolve index {}", index_dir.display()))?;
            if canonical_index_dir.parent() != Some(canonical_indexes_dir.as_path()) {
                anyhow::bail!(
                    "refusing to clear an index outside {}",
                    canonical_indexes_dir.display()
                );
            }

            std::fs::remove_dir_all(index_dir)
                .with_context(|| format!("cannot clear index {}", index_dir.display()))?;
            println!("Cleared index: {}", index_dir.display());
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // The CLI derives this path from the project root; a missing index is already clear.
            println!("Index already clear: {}", index_dir.display());
        }
        Err(error) => {
            return Err(error)
                .with_context(|| format!("cannot inspect index {}", index_dir.display()));
        }
    }
    Ok(())
}

/// Prints stored index usage.
pub fn report_index_usage() -> Result<()> {
    let indexes_dir = data_home(std::env::var_os("XDG_DATA_HOME"))?.join("tantivy-markdown-mcp");
    let mut projects = std::fs::read_dir(&indexes_dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    projects.sort_by_key(|entry| entry.file_name());

    let mut total = 0;
    println!("Indexed projects: {}", projects.len());
    for project in projects {
        let bytes = directory_size(&project.path())?;
        total += bytes;
        println!(
            "  {}: {} ({bytes} bytes)",
            project.file_name().to_string_lossy(),
            format_bytes(bytes)
        );
    }
    println!(
        "Total indexed data: {} ({total} bytes)",
        format_bytes(total)
    );
    Ok(())
}

fn directory_size(path: &Path) -> Result<u64> {
    if !path.exists() {
        return Ok(0);
    }
    let mut total = 0;
    for entry in walkdir::WalkDir::new(path) {
        let entry = entry?;
        if entry.file_type().is_file() {
            total += entry.metadata()?.len();
        }
    }
    Ok(total)
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
