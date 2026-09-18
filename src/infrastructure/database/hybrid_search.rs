//! Hybrid lexical/semantic retrieval using reciprocal rank fusion.

use std::fmt;

use crate::{
    application::ports::{MemorySearcher, SearchError},
    domain::{SearchQuery, SearchResult},
};

use super::{SqliteMemorySearcher, SqliteSemanticMemorySearcher};

/// Combines lexical and semantic candidates while keeping both implementations replaceable.
pub struct SqliteHybridMemorySearcher<L, S> {
    lexical: L,
    semantic: S,
}

impl<L, S> SqliteHybridMemorySearcher<L, S> {
    /// Creates a hybrid searcher from two independent retrieval strategies.
    pub const fn new(lexical: L, semantic: S) -> Self {
        Self { lexical, semantic }
    }
}

impl<L, S> MemorySearcher for SqliteHybridMemorySearcher<L, S>
where
    L: MemorySearcher,
    S: crate::application::ports::SemanticMemorySearcher,
{
    fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, SearchError> {
        let lexical = self.lexical.search(query)?;
        // Semantic retrieval is derived intelligence. If it is unavailable,
        // lexical retrieval remains a valid answer path.
        let semantic = self.semantic.search_semantic(query).unwrap_or_default();
        if semantic.is_empty() {
            return Ok(lexical);
        }
        Ok(crate::application::retrieval::merge_results(
            lexical,
            semantic,
            query.limit(),
        ))
    }
}

impl<L: fmt::Debug, S: fmt::Debug> fmt::Debug for SqliteHybridMemorySearcher<L, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqliteHybridMemorySearcher")
            .field("lexical", &self.lexical)
            .field("semantic", &self.semantic)
            .finish()
    }
}

// Type aliases keep the composition root readable.
pub type HybridWithOllama = SqliteHybridMemorySearcher<
    SqliteMemorySearcher,
    SqliteSemanticMemorySearcher<std::sync::Arc<crate::infrastructure::inference::OllamaBackend>>,
>;
