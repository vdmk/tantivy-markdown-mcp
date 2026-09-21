//! Indexes Markdown knowledge bases and exposes BM25 MCP search tools.

#![warn(missing_docs)]

/// Tantivy-backed indexing and search abstractions.
pub mod index;
/// Commands for clearing indexes and reporting storage usage.
pub mod maintenance;
/// JSON-RPC server implementation for the MCP transport.
pub mod mcp;
/// MCP tool schemas and dispatch.
pub mod tools;
/// Shared filesystem and storage helpers.
pub mod utils;
/// Filesystem watching and incremental index updates.
pub mod watcher;
