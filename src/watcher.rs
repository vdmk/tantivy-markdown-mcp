use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Result;
use notify_debouncer_mini::{DebounceEventResult, new_debouncer, notify::RecursiveMode};

use crate::index::Writer;

const DEFAULT_RECONCILIATION_INTERVAL_SECS: u64 = 10 * 60;
const RECONCILIATION_INTERVAL_ENV: &str = "TANTIVY_MARKDOWN_MCP_RECONCILE_SECS";

/// Starts a background watcher with periodic reconciliation.
///
/// The watcher thread runs independently and is not explicitly cancellable.
pub fn spawn<W: Writer + Send + 'static>(
    watch_path: &Path,
    mut writer: W,
    filter: fn(&Path) -> bool,
) -> Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    let watch_path = watch_path.to_path_buf();

    let mut debouncer = new_debouncer(Duration::from_millis(500), tx)?;
    debouncer
        .watcher()
        .watch(&watch_path, RecursiveMode::Recursive)?;
    let reconciliation_interval = reconciliation_interval();

    // Register the watcher before the initial scan. Events that arrive while
    // the scan is running remain queued and are processed by the worker below.
    writer.index_all(&watch_path)?;

    std::thread::spawn(move || {
        let _debouncer = debouncer;
        if catch_unwind(AssertUnwindSafe(|| {
            let mut next_reconciliation = Instant::now() + reconciliation_interval;
            loop {
                let timeout = next_reconciliation.saturating_duration_since(Instant::now());
                match rx.recv_timeout(timeout) {
                    Ok(result) => {
                        handle_events(result, &watch_path, filter, &mut writer);
                        if Instant::now() >= next_reconciliation {
                            if let Err(e) = writer.index_all(&watch_path) {
                                eprintln!("[tantivy-markdown-mcp] periodic rescan error: {e}");
                            }
                            next_reconciliation = Instant::now() + reconciliation_interval;
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        if let Err(e) = writer.index_all(&watch_path) {
                            eprintln!("[tantivy-markdown-mcp] periodic rescan error: {e}");
                        }
                        next_reconciliation = Instant::now() + reconciliation_interval;
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        }))
        .is_err()
        {
            eprintln!("[tantivy-markdown-mcp] watcher thread panicked; restarting");
            std::thread::sleep(Duration::from_secs(1));
            if let Err(e) = spawn(&watch_path, writer, filter) {
                eprintln!("[tantivy-markdown-mcp] watcher restart failed: {e}");
            }
        }
    });

    Ok(())
}

fn reconciliation_interval() -> Duration {
    std::env::var(RECONCILIATION_INTERVAL_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(DEFAULT_RECONCILIATION_INTERVAL_SECS))
}

/// Reindexes individual files and runs a full scan for directory or deletion events.
/// Full scans check metadata and skip unchanged files.
fn handle_events<W: Writer>(
    result: DebounceEventResult,
    watch_path: &Path,
    filter: fn(&Path) -> bool,
    writer: &mut W,
) {
    match result {
        Ok(events) => {
            let needs_rescan = events
                .iter()
                .any(|event| event.path.is_dir() || (!event.path.is_file() && filter(&event.path)));
            if needs_rescan {
                if let Err(e) = writer.index_all(watch_path) {
                    eprintln!("[tantivy-markdown-mcp] rescan error: {e}");
                }
                return;
            }
            for event in events {
                if let Err(e) = writer.reindex_file(&event.path) {
                    eprintln!(
                        "[tantivy-markdown-mcp] reindex error for {}: {e}",
                        event.path.display()
                    );
                }
            }
        }
        Err(e) => eprintln!("[tantivy-markdown-mcp] watch error: {e}"),
    }
}
