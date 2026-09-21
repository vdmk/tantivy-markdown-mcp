use anyhow::{Context, Result};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::snippet::SnippetGenerator;
use tantivy::{Index as TantivyIdx, IndexReader};

use super::{SearchResult, Searcher};

/// Searcher backed by a Tantivy index reader.
pub struct TantivySearcher {
    index: TantivyIdx,
    reader: IndexReader,
    path_field: Field,
    body_field: Field,
}

impl TantivySearcher {
    /// Creates a searcher from a Tantivy index, reader, and schema fields.
    pub fn new(
        index: TantivyIdx,
        reader: IndexReader,
        path_field: Field,
        body_field: Field,
    ) -> Self {
        Self {
            index,
            reader,
            path_field,
            body_field,
        }
    }
}

impl Searcher for TantivySearcher {
    fn search(
        &self,
        query_str: &str,
        limit: usize,
        snippet_size: usize,
    ) -> Result<Vec<SearchResult>> {
        self.reader.reload()?;
        let searcher = self.reader.searcher();

        let query_parser = QueryParser::for_index(&self.index, vec![self.body_field]);
        let query = query_parser
            .parse_query(query_str)
            .with_context(|| format!("parsing query: {query_str}"))?;

        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit).order_by_score())?;

        let mut snippet_gen = SnippetGenerator::create(&searcher, &query, self.body_field)?;
        snippet_gen.set_max_num_chars(snippet_size);

        let mut results = Vec::new();
        for (score, doc_addr) in top_docs {
            let doc: TantivyDocument = searcher.doc(doc_addr)?;

            let path = doc
                .get_first(self.path_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let snippet = snippet_gen.snippet_from_doc(&doc);
            let fragment = snippet.fragment();

            results.push(SearchResult {
                path,
                score,
                snippet: fragment.to_string(),
            });
        }
        Ok(results)
    }
}
