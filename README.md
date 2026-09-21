# Tantivy Markdown MCP

[![CI](https://github.com/vdmk/tantivy-markdown-mcp/actions/workflows/ci.yml/badge.svg)](https://github.com/vdmk/tantivy-markdown-mcp/actions/workflows/ci.yml)

Search your Markdown knowledge base from a coding agent without loading whole files.

This local MCP server keeps an incremental index and returns ranked matches with file paths and snippets. It requires no embeddings or hosted service.

- Local stdio MCP server
- BM25 keyword ranking
- Filesystem watching and periodic reconciliation keep results up to date

I use this MCP with my own knowledge base. I ask my coding agent to call the search tool before handling every request:

```markdown
ALWAYS call `mcp__tantivy-markdown-mcp__search` as the first action before reading,
inspecting, or otherwise handling each request. No exceptions and no fallbacks.
```

This keeps my knowledge base available to my agent while using less context: the model receives ranked snippets instead of reading entire files.

## Install

Prerequisites:

- Rust stable with Cargo (`rustup` is recommended).
- An MCP-compatible client that can launch a local stdio server.

The project is intended for Linux and macOS. Windows support is not validated.

Install the binary into Cargo's binary directory:

```sh
cargo install --path .
```

To install directly from GitHub:

```sh
cargo install --git https://github.com/vdmk/tantivy-markdown-mcp.git --locked
```

Alternatively, build a release binary in the repository:

```sh
cargo build --release
```

The installed command is `tantivy-markdown-mcp`. A release build creates `target/release/tantivy-markdown-mcp`.

For MCP clients using the common `mcpServers` configuration format, use an absolute command path:

```json
{
  "mcpServers": {
    "tantivy-markdown-mcp": {
      "command": "/absolute/path/to/tantivy-markdown-mcp",
      "args": ["serve", "/absolute/path/to/notes"]
    }
  }
}
```

For Claude Code, configure the server once at user scope:

```sh
claude mcp add --scope user tantivy-markdown-mcp \
  /absolute/path/to/tantivy-markdown-mcp/target/release/tantivy-markdown-mcp serve
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and checks.

## Maintenance commands

Clear all persisted indexes:

```sh
tantivy-markdown-mcp clear
```

Clear the persisted index for one knowledge-base root:

```sh
tantivy-markdown-mcp clear /path/to/notes
```

Report total indexed data and usage per stored project:

```sh
tantivy-markdown-mcp index-usage
```

`clear` without a path removes all stored project indexes; with a path, it removes only that project’s index. The `index-usage` command always reports all stored projects and does not accept a path.

Maintenance commands support regular storage directories only; symlinked app-storage or project-index directories are unsupported.

## Scope

The index root is selected in this order:

- The path passed to `serve`.
- `CLAUDE_PROJECT_DIR`, if no path was passed.
- The MCP process's working directory, if neither is set.

Indexing and updates:

- Files with a `.md` extension, regardless of letter case, are indexed recursively. `.git` directories are excluded, and symlink targets are not traversed.
- Files larger than 100 MiB are skipped. If an indexed file grows beyond this limit, it is removed from the index.
- At startup, the server checks file metadata and rereads only changed files. It then watches for filesystem changes and runs a full reconciliation every 10 minutes to recover from missed or reordered events. Set `TANTIVY_MARKDOWN_MCP_RECONCILE_SECS` to a positive number of seconds to change the interval.
- Paths that are not valid UTF-8 are unsupported. Encountering one stops indexing before the existing index is changed.

Choose an intentional project root. A broad directory such as your home directory can contain large dependency or generated-document trees that slow scanning, and its Markdown files may appear in search results.

## Search

The `search` tool accepts a Tantivy/BM25 keyword query. Prefer specific terms or quoted phrases, for example `"decision model"`.

`limit` defaults to 5 (maximum 1,000). `snippet_size` sets the target snippet length; results may be longer to preserve token boundaries. It defaults to 500 characters and accepts values up to 10,000.

## Storage and updates

Indexes are stored under `$XDG_DATA_HOME/tantivy-markdown-mcp` or, when unset, `~/.local/share/tantivy-markdown-mcp`. The serving command may follow symlinks in the app storage directory or a project index directory, but maintenance commands support regular directories only.

After updating the binary, restart your MCP client so the index can be rebuilt if the index schema changes.
