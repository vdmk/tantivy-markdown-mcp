use std::collections::HashMap;
use std::io::ErrorKind;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use std::{fs::File, fs::OpenOptions};

use anyhow::{Context, Result, anyhow};
use tantivy::schema::*;
use tantivy::{IndexWriter, ReloadPolicy};
use walkdir::WalkDir;

use super::Writer;

const MAX_MARKDOWN_SIZE: u64 = 100 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
struct FileMetadata {
    mtime: u64,
    size: u64,
}

/// Writer backed by a Tantivy index writer.
pub struct TantivyWriter {
    writer: IndexWriter,
    path_field: Field,
    body_field: Field,
    mtime_field: Field,
    size_field: Field,
    filter: Box<dyn Fn(&Path) -> bool + Send>,
}

impl TantivyWriter {
    /// Creates a writer that indexes only paths accepted by `filter`.
    pub fn new(
        writer: IndexWriter,
        path_field: Field,
        body_field: Field,
        mtime_field: Field,
        size_field: Field,
        filter: Box<dyn Fn(&Path) -> bool + Send>,
    ) -> Self {
        Self {
            writer,
            path_field,
            body_field,
            mtime_field,
            size_field,
            filter,
        }
    }
}

impl Writer for TantivyWriter {
    fn index_all(&mut self, watch_path: &Path) -> Result<()> {
        let indexed = self.read_indexed_metadata()?;

        let mut disk = HashMap::new();
        for entry in WalkDir::new(watch_path)
            .into_iter()
            .filter_entry(|entry| entry.file_name() != ".git")
        {
            let path = entry?.into_path();
            if path.to_str().is_none() {
                return Err(anyhow!("cannot index non-UTF-8 path {}", path.display()));
            }
            if !is_regular_file(&path) || !(self.filter)(&path) {
                continue;
            }
            if let Some(metadata) =
                file_metadata(&path).filter(|metadata| metadata.size <= MAX_MARKDOWN_SIZE)
            {
                disk.insert(path, metadata);
            }
        }

        for path in indexed.keys() {
            if !disk.contains_key(path) {
                self.delete_by_path(path);
            }
        }

        for (path, metadata) in &disk {
            if indexed.get(path) != Some(metadata) {
                match read_file(path) {
                    Ok(Some(body)) => {
                        if let Err(error) = self.replace_file(path, &body, *metadata) {
                            eprintln!("skipping {}: {error}", path.display());
                        }
                    }
                    Ok(None) => self.delete_by_path(path),
                    Err(error) => eprintln!("skipping {}: {error}", path.display()),
                }
            }
        }

        self.writer.commit()?;
        Ok(())
    }

    fn reindex_file(&mut self, path: &Path) -> Result<()> {
        if path.to_str().is_none() {
            anyhow::bail!("cannot index non-UTF-8 path {}", path.display());
        }
        if !(self.filter)(path) {
            return Ok(());
        }
        if !is_regular_file(path) {
            self.delete_by_path(path);
            self.writer.commit()?;
            return Ok(());
        }
        let Some(metadata) = file_metadata(path) else {
            self.delete_by_path(path);
            self.writer.commit()?;
            return Ok(());
        };
        if metadata.size > MAX_MARKDOWN_SIZE {
            self.delete_by_path(path);
            self.writer.commit()?;
            return Ok(());
        }
        match read_file(path)? {
            Some(body) => self.replace_file(path, &body, metadata)?,
            None => self.delete_by_path(path),
        }
        self.writer.commit()?;
        Ok(())
    }
}

impl TantivyWriter {
    fn read_indexed_metadata(&self) -> Result<HashMap<PathBuf, FileMetadata>> {
        let reader = self
            .writer
            .index()
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()?;
        reader.reload()?;
        let searcher = reader.searcher();

        let mut result = HashMap::new();
        for segment_reader in searcher.segment_readers() {
            let store_reader = segment_reader.get_store_reader(0)?;
            for doc_id in 0..segment_reader.max_doc() {
                if segment_reader.is_deleted(doc_id) {
                    continue;
                }
                let doc: TantivyDocument = store_reader.get(doc_id)?;
                let path = doc
                    .get_first(self.path_field)
                    .and_then(|v| v.as_str())
                    .map(PathBuf::from);
                let mtime = doc
                    .get_first(self.mtime_field)
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let size = doc
                    .get_first(self.size_field)
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                if let Some(path) = path {
                    result.insert(path, FileMetadata { mtime, size });
                }
            }
        }
        Ok(result)
    }

    fn replace_file(&mut self, path: &Path, body: &str, metadata: FileMetadata) -> Result<()> {
        self.delete_by_path(path);

        let mut doc = TantivyDocument::default();
        doc.add_text(
            self.path_field,
            path.to_str().expect("path validated as UTF-8"),
        );
        doc.add_text(self.body_field, body);
        doc.add_u64(self.mtime_field, metadata.mtime);
        doc.add_u64(self.size_field, metadata.size);
        self.writer.add_document(doc)?;
        Ok(())
    }

    fn delete_by_path(&mut self, path: &Path) {
        let path_str = path.to_string_lossy().into_owned();
        let term = Term::from_field_text(self.path_field, &path_str);
        self.writer.delete_term(term);
    }
}

fn read_file(path: &Path) -> Result<Option<String>> {
    let mut file = match open_for_read(path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("reading {}", path.display())),
    };
    let mut body = String::new();
    match file.read_to_string(&mut body) {
        Ok(_) => Ok(Some(body)),
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
    }
}

// Use an explicit open so Unix can reject a final-component symlink between
// metadata validation and reading. `std::fs::read_to_string` follows symlinks.
#[cfg(unix)]
fn open_for_read(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .read(true)
        // Keep the path check and content read from being bypassed by swapping
        // the final component for a symlink between those operations.
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
}

#[cfg(not(unix))]
fn open_for_read(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().read(true).open(path)
}

fn file_metadata(path: &Path) -> Option<FileMetadata> {
    let metadata = path.metadata().ok()?;
    let mtime = metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_nanos() as u64)?;
    Some(FileMetadata {
        mtime,
        size: metadata.len(),
    })
}

fn is_regular_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
}
