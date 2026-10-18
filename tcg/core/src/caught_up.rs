use agora_domain::Money;
use event_sourcing::{EventStore, Projection, Projector};
use tcg_domain::{Collector, TcgError};
use tcg_spi::{CollectorView, TreasuryView};

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

impl<S, P, I, E> CollectorView for CaughtUpView<S, P, I, E>
where
    S: EventStore<I, E>,
    P: Projection<I, E> + CollectorView,
    I: Send + Sync,
    E: Send + Sync,
{
    async fn list(&self) -> Result<Vec<Collector>, TcgError> {
        self.0.catch_up().await?;
        self.0.projection().list().await
    }
}

impl<S, P, I, E> TreasuryView for CaughtUpView<S, P, I, E>
where
    S: EventStore<I, E>,
    P: Projection<I, E> + TreasuryView,
    I: Send + Sync,
    E: Send + Sync,
{
    async fn total(&self) -> Result<Money, TcgError> {
        self.0.catch_up().await?;
        self.0.projection().total().await
    }
}
