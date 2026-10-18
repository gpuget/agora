use std::marker::PhantomData;

use agora_domain::Money;
use game_domain::{Decider, GameError, Player};
use game_spi::{EventStore, PlayerView, Projection, TreasuryView};

/// Attempts of one command before giving up on [`GameError::Conflict`].
const MAX_ATTEMPTS: usize = 3;
/// Events read from the store per projection batch.
const BATCH: usize = 256;

/// Runs `command` on the aggregate of stream `id`: loads its events, folds them, decides and
/// appends the new events if no other write happened in between. Retries on conflict.
/// Returns the recorded events, empty when the command was already applied.
pub(crate) async fn execute<D, S>(
    store: &S,
    id: &D::Id,
    command: &D::Command,
) -> Result<Vec<D::Event>, GameError>
where
    D: Decider,
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
            Err(GameError::Conflict) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(GameError::Conflict)
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
    pub fn new(store: S, projection: P) -> Self {
        Self {
            store,
            projection,
            _events: PhantomData,
        }
    }

    /// Applies every event recorded since the checkpoint of the projection. Returns how many.
    pub async fn catch_up(&self) -> Result<usize, GameError> {
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

/// A read model that catches up with its event store on every read, so it never lags
/// behind the events recorded before the call.
pub struct CaughtUpView<S, P, I, E>(Projector<S, P, I, E>);

impl<S, P, I, E> CaughtUpView<S, P, I, E>
where
    S: EventStore<I, E>,
    P: Projection<I, E>,
    I: Send + Sync,
    E: Send + Sync,
{
    pub fn new(store: S, projection: P) -> Self {
        Self(Projector::new(store, projection))
    }
}

impl<S, P, I, E> PlayerView for CaughtUpView<S, P, I, E>
where
    S: EventStore<I, E>,
    P: Projection<I, E> + PlayerView,
    I: Send + Sync,
    E: Send + Sync,
{
    async fn list(&self) -> Result<Vec<Player>, GameError> {
        self.0.catch_up().await?;
        self.0.projection.list().await
    }
}

impl<S, P, I, E> TreasuryView for CaughtUpView<S, P, I, E>
where
    S: EventStore<I, E>,
    P: Projection<I, E> + TreasuryView,
    I: Send + Sync,
    E: Send + Sync,
{
    async fn total(&self) -> Result<Money, GameError> {
        self.0.catch_up().await?;
        self.0.projection.total().await
    }
}
