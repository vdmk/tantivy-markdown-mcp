use std::path::Path;

use anyhow::Result;
use tantivy::schema::*;
use tantivy::{Index as TantivyIdx, ReloadPolicy};

use super::Index;
use super::tantivy_searcher::TantivySearcher;
use super::tantivy_writer::TantivyWriter;

const WRITER_HEAP_SIZE: usize = 50_000_000;

/// Tantivy-backed Markdown index.
pub struct TantivyIndex {
    index: TantivyIdx,
    path_field: Field,
    body_field: Field,
    mtime_field: Field,
    size_field: Field,
}

impl TantivyIndex {
    /// Opens an existing Tantivy index or creates one at `index_path`.
    ///
    /// An incompatible schema is rebuilt in place; callers must repopulate it.
    pub fn open_or_create(index_path: &Path) -> Result<Self> {
        std::fs::create_dir_all(index_path)?;
        let mut b = Schema::builder();
        let path_field = b.add_text_field("path", STRING | STORED);
        let body_field = b.add_text_field("body", TEXT | STORED);
        let mtime_field = b.add_u64_field("mtime", STORED);
        let size_field = b.add_u64_field("size", STORED);
        let schema = b.build();

        let dir = tantivy::directory::MmapDirectory::open(index_path)?;
        let index = match TantivyIdx::open_or_create(dir, schema.clone()) {
            Ok(idx) => idx,
            Err(e) => {
                if matches!(e, tantivy::TantivyError::SchemaError(_)) {
                    // Schema changed — wipe and recreate; index_all will repopulate.
                    eprintln!("[tantivy-markdown-mcp] index schema changed, rebuilding: {e}");
                    std::fs::remove_dir_all(index_path)?;
                    std::fs::create_dir_all(index_path)?;
                    let dir = tantivy::directory::MmapDirectory::open(index_path)?;
                    TantivyIdx::open_or_create(dir, schema)?
                } else {
                    return Err(e.into());
                }
            }
        };

        Ok(Self {
            index,
            path_field,
            body_field,
            mtime_field,
            size_field,
        })
    }
}

impl Index for TantivyIndex {
    type W = TantivyWriter;
    type S = TantivySearcher;

    fn writer(&self, filter: impl Fn(&Path) -> bool + Send + 'static) -> Result<TantivyWriter> {
        let writer = self.index.writer(WRITER_HEAP_SIZE)?;
        Ok(TantivyWriter::new(
            writer,
            self.path_field,
            self.body_field,
            self.mtime_field,
            self.size_field,
            Box::new(filter),
        ))
    }

    fn searcher(&self) -> Result<TantivySearcher> {
        let reader = self
            .index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()?;
        Ok(TantivySearcher::new(
            self.index.clone(),
            reader,
            self.path_field,
            self.body_field,
        ))
    }
}
