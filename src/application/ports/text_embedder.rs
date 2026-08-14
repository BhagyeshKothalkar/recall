//! Port for embedding arbitrary query text.

use crate::domain::InferenceModel;

use super::InferenceError;

/// Capability used by semantic retrieval to embed a query without pretending
/// that the query is a canonical memory.
pub trait TextEmbedder {
    /// Produces a vector for arbitrary text using the requested model.
    fn embed_text(&self, text: &str, model: &InferenceModel) -> Result<Vec<f32>, InferenceError>;
}

impl<T: TextEmbedder + ?Sized> TextEmbedder for std::sync::Arc<T> {
    fn embed_text(&self, text: &str, model: &InferenceModel) -> Result<Vec<f32>, InferenceError> { (**self).embed_text(text, model) }
}
