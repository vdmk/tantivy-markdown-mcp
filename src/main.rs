use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Error, Result};
use clap::{Parser, Subcommand};
use tantivy::TantivyError;
use tantivy::directory::error::LockError;
use tantivy_markdown_mcp::index::{Index, TantivyIndex};
use tantivy_markdown_mcp::maintenance::{clear_all_indexes, clear_index, report_index_usage};
use tantivy_markdown_mcp::utils::{data_lock, index_dir};
use tantivy_markdown_mcp::{mcp, watcher};

#[derive(Parser)]
#[command(
    version,
    about = "A local MCP server for searching Markdown knowledge bases"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Serve MCP search requests for a Markdown root.
    Serve { path: Option<PathBuf> },
    /// Remove the persisted index for a Markdown root.
    Clear { path: Option<PathBuf> },
    /// Report persisted index usage.
    IndexUsage,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let (command_name, path_arg) = match cli.command {
        Command::Serve { path } => ("serve", path),
        Command::Clear { path } => ("clear", path),
        Command::IndexUsage => {
            let _data_lock = data_lock(true)?;
            return report_index_usage();
        }
    };

    if command_name == "clear" && path_arg.is_none() {
        return clear_all_indexes();
    }

    let watch_path = path_arg
        .or_else(|| {
            std::env::var_os("CLAUDE_PROJECT_DIR")
                .filter(|path| !path.is_empty())
                .map(PathBuf::from)
        })
        .unwrap_or(std::env::current_dir()?)
        .canonicalize()
        .context("cannot access knowledge base")?;

    let index_dir = index_dir(&watch_path)?;

    if command_name == "clear" {
        return clear_index(&index_dir);
    }

    let _data_lock = data_lock(true)?;

    prepare_private_index_dir(&index_dir)?;
    let index = TantivyIndex::open_or_create(&index_dir).context("failed to open index")?;

    let searcher = index.searcher()?;

    match index.writer(is_indexable) {
        Ok(writer) => {
            watcher::spawn(&watch_path, writer, is_indexable)?;
        }
        Err(error) if is_writer_lock_busy(&error) => {
            eprintln!("[tantivy-markdown-mcp] index locked by another instance, running read-only");
            spawn_retry_thread(index, watch_path.clone());
        }
        Err(error) => return Err(error).context("failed to acquire index writer"),
    }

    mcp::McpServer::new(searcher).run()
}

fn is_indexable(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
        && !path.components().any(|c| c.as_os_str() == ".git")
}

fn prepare_private_index_dir(index_dir: &Path) -> Result<()> {
    let storage_dir = index_dir
        .parent()
        .expect("index directory is nested under app storage");
    std::fs::create_dir_all(storage_dir)
        .with_context(|| format!("cannot create index storage {}", storage_dir.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        std::fs::set_permissions(storage_dir, std::fs::Permissions::from_mode(0o700))
            .with_context(|| format!("cannot restrict index storage {}", storage_dir.display()))?;
    }

    Ok(())
}

fn spawn_retry_thread(index: TantivyIndex, watch_path: PathBuf) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(30));
            match index.writer(is_indexable) {
                Ok(writer) => match watcher::spawn(&watch_path, writer, is_indexable) {
                    Ok(()) => {
                        eprintln!("[tantivy-markdown-mcp] acquired writer lock, now watching");
                        break;
                    }
                    Err(e) => eprintln!("[tantivy-markdown-mcp] watcher spawn failed: {e}"),
                },
                Err(error) if is_writer_lock_busy(&error) => {}
                Err(error) => {
                    eprintln!("[tantivy-markdown-mcp] failed to acquire index writer: {error}");
                    break;
                }
            }
        }
    });
}

fn is_writer_lock_busy(error: &Error) -> bool {
    matches!(
        error.downcast_ref::<TantivyError>(),
        Some(TantivyError::LockFailure(LockError::LockBusy, _))
    )
}
