//! Retrieval-domain types.
//!
//! These types describe queries and retrieval results without exposing how a
//! search is implemented. In particular, a score belongs to a result, not to
//! canonical [`Memory`](super::memory::Memory) state.

use super::memory::{Memory, MemoryId};

/// A validated query presented to a memory searcher.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchQuery {
    text: String,
    limit: usize,
}

/// Validation failures for a search query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchQueryError {
    /// Query text contains no non-whitespace characters.
    EmptyText,
    /// A search must request at least one result.
    ZeroLimit,
}

impl SearchQuery {
    /// Constructs a search query after validating its required fields.
    pub fn new(text: String, limit: usize) -> Result<Self, SearchQueryError> {
        if text.trim().is_empty() {
            return Err(SearchQueryError::EmptyText);
        }
        if limit == 0 {
            return Err(SearchQueryError::ZeroLimit);
        }

        Ok(Self { text, limit })
    }

    /// Returns the query text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the maximum number of results requested.
    pub const fn limit(&self) -> usize {
        self.limit
    }
}

/// Retrieval score attached to a search result.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct SearchScore(f32);

impl SearchScore {
    /// Creates a score, rejecting NaN because it cannot be ordered reliably.
    pub fn new(value: f32) -> Option<Self> {
        value
            .is_nan()
            .then_some(())
            .map_or(Some(Self(value)), |_| None)
    }

    /// Returns the numeric score.
    pub const fn value(self) -> f32 {
        self.0
    }
}

/// A memory returned by retrieval with its retrieval-specific score.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchResult {
    memory: Memory,
    score: SearchScore,
}

impl SearchResult {
    /// Creates a scored search result.
    pub const fn new(memory: Memory, score: SearchScore) -> Self {
        Self { memory, score }
    }

    /// Returns the retrieved memory.
    pub const fn memory(&self) -> &Memory {
        &self.memory
    }

    /// Returns the retrieval score.
    pub const fn score(&self) -> SearchScore {
        self.score
    }

    /// Returns the identity of the retrieved memory.
    pub const fn memory_id(&self) -> MemoryId {
        self.memory.id()
    }
}

/// Relevance attached to memory selected for generation context.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Relevance(f32);

impl Relevance {
    /// Creates a relevance value, rejecting NaN.
    pub fn new(value: f32) -> Option<Self> {
        if value.is_nan() {
            None
        } else {
            Some(Self(value))
        }
    }

    /// Returns the numeric relevance value.
    pub const fn value(self) -> f32 {
        self.0
    }
}

/// A memory explicitly selected as context for inference.
#[derive(Clone, Debug, PartialEq)]
pub struct RetrievedMemory {
    memory: Memory,
    relevance: Relevance,
}

impl RetrievedMemory {
    /// Creates an inference-context memory from a retrieved canonical memory.
    pub const fn new(memory: Memory, relevance: Relevance) -> Self {
        Self { memory, relevance }
    }

    /// Returns the canonical memory supplied to the inference layer.
    pub const fn memory(&self) -> &Memory {
        &self.memory
    }

    /// Returns the retrieval relevance assigned by the application.
    pub const fn relevance(&self) -> Relevance {
        self.relevance
    }

    /// Returns the source memory identity for provenance.
    pub const fn memory_id(&self) -> MemoryId {
        self.memory.id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_rejects_empty_text() {
        assert_eq!(
            SearchQuery::new("  ".to_owned(), 5),
            Err(SearchQueryError::EmptyText)
        );
    }

    #[test]
    fn query_rejects_zero_limit() {
        assert_eq!(
            SearchQuery::new("rust".to_owned(), 0),
            Err(SearchQueryError::ZeroLimit)
        );
    }

    #[test]
    fn scores_reject_nan() {
        assert!(SearchScore::new(f32::NAN).is_none());
        assert!(Relevance::new(f32::NAN).is_none());
    }
}
