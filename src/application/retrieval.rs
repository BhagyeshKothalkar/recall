//! Application-level hybrid retrieval ranking.

use std::collections::HashMap;

use crate::domain::{SearchResult, SearchScore};

/// Combines lexical and semantic candidate sets using reciprocal rank fusion.
pub(crate) fn merge_results(
    lexical: Vec<SearchResult>,
    semantic: Vec<SearchResult>,
    limit: usize,
) -> Vec<SearchResult> {
    let mut scores = HashMap::new();
    let mut memories = HashMap::new();
    for (rank, result) in lexical.into_iter().enumerate() {
        memories.insert(result.memory_id(), result.clone());
        *scores.entry(result.memory_id()).or_insert(0.0) += 1.0 / (60.0 + rank as f32 + 1.0);
    }
    for (rank, result) in semantic.into_iter().enumerate() {
        memories.entry(result.memory_id()).or_insert_with(|| result.clone());
        *scores.entry(result.memory_id()).or_insert(0.0) += 1.0 / (60.0 + rank as f32 + 1.0);
    }
    let mut ids: Vec<_> = scores.into_iter().collect();
    ids.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    ids.into_iter().take(limit).filter_map(|(id, score)| {
        memories.remove(&id).and_then(|result| {
            SearchScore::new(score).map(|score| SearchResult::new(result.memory().clone(), score))
        })
    }).collect()
}
