//! Recall's question-answering use case.
//!
//! Retrieval and generation remain separate capabilities: this use case first
//! selects memories, then gives only those memories to the inference backend.
//! Provenance is derived from the retrieved memories rather than from model
//! output.

use std::fmt;

use super::ports::{InferenceBackend, InferenceError, MemorySearcher, SearchError};
use crate::domain::{GenerationRequest, MemoryId, Relevance, RetrievedMemory, SearchQuery};

/// User request handled by the Recall answering use case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AskRequest {
    question: String,
    use_ai: bool,
}

impl AskRequest {
    /// Creates an ask request, rejecting an empty question.
    pub fn new(question: String, use_ai: bool) -> Result<Self, AskRequestError> {
        if question.trim().is_empty() {
            Err(AskRequestError::EmptyQuestion)
        } else {
            Ok(Self { question, use_ai })
        }
    }

    pub fn use_ai(&self) -> bool {
        self.use_ai
    }

    /// Returns the user's question.
    pub fn question(&self) -> &str {
        &self.question
    }
}

/// Validation failure for an ask request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AskRequestError {
    /// The question contains no non-whitespace characters.
    EmptyQuestion,
}

impl fmt::Display for AskRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyQuestion => formatter.write_str("question cannot be empty"),
        }
    }
}

impl std::error::Error for AskRequestError {}

/// Application-level answer returned after retrieval and generation.
#[derive(Clone, Debug, PartialEq)]
pub struct Answer {
    text: String,
    sources: Vec<MemoryId>,
    retrieved: Option<Vec<RetrievedMemoryAnswer>>,
}

impl Answer {
    /// Creates an answer from generated text and application-owned provenance.
    fn new(text: String, sources: Vec<MemoryId>) -> Self {
        Self {
            text,
            sources,
            retrieved: None,
        }
    }

    /// Creates a retrieval-only answer while retaining the complete scored results.
    fn from_retrieved(memories: Vec<RetrievedMemoryAnswer>) -> Self {
        let text = memories
            .iter()
            .map(|memory| memory.content())
            .collect::<Vec<_>>()
            .join("\n");
        let sources = memories
            .iter()
            .map(RetrievedMemoryAnswer::memory_id)
            .collect();

        Self {
            text,
            sources,
            retrieved: Some(memories),
        }
    }

    /// Returns generated answer text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns canonical memory identities selected by retrieval.
    pub fn sources(&self) -> &[MemoryId] {
        &self.sources
    }

    /// Returns the structured retrieval results for a retrieval-only answer.
    pub fn retrieved(&self) -> Option<&[RetrievedMemoryAnswer]> {
        self.retrieved.as_deref()
    }
}

/// One scored memory returned by a retrieval-only ask.
#[derive(Clone, Debug, PartialEq)]
pub struct RetrievedMemoryAnswer {
    memory_id: MemoryId,
    content: String,
    score: f32,
}

impl RetrievedMemoryAnswer {
    fn new(memory_id: MemoryId, content: String, score: f32) -> Self {
        Self {
            memory_id,
            content,
            score,
        }
    }

    /// Returns the stable canonical memory identity.
    pub const fn memory_id(&self) -> MemoryId {
        self.memory_id
    }

    /// Returns the canonical memory content.
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Returns the retrieval score assigned by the search implementation.
    pub const fn score(&self) -> f32 {
        self.score
    }
}

/// Errors produced by the ask use case.
#[derive(Debug)]
pub enum AskError {
    /// The supplied request was invalid.
    InvalidRequest(AskRequestError),
    /// Retrieval failed before generation.
    Search(SearchError),
    /// The inference backend failed to generate an answer.
    Inference(InferenceError),
    /// The application could not construct a valid generation request.
    InvalidGenerationRequest,
    /// A search result had an invalid relevance score.
    InvalidRelevance,
}

impl fmt::Display for AskError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(error) => write!(formatter, "invalid ask request: {error}"),
            Self::Search(error) => write!(formatter, "memory retrieval failed: {error}"),
            Self::Inference(error) => write!(formatter, "inference failed: {error}"),
            Self::InvalidGenerationRequest => {
                formatter.write_str("could not construct generation request")
            }
            Self::InvalidRelevance => formatter.write_str("retrieval returned an invalid score"),
        }
    }
}

impl std::error::Error for AskError {}

/// Application use case for retrieval followed by language generation.
pub struct AskRecall<S, I> {
    searcher: S,
    ai_searcher: Option<Box<dyn MemorySearcher>>,
    inference: I,
    retrieval_limit: usize,
}

