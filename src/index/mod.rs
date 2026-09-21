use std::path::Path;

use anyhow::Result;

/// Tantivy implementation of the index abstraction.
pub mod tantivy_index;
mod tantivy_searcher;
mod tantivy_writer;
pub use tantivy_index::TantivyIndex;

/// A document returned by a search.
pub struct SearchResult {
    /// Filesystem path of the matching Markdown file.
    pub path: String,
    /// Tantivy BM25 relevance score.
    pub score: f32,
    /// Text fragment around the match.
    pub snippet: String,
}

/// Opens writers and searchers for an index implementation.
pub trait Index {
    /// Writer type produced by this index.
    type W: Writer;
    /// Searcher type produced by this index.
    type S: Searcher;

    /// Creates a writer that accepts only paths matching `filter`.
    fn writer(&self, filter: impl Fn(&Path) -> bool + Send + 'static) -> Result<Self::W>;
    /// Creates a searcher for the current index contents.
    fn searcher(&self) -> Result<Self::S>;
}

/// Writes files and updates their indexed contents.
pub trait Writer {
    /// Synchronizes indexed entries with eligible files below `path`.
    fn index_all(&mut self, path: &Path) -> Result<()>;
    /// Reindexes one file, or removes it when it is no longer eligible.
    fn reindex_file(&mut self, path: &Path) -> Result<()>;
}

/// Searches indexed documents.
pub trait Searcher {
    /// Searches `query` and returns up to `limit` results with snippets capped at `snippet_size` characters.
    fn search(&self, query: &str, limit: usize, snippet_size: usize) -> Result<Vec<SearchResult>>;
}
