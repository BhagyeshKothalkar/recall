//! Port for memory retrieval.

use std::fmt;

use crate::domain::{SearchQuery, SearchResult};

/// Failure returned by a memory search implementation.
#[derive(Debug)]
pub enum SearchError {
    /// The retrieval implementation could not complete the search.
    Retrieval(Box<dyn std::error::Error + Send + Sync>),
}

impl fmt::Display for SearchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Retrieval(error) => write!(formatter, "memory search error: {error}"),
        }
    }
}

impl std::error::Error for SearchError {}

/// Retrieval port used by the application.
///
/// Implementations may use SQLite FTS, semantic indexes, or a future hybrid
/// strategy. The application receives domain search results and does not
/// depend on the underlying indexing mechanism.
pub trait MemorySearcher {
    /// Returns up to `query.limit()` relevant memories.
    fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, SearchError>;
}