impl<S, I> AskRecall<S, I>
where
    S: MemorySearcher,
    I: InferenceBackend,
{
    /// Creates an ask use case with an explicit retrieval-context limit.
    pub fn new(searcher: S, inference: I, retrieval_limit: usize) -> Result<Self, AskConfigError> {
        if retrieval_limit == 0 {
            return Err(AskConfigError::ZeroRetrievalLimit);
        }
        Ok(Self {
            searcher,
            ai_searcher: None,
            inference,
            retrieval_limit,
        })
    }

    /// Creates an ask use case with a separate hybrid searcher for AI mode.
    pub fn with_hybrid(
        searcher: S,
        ai_searcher: Box<dyn MemorySearcher>,
        inference: I,
        retrieval_limit: usize,
    ) -> Result<Self, AskConfigError> {
        if retrieval_limit == 0 {
            return Err(AskConfigError::ZeroRetrievalLimit);
        }
        Ok(Self {
            searcher,
            ai_searcher: Some(ai_searcher),
            inference,
            retrieval_limit,
        })
    }

    /// Retrieves relevant memories and generates an answer from that context.
    pub fn execute(&self, request: AskRequest) -> Result<Answer, AskError> {
        let query = SearchQuery::new(request.question().to_owned(), self.retrieval_limit)
            .map_err(|_| AskError::InvalidGenerationRequest)?;
        let results = if request.use_ai() {
            self.ai_searcher
                .as_ref()
                .map(|searcher| searcher.search(&query))
                .unwrap_or_else(|| self.searcher.search(&query))
        } else {
            self.searcher.search(&query)
        }
        .map_err(AskError::Search)?;
        let context = results
            .iter()
            .map(|result| {
                Relevance::new(result.score().value())
                    .map(|relevance| RetrievedMemory::new(result.memory().clone(), relevance))
                    .ok_or(AskError::InvalidRelevance)
            })
            .collect::<Result<Vec<_>, _>>()?;

        if !request.use_ai() {
            let memories = results
                .into_iter()
                .map(|result| {
                    RetrievedMemoryAnswer::new(
                        result.memory().id(),
                        result.memory().content().to_owned(),
                        result.score().value(),
                    )
                })
                .collect();

            return Ok(Answer::from_retrieved(memories));
        }

        if context.is_empty() {
            return Err(AskError::InvalidGenerationRequest);
        }

        let generation = GenerationRequest::new(request.question().to_owned(), context)
            .ok_or(AskError::InvalidGenerationRequest)?;
        let response = self
            .inference
            .generate(&generation)
            .map_err(AskError::Inference)?;

        let sources = generation.source_ids();
        Ok(Answer::new(response.text().to_owned(), sources))
    }
}

/// Invalid configuration for the ask use case.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AskConfigError {
    /// Retrieval must request at least one candidate.
    ZeroRetrievalLimit,
}

impl fmt::Display for AskConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroRetrievalLimit => {
                formatter.write_str("retrieval limit must be greater than zero")
            }
        }
    }
}

impl std::error::Error for AskConfigError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        GenerationResponse, Memory, MemoryId, MemorySource, SearchScore, Timestamp,
    };

    struct Searcher {
        memory: Memory,
    }

    impl MemorySearcher for Searcher {
        fn search(
            &self,
            _query: &SearchQuery,
        ) -> Result<Vec<crate::domain::SearchResult>, SearchError> {
            Ok(vec![crate::domain::SearchResult::new(
                self.memory.clone(),
                SearchScore::new(0.75).unwrap(),
            )])
        }
    }

    struct EmptySearcher;

    impl MemorySearcher for EmptySearcher {
        fn search(
            &self,
            _query: &SearchQuery,
        ) -> Result<Vec<crate::domain::SearchResult>, SearchError> {
            Ok(Vec::new())
        }
    }

    struct Inference {
        seen: std::cell::RefCell<Option<GenerationRequest>>,
    }

    impl InferenceBackend for &Inference {
        fn generate(
            &self,
            request: &GenerationRequest,
        ) -> Result<GenerationResponse, InferenceError> {
            self.seen.borrow_mut().replace(request.clone());
            Ok(GenerationResponse::new("answer".to_owned()).unwrap())
        }

        fn embed(
            &self,
            _request: &crate::domain::EmbeddingRequest,
        ) -> Result<crate::domain::EmbeddingResponse, InferenceError> {
            unreachable!("ask does not embed")
        }
    }

    fn memory() -> Memory {
        let timestamp = Timestamp::from_unix_millis(10);
        Memory::new(
            MemoryId::new(),
            "Rust uses ownership.".to_owned(),
            MemorySource::DirectInput,
            timestamp,
            timestamp,
        )
        .unwrap()
    }

    #[test]
    fn ask_passes_retrieved_memory_to_inference_and_preserves_provenance() {
        let memory = memory();
        let id = memory.id();
        let inference = Inference {
            seen: std::cell::RefCell::new(None),
        };
        let ask = AskRecall::new(Searcher { memory }, &inference, 5).unwrap();

        let answer = ask
            .execute(AskRequest::new("how does Rust manage memory?".to_owned(), true).unwrap())
            .unwrap();

        assert_eq!(answer.text(), "answer");
        assert_eq!(answer.sources(), &[id]);
        let seen = inference.seen.borrow();
        assert_eq!(seen.as_ref().unwrap().source_ids(), vec![id]);
    }

    #[test]
    fn retrieval_only_answer_preserves_ids_content_and_scores() {
        let memory = memory();
        let id = memory.id();
        let inference = Inference {
            seen: std::cell::RefCell::new(None),
        };
        let ask = AskRecall::new(Searcher { memory }, &inference, 5).unwrap();

        let answer = ask
            .execute(AskRequest::new("ownership".to_owned(), false).unwrap())
            .unwrap();

        let retrieved = answer.retrieved().expect("retrieval-only answer");
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].memory_id(), id);
        assert_eq!(retrieved[0].content(), "Rust uses ownership.");
        assert_eq!(retrieved[0].score(), 0.75);
        assert_eq!(answer.sources(), &[id]);
        assert!(inference.seen.borrow().is_none());
    }

    #[test]
    fn ai_with_no_retrieved_context_does_not_invoke_inference() {
        let inference = Inference {
            seen: std::cell::RefCell::new(None),
        };
        let ask = AskRecall::new(EmptySearcher, &inference, 5).unwrap();

        let result = ask.execute(AskRequest::new("unknown".to_owned(), true).unwrap());

        assert!(matches!(result, Err(AskError::InvalidGenerationRequest)));
        assert!(inference.seen.borrow().is_none());
    }

    #[test]
    fn zero_retrieval_limit_is_rejected() {
        let inference = Inference {
            seen: std::cell::RefCell::new(None),
        };
        assert!(matches!(
            AskRecall::new(Searcher { memory: memory() }, &inference, 0),
            Err(AskConfigError::ZeroRetrievalLimit)
        ));
    }
}
