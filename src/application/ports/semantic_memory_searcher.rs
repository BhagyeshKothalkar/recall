//! Port for embedding-backed semantic retrieval.

use crate::domain::{SearchQuery, SearchResult};

use super::SearchError;

/// Retrieval capability backed by derived semantic embeddings.
pub trait SemanticMemorySearcher {
    /// Searches canonical memories using semantic similarity.
    fn search_semantic(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, SearchError>;
}
