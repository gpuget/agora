//! In-memory event store. Cheap to clone: clones share state.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use crate::{EventStore, Recorded, StoreError};

struct Log<I, E> {
    streams: BTreeMap<I, Vec<E>>,
    /// The global log: the event at index `i` has position `i + 1`.
    all: Vec<Recorded<I, E>>,
}

/// [`EventStore`] behind a mutex. Never fails with [`StoreError::Storage`].
pub struct InMemoryEventStore<I, E>(Arc<Mutex<Log<I, E>>>);

impl<I, E> Default for InMemoryEventStore<I, E> {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(Log {
            streams: BTreeMap::new(),
            all: Vec::new(),
        })))
    }
}

impl<I, E> Clone for InMemoryEventStore<I, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<I, E> EventStore<I, E> for InMemoryEventStore<I, E>
where
    I: Ord + Clone + Send + Sync,
    E: Clone + Send + Sync,
{
    async fn load(&self, stream: &I) -> Result<Vec<E>, StoreError> {
        let log = self.0.lock().unwrap();
        Ok(log.streams.get(stream).cloned().unwrap_or_default())
    }

    async fn append(&self, stream: &I, expected: u64, events: Vec<E>) -> Result<u64, StoreError> {
        let mut log = self.0.lock().unwrap();
        let Log { streams, all } = &mut *log;
        let recorded = streams.entry(stream.clone()).or_default();
        if recorded.len() as u64 != expected {
            return Err(StoreError::Conflict);
        }
        for event in events {
            all.push(Recorded {
                position: all.len() as u64 + 1,
                stream: stream.clone(),
                event: event.clone(),
            });
            recorded.push(event);
        }
        Ok(recorded.len() as u64)
    }

    async fn read_all(&self, after: u64, limit: usize) -> Result<Vec<Recorded<I, E>>, StoreError> {
        let log = self.0.lock().unwrap();
        let start = usize::try_from(after).unwrap_or(usize::MAX);
        Ok(log.all.iter().skip(start).take(limit).cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn append_checks_the_expected_length() {
        let store = InMemoryEventStore::<&str, u32>::default();
        assert_eq!(store.append(&"a", 0, vec![1, 2]).await, Ok(2));
        assert_eq!(
            store.append(&"a", 1, vec![3]).await,
            Err(StoreError::Conflict)
        );
        assert_eq!(store.append(&"b", 0, vec![4]).await, Ok(1));
        assert_eq!(store.load(&"a").await, Ok(vec![1, 2]));
    }

    #[tokio::test]
    async fn read_all_follows_the_global_log() {
        let store = InMemoryEventStore::<&str, u32>::default();
        store.append(&"a", 0, vec![1]).await.unwrap();
        store.append(&"b", 0, vec![2]).await.unwrap();
        store.append(&"a", 1, vec![3]).await.unwrap();
        let read = store.read_all(1, 10).await.unwrap();
        let read: Vec<_> = read
            .iter()
            .map(|r| (r.position, r.stream, r.event))
            .collect();
        assert_eq!(read, vec![(2, "b", 2), (3, "a", 3)]);
    }
}
