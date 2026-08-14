//! Port for application-visible time.

use crate::domain::Timestamp;

/// Source of current time for application operations.
///
/// Production infrastructure supplies a system clock. Tests can supply a
/// deterministic implementation without introducing time-system concerns
/// into domain code.
pub trait Clock {
    /// Returns the current application timestamp.
    fn now(&self) -> Timestamp;
}
