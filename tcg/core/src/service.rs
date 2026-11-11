use agora_api::{AuctionApi, BidOutcome, CreateListing};
use agora_domain::{
    AuctionHouse, HouseConfig, ItemStack, Listing, ListingId, Money, Page, PlayerRef, SearchQuery,
};
use tcg_api::{SellCommand, TcgApi};
use tcg_domain::{CardEntry, Collector, TcgError};
use tcg_spi::{Catalog, CollectorView, HouseRegistry, TreasuryView};
use tracing::info;

/// A listing always holds one copy of a card: the game never sells more than one at a time.
const COPIES_PER_LISTING: u32 = 1;

/// Implements [`TcgApi`]. Market use cases go to Agora (`A`); the base value of a new
/// listing comes from the catalog (`C`). Every listing holds one copy and is never a
/// commodity. Each successful change is logged at info level.
pub struct TcgService<P, C, H, T, A> {
    collectors: P,
    catalog: C,
    houses: H,
    treasury: T,
    auction: A,
}

impl<P, C, H, T, A> TcgService<P, C, H, T, A>
where
    P: CollectorView,
    C: Catalog,
    H: HouseRegistry,
    T: TreasuryView,
    A: AuctionApi,
{
    /// `auction` is the Agora core. The other arguments are the game outbound ports.
    pub fn new(collectors: P, catalog: C, houses: H, treasury: T, auction: A) -> Self {
        Self {
            collectors,
            catalog,
            houses,
            treasury,
            auction,
        }
    }
}

impl<P, C, H, T, A> TcgApi for TcgService<P, C, H, T, A>
where
    P: CollectorView,
    C: Catalog,
    H: HouseRegistry,
    T: TreasuryView,
    A: AuctionApi,
{
    async fn open_auction_house(&self, config: HouseConfig) -> Result<AuctionHouse, TcgError> {
        let id = self.auction.create_auction_house(config.clone()).await?;
        let house = AuctionHouse { id, config };
        self.houses.add(house.clone()).await?;
        info!(house = %house.id, name = %house.config.name, commission_bps = house.config.commission.0, "auction house opened");
        Ok(house)
    }

    async fn auction_houses(&self) -> Result<Vec<AuctionHouse>, TcgError> {
        self.houses.list().await
    }

    async fn collectors(&self) -> Result<Vec<Collector>, TcgError> {
        self.collectors.list().await
    }

    async fn catalog(&self) -> Result<Vec<CardEntry>, TcgError> {
        self.catalog.entries().await
    }

    async fn dust_taken(&self) -> Result<Money, TcgError> {
        self.treasury.total().await
    }

    async fn search_market(&self, query: SearchQuery) -> Result<Page<Listing>, TcgError> {
        Ok(self.auction.search(query).await?)
    }

    async fn sell(&self, cmd: SellCommand) -> Result<ListingId, TcgError> {
        let entry = self
            .catalog
            .entry(&cmd.card)
            .await?
            .ok_or_else(|| TcgError::UnknownCard(cmd.card.clone()))?;
        let listing = CreateListing {
            house: cmd.house,
            seller: cmd.collector.clone(),
            item: ItemStack {
                item_type: cmd.card.clone(),
                quantity: COPIES_PER_LISTING,
                commodity: false,
                base_value: entry.base_value,
                attributes: Default::default(),
            },
            start_unit_price: cmd.start_price,
            buyout_unit_price: cmd.buyout_price,
            duration: cmd.duration,
        };
        let id = self.auction.create_listing(listing).await?;
        info!(
            listing = %id,
            house = %cmd.house,
            seller = %cmd.collector,
            card = %cmd.card,
            start_price = cmd.start_price.0,
            buyout_price = cmd.buyout_price.map(|b| b.0),
            "listing created"
        );
        Ok(id)
    }

    async fn bid(
        &self,
        collector: PlayerRef,
        listing: ListingId,
        price: Money,
    ) -> Result<BidOutcome, TcgError> {
        let outcome = self
            .auction
            .place_bid(collector.clone(), listing, price)
            .await?;
        info!(%listing, bidder = %collector, price = price.0, ?outcome, "bid placed");
        Ok(outcome)
    }

    async fn buyout(&self, collector: PlayerRef, listing: ListingId) -> Result<(), TcgError> {
        self.auction
            .buyout(collector.clone(), listing, COPIES_PER_LISTING)
            .await?;
        info!(%listing, buyer = %collector, "listing bought out");
        Ok(())
    }

    async fn cancel(&self, collector: PlayerRef, listing: ListingId) -> Result<(), TcgError> {
        self.auction
            .cancel_listing(collector.clone(), listing)
            .await?;
        info!(%listing, seller = %collector, "listing cancelled");
        Ok(())
    }

    async fn settle_auctions(&self) -> Result<usize, TcgError> {
        let settled = self.auction.settle_expired().await?;
        if settled > 0 {
            info!(settled, "expired listings settled");
        }
        Ok(settled)
    }
}
