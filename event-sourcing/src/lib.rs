//! Event sourcing, independent of any domain: aggregates as [`Decider`]s, the [`EventStore`]
//! port that records their events, and [`Projection`]s fed from its global log.
//!
//! Write side: [`execute`] runs a command on one aggregate with optimistic concurrency.
//! Read side: a [`Projector`] feeds a [`Projection`] from [`EventStore::read_all`].
//!
//! Not part of the Agora hexagon. In this workspace the `ledger` crate defines its
//! aggregates as deciders, and `game-core` and `tcg-core` keep projections of the ledger.
//!
//! Feature `memory`: enables the `memory` module, an in-memory [`EventStore`].

use std::future::Future;
use std::marker::PhantomData;

use thiserror::Error;

#[cfg(any(test, feature = "memory"))]
pub mod memory;

/// Attempts of one command before giving up on [`StoreError::Conflict`].
const MAX_ATTEMPTS: usize = 3;
/// Events read from the store per projection batch.
const BATCH: usize = 256;

/// An event-sourced aggregate.
pub trait Decider {
    /// Identifies the event stream of one aggregate.
    type Id: Send + Sync;
    /// Request to change the aggregate.
    type Command: Send + Sync;
    /// Fact recorded in the stream.
    type Event: Clone + Send + Sync;
    /// State folded from the events. `Default` is the state of an empty stream.
    type State: Default + Send;
    /// Rejection of a command. [`execute`] also needs it to wrap a [`StoreError`].
    type Error;

    /// Events to record for `command`. Empty when the command was already applied.
    fn decide(
        id: &Self::Id,
        command: &Self::Command,
        state: &Self::State,
    ) -> Result<Vec<Self::Event>, Self::Error>;

    /// Applies one recorded event. Never fails: events are facts.
    fn evolve(state: Self::State, event: &Self::Event) -> Self::State;
}

/// Failure of an [`EventStore`] or a [`Projection`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StoreError {
    /// The event stream changed since it was loaded.
    #[error("concurrent change, retry")]
    Conflict,
    /// Any other storage failure.
    #[error("storage error: {0}")]
    Storage(String),
}

/// One event of the global log of an event store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded<I, E> {
    /// Position in the global log, starting at 1.
    pub position: u64,
    /// Stream the event belongs to.
    pub stream: I,
    pub event: E,
}

/// Append-only log of events, split into streams identified by `I`.
pub trait EventStore<I, E>: Send + Sync {
    /// Events of one stream, in order. Empty if the stream does not exist.
    fn load(&self, stream: &I) -> impl Future<Output = Result<Vec<E>, StoreError>> + Send;

    /// Appends `events` to a stream if it still holds `expected` events. Fails with
    /// [`StoreError::Conflict`] otherwise. Returns the new number of events of the stream.
    fn append(
        &self,
        stream: &I,
        expected: u64,
        events: Vec<E>,
    ) -> impl Future<Output = Result<u64, StoreError>> + Send;

    /// At most `limit` events of all streams with a position greater than `after`, in
    /// position order.
    fn read_all(
        &self,
        after: u64,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<Recorded<I, E>>, StoreError>> + Send;
}

/// Read model fed by a projector from the global log of an [`EventStore`].
pub trait Projection<I, E>: Send + Sync {
    /// Position of the last applied event, 0 if none.
    fn checkpoint(&self) -> impl Future<Output = Result<u64, StoreError>> + Send;

    /// Applies `events` in order and moves the checkpoint to the last one, atomically.
    /// Skips events at or below the checkpoint, so a replayed batch has no effect.
    fn apply(
        &self,
        events: &[Recorded<I, E>],
    ) -> impl Future<Output = Result<(), StoreError>> + Send;
}

/// Runs `command` on the aggregate of stream `id`: loads its events, folds them, decides and
/// appends the new events if no other write happened in between. Retries on conflict.
/// Returns the recorded events, empty when the command was already applied.
///
/// # Errors
///
/// The error of [`Decider::decide`], or a [`StoreError`] converted into `D::Error`.
/// [`StoreError::Conflict`] after 3 attempts that all lost the race.
pub async fn execute<D, S>(
    store: &S,
    id: &D::Id,
    command: &D::Command,
) -> Result<Vec<D::Event>, D::Error>
where
    D: Decider,
    D::Error: From<StoreError>,
    S: EventStore<D::Id, D::Event>,
{
    for _ in 0..MAX_ATTEMPTS {
        let history = store.load(id).await?;
        let expected = history.len() as u64;
        let state = history.iter().fold(D::State::default(), D::evolve);
        let events = D::decide(id, command, &state)?;
        if events.is_empty() {
            return Ok(events);
        }
        match store.append(id, expected, events.clone()).await {
            Ok(_) => return Ok(events),
            Err(StoreError::Conflict) => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(StoreError::Conflict.into())
}

/// Feeds a projection with the events recorded in a store since its checkpoint.
pub struct Projector<S, P, I, E> {
    store: S,
    projection: P,
    _events: PhantomData<fn() -> (I, E)>,
}

impl<S, P, I, E> Projector<S, P, I, E>
where
    S: EventStore<I, E>,
    P: Projection<I, E>,
    I: Send + Sync,
    E: Send + Sync,
{
    /// Feeds `projection` from `store`.
    pub fn new(store: S, projection: P) -> Self {
        Self {
            store,
            projection,
            _events: PhantomData,
        }
    }

    /// The projection, to read the model.
    pub fn projection(&self) -> &P {
        &self.projection
    }

    /// Applies every event recorded since the checkpoint of the projection. Returns how many.
    ///
    /// Reads the log in batches of 256 events until none is left. Stops at the first error;
    /// the batches applied before it stay applied.
    pub async fn catch_up(&self) -> Result<usize, StoreError> {
        let mut applied = 0;
        loop {
            let after = self.projection.checkpoint().await?;
            let events = self.store.read_all(after, BATCH).await?;
            if events.is_empty() {
                return Ok(applied);
            }
            self.projection.apply(&events).await?;
            applied += events.len();
        }
    }
}
