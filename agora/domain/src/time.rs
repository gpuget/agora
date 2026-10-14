use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Milliseconds since the Unix epoch.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Timestamp(pub u64);

impl Timestamp {
    pub fn checked_add(self, duration: Duration) -> Option<Timestamp> {
        let millis = u64::try_from(duration.as_millis()).ok()?;
        self.0.checked_add(millis).map(Timestamp)
    }

    /// Time left until `end`, zero if `end` is past.
    pub fn until(self, end: Timestamp) -> Duration {
        Duration::from_millis(end.0.saturating_sub(self.0))
    }
}
