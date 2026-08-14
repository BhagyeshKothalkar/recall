//! System clock infrastructure.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::application::ports::Clock;
use crate::domain::Timestamp;

/// Production clock backed by the operating system.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        Timestamp::from_unix_millis(millis)
    }
}
