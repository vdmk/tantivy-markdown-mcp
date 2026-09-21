# Repository guidance

## Accepted tradeoffs

Informational only; do not treat these as tasks:

- Schema rebuilds may race with other running server processes. Accepted.
- A file may grow past the 100 MiB check while being read. Accepted.
- `clear PATH` requires the project directory to still exist. Accepted.
- Modification-time and size detection can miss same-size content changes with preserved or coarse timestamps. Accepted.
- File-read failures during updates can leave stale indexed content searchable. Accepted.
- The stdio MCP transport processes one JSON-RPC message per line and does not support batch requests. Accepted.
- Watcher and writer-lock retry failures are logged in background threads; lock retries use a fixed 30-second interval, and background threads do not have explicit cancellation or graceful shutdown. Watcher panics are restarted once per failure with a short backoff. Accepted.
- Non-UTF-8 filesystem paths are outside the supported scope and cause indexing to stop before changing the existing index. Accepted.
- The MCP protocol layer uses generic `serde_json::Value` messages rather than typed request and response structures. Accepted for the small local tool surface.
- Filesystem events are committed individually to keep updates promptly searchable and failures isolated; bulk edits may therefore incur extra Tantivy commits. Accepted for the local knowledge-base workload.

## Before publishing

Run `scripts/pre-publish.sh`. It scans the Git history with Gitleaks and checks
Rust dependencies with `cargo audit`.

## TODO

- [ ] Add an integration test for live filesystem watcher updates. While
  the server remains running, verify that creating, editing, and deleting or
  renaming Markdown files changes search results without restarting it.
