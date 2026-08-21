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
}

impl AskRequest {
    /// Creates an ask request, rejecting an empty question.
    pub fn new(question: String) -> Result<Self, AskRequestError> {
        if question.trim().is_empty() {
            Err(AskRequestError::EmptyQuestion)
        } else {
            Ok(Self { question })
        }
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Answer {
    text: String,
    sources: Vec<MemoryId>,
}

impl Answer {
    /// Creates an answer from generated text and application-owned provenance.
    fn new(text: String, sources: Vec<MemoryId>) -> Self {
        Self { text, sources }
    }

    /// Returns generated answer text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns canonical memory identities selected by retrieval.
    pub fn sources(&self) -> &[MemoryId] {
        &self.sources
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
            inference,
            retrieval_limit,
        })
    }

    /// Retrieves relevant memories and generates an answer from that context.
    pub fn execute(&self, request: AskRequest) -> Result<Answer, AskError> {
        let query = SearchQuery::new(request.question().to_owned(), self.retrieval_limit)
            .map_err(|_| AskError::InvalidGenerationRequest)?;
        let results = self.searcher.search(&query).map_err(AskError::Search)?;
        let context = results
            .iter()
            .map(|result| {
                Relevance::new(result.score().value())
                    .map(|relevance| RetrievedMemory::new(result.memory().clone(), relevance))
                    .ok_or(AskError::InvalidRelevance)
            })
            .collect::<Result<Vec<_>, _>>()?;

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
            .execute(AskRequest::new("how does Rust manage memory?".to_owned()).unwrap())
            .unwrap();

        assert_eq!(answer.text(), "answer");
        assert_eq!(answer.sources(), &[id]);
        let seen = inference.seen.borrow();
        assert_eq!(seen.as_ref().unwrap().source_ids(), vec![id]);
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
