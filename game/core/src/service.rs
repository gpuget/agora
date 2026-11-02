use agora_api::{AuctionApi, BidOutcome, CreateListing};
use agora_domain::{
    AuctionHouse, HouseConfig, ItemStack, Listing, ListingId, Money, Page, PlayerRef, SearchQuery,
};
use game_api::{GameApi, SellCommand};
use game_domain::{CatalogEntry, GameError, Player};
use game_spi::{Catalog, HouseRegistry, PlayerView, TreasuryView};
use tracing::info;

pub struct GameService<P, C, H, T, A> {
    players: P,
    catalog: C,
    houses: H,
    treasury: T,
    auction: A,
}

impl<P, C, H, T, A> GameService<P, C, H, T, A>
where
    P: PlayerView,
    C: Catalog,
    H: HouseRegistry,
    T: TreasuryView,
    A: AuctionApi,
{
    pub fn new(players: P, catalog: C, houses: H, treasury: T, auction: A) -> Self {
        Self {
            players,
            catalog,
            houses,
            treasury,
            auction,
        }
    }
}

impl<P, C, H, T, A> GameApi for GameService<P, C, H, T, A>
where
    P: PlayerView,
    C: Catalog,
    H: HouseRegistry,
    T: TreasuryView,
    A: AuctionApi,
{
    async fn open_auction_house(&self, config: HouseConfig) -> Result<AuctionHouse, GameError> {
        let id = self.auction.create_auction_house(config.clone()).await?;
        let house = AuctionHouse { id, config };
        self.houses.add(house.clone()).await?;
        info!(house = %house.id, name = %house.config.name, commission_bps = house.config.commission.0, "auction house opened");
        Ok(house)
    }

    async fn auction_houses(&self) -> Result<Vec<AuctionHouse>, GameError> {
        self.houses.list().await
    }

    async fn players(&self) -> Result<Vec<Player>, GameError> {
        self.players.list().await
    }

    async fn catalog(&self) -> Result<Vec<CatalogEntry>, GameError> {
        self.catalog.entries().await
    }

    async fn money_taken(&self) -> Result<Money, GameError> {
        self.treasury.total().await
    }

    async fn search_market(&self, query: SearchQuery) -> Result<Page<Listing>, GameError> {
        Ok(self.auction.search(query).await?)
    }

    async fn sell(&self, cmd: SellCommand) -> Result<ListingId, GameError> {
        let entry = self
            .catalog
            .entry(&cmd.item_type)
            .await?
            .ok_or_else(|| GameError::UnknownItemType(cmd.item_type.clone()))?;
        let listing = CreateListing {
            house: cmd.house,
            seller: cmd.player.clone(),
            item: ItemStack {
                item_type: cmd.item_type.clone(),
                quantity: cmd.quantity,
                commodity: entry.commodity,
                base_value: entry.base_value,
                attributes: Default::default(),
            },
            start_unit_price: cmd.start_unit_price,
            buyout_unit_price: cmd.buyout_unit_price,
            duration: cmd.duration,
        };
        let id = self.auction.create_listing(listing).await?;
        info!(
            listing = %id,
            house = %cmd.house,
            seller = %cmd.player,
            item_type = %cmd.item_type,
            quantity = cmd.quantity,
            start_unit_price = cmd.start_unit_price.0,
            buyout_unit_price = cmd.buyout_unit_price.map(|b| b.0),
            "listing created"
        );
        Ok(id)
    }

    async fn bid(
        &self,
        player: PlayerRef,
        listing: ListingId,
        unit_price: Money,
    ) -> Result<BidOutcome, GameError> {
        let outcome = self
            .auction
            .place_bid(player.clone(), listing, unit_price)
            .await?;
        info!(%listing, bidder = %player, unit_price = unit_price.0, ?outcome, "bid placed");
        Ok(outcome)
    }

    async fn buyout(
        &self,
        player: PlayerRef,
        listing: ListingId,
        quantity: u32,
    ) -> Result<(), GameError> {
        self.auction
            .buyout(player.clone(), listing, quantity)
            .await?;
        info!(%listing, buyer = %player, quantity, "listing bought out");
        Ok(())
    }

    async fn cancel(&self, player: PlayerRef, listing: ListingId) -> Result<(), GameError> {
        self.auction.cancel_listing(player.clone(), listing).await?;
        info!(%listing, seller = %player, "listing cancelled");
        Ok(())
    }

    async fn settle_auctions(&self) -> Result<usize, GameError> {
        let settled = self.auction.settle_expired().await?;
        if settled > 0 {
            info!(settled, "expired listings settled");
        }
        Ok(settled)
    }
}
