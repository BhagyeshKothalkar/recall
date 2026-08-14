//! Canonical memory domain types.
//!
//! A [`Memory`] is the authoritative representation of information Recall has
//! stored. Search scores, embeddings, summaries, and model output deliberately
//! do not belong here because they are derived data.

use std::fmt;

use uuid::Uuid;

use super::source::MemorySource;

/// Stable identity of a canonical memory.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MemoryId(Uuid);

impl MemoryId {
    /// Creates a new randomly generated memory identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Wraps an existing UUID as a memory identifier.
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Returns the underlying UUID.
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Consumes the identifier and returns its underlying UUID.
    pub const fn into_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for MemoryId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MemoryId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Domain timestamp represented as Unix milliseconds.
///
/// Keeping the representation small and explicit makes timestamps easy to
/// persist and deterministic to construct in tests. Conversion from the
/// system clock belongs to the infrastructure clock implementation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Timestamp(i64);

impl Timestamp {
    /// Creates a timestamp from Unix milliseconds.
    pub const fn from_unix_millis(value: i64) -> Self {
        Self(value)
    }

    /// Returns the Unix-millisecond representation.
    pub const fn as_unix_millis(self) -> i64 {
        self.0
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Validation failures for canonical memory construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryValidationError {
    /// Content contains no non-whitespace characters.
    EmptyContent,
    /// The update timestamp predates creation.
    InvalidTimestampOrder {
        created_at: Timestamp,
        updated_at: Timestamp,
    },
}

impl fmt::Display for MemoryValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyContent => formatter.write_str("memory content cannot be empty"),
            Self::InvalidTimestampOrder {
                created_at,
                updated_at,
            } => write!(
                formatter,
                "memory updated_at ({updated_at}) cannot precede created_at ({created_at})"
            ),
        }
    }
}

impl std::error::Error for MemoryValidationError {}

/// Canonical, authoritative memory state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Memory {
    id: MemoryId,
    content: String,
    source: MemorySource,
    created_at: Timestamp,
    updated_at: Timestamp,
}

impl Memory {
    /// Constructs a canonical memory after enforcing domain invariants.
    pub fn new(
        id: MemoryId,
        content: String,
        source: MemorySource,
        created_at: Timestamp,
        updated_at: Timestamp,
    ) -> Result<Self, MemoryValidationError> {
        if content.trim().is_empty() {
            return Err(MemoryValidationError::EmptyContent);
        }

        if updated_at < created_at {
            return Err(MemoryValidationError::InvalidTimestampOrder {
                created_at,
                updated_at,
            });
        }

        Ok(Self {
            id,
            content,
            source,
            created_at,
            updated_at,
        })
    }

    /// Returns the stable memory identifier.
    pub const fn id(&self) -> MemoryId {
        self.id
    }

    /// Returns the canonical content.
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Returns the source from which the memory was captured.
    pub const fn source(&self) -> &MemorySource {
        &self.source
    }

    /// Returns the creation timestamp.
    pub const fn created_at(&self) -> Timestamp {
        self.created_at
    }

    /// Returns the last-update timestamp.
    pub const fn updated_at(&self) -> Timestamp {
        self.updated_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timestamps() -> (Timestamp, Timestamp) {
        (Timestamp::from_unix_millis(10), Timestamp::from_unix_millis(20))
    }

    #[test]
    fn memory_rejects_whitespace_only_content() {
        let (created_at, updated_at) = timestamps();

        let result = Memory::new(
            MemoryId::new(),
            " \n\t ".to_owned(),
            MemorySource::DirectInput,
            created_at,
            updated_at,
        );

        assert_eq!(result, Err(MemoryValidationError::EmptyContent));
    }

    #[test]
    fn memory_rejects_invalid_timestamp_order() {
        let created_at = Timestamp::from_unix_millis(20);
        let updated_at = Timestamp::from_unix_millis(10);

        let result = Memory::new(
            MemoryId::new(),
            "hello".to_owned(),
            MemorySource::DirectInput,
            created_at,
            updated_at,
        );

        assert_eq!(
            result,
            Err(MemoryValidationError::InvalidTimestampOrder {
                created_at,
                updated_at,
            })
        );
    }
}
