use agora_api::{AuctionApi, BidOutcome, CreateListing};
use agora_domain::{
    AuctionError, AuctionHouse, Bid, HouseConfig, HouseId, Listing, ListingId, ListingStatus,
    Money, OpId, Page, PlayerRef, SearchQuery,
};
use agora_spi::{AuctionRepository, Clock, Inventory, MoneySink, RepoError, SinkReason, Wallet};
use uuid::Uuid;

/// Attempts of a use case before giving up on version conflicts.
const MAX_ATTEMPTS: usize = 3;
/// Listings settled per call to `settle_expired`.
const SETTLE_BATCH: u32 = 100;
const MAX_PAGE: u32 = 100;

pub struct AuctionService<R, I, W, S, C> {
    repository: R,
    inventory: I,
    wallet: W,
    sink: S,
    clock: C,
}

impl<R, I, W, S, C> AuctionService<R, I, W, S, C>
where
    R: AuctionRepository,
    I: Inventory,
    W: Wallet,
    S: MoneySink,
    C: Clock,
{
    pub fn new(repository: R, inventory: I, wallet: W, sink: S, clock: C) -> Self {
        Self {
            repository,
            inventory,
            wallet,
            sink,
            clock,
        }
    }

    pub fn repository(&self) -> &R {
        &self.repository
    }

    pub fn inventory(&self) -> &I {
        &self.inventory
    }

    pub fn wallet(&self) -> &W {
        &self.wallet
    }

    pub fn sink(&self) -> &S {
        &self.sink
    }

    pub fn clock(&self) -> &C {
        &self.clock
    }

    /// Loads a listing that still accepts bids, buyouts and cancellation.
    async fn load_open(&self, id: ListingId) -> Result<Listing, AuctionError> {
        let listing = self
            .repository
            .get_listing(id)
            .await
            .map_err(repo_error)?
            .ok_or(AuctionError::ListingNotFound)?;
        if listing.status != ListingStatus::Active {
            return Err(AuctionError::NotActive);
        }
        if listing.is_past_end(self.clock.now()) {
            return Err(AuctionError::Expired);
        }
        Ok(listing)
    }

    /// Bumps the version and stores the listing. `Ok(false)` on version conflict.
    async fn save(&self, listing: &mut Listing) -> Result<bool, AuctionError> {
        let expected = listing.version;
        listing.version += 1;
        match self.repository.update_listing(listing, expected).await {
            Ok(()) => Ok(true),
            Err(RepoError::Conflict) => Ok(false),
            Err(e) => Err(repo_error(e)),
        }
    }

    /// Saves after `payer` was debited `paid`. Refunds the payer if the save does not go through.
    async fn save_or_refund(
        &self,
        listing: &mut Listing,
        payer: &PlayerRef,
        paid: Money,
    ) -> Result<bool, AuctionError> {
        let saved = self.save(listing).await;
        if !matches!(saved, Ok(true)) {
            self.wallet.credit(op(), payer, paid).await?;
        }
        saved
    }

    async fn refund_bid(&self, listing: &Listing, bid: &Bid) -> Result<(), AuctionError> {
        let amount = listing
            .total(bid.unit_price)
            .ok_or(AuctionError::Overflow)?;
        self.wallet.credit(op(), &bid.bidder, amount).await?;
        Ok(())
    }

    /// Delivers the stack to the buyer, pays the seller and sends the commission to the sink.
    async fn pay_out(
        &self,
        listing: &Listing,
        buyer: &PlayerRef,
        unit_price: Money,
    ) -> Result<(), AuctionError> {
        let total = listing.total(unit_price).ok_or(AuctionError::Overflow)?;
        let commission = total.apply_bps(listing.commission);
        let proceeds = total
            .checked_sub(commission)
            .and_then(|m| m.checked_add(listing.deposit))
            .ok_or(AuctionError::Overflow)?;
        self.inventory
            .deposit_item(op(), buyer, &listing.item)
            .await?;
        self.wallet.credit(op(), &listing.seller, proceeds).await?;
        if commission > Money::ZERO {
            let reason = SinkReason::Commission {
                listing: listing.id,
            };
            self.sink
                .absorb(op(), listing.house, commission, reason)
                .await?;
        }
        Ok(())
    }

    /// Returns the stack to the seller and forfeits the deposit.
    async fn return_unsold(&self, listing: &Listing) -> Result<(), AuctionError> {
        self.inventory
            .deposit_item(op(), &listing.seller, &listing.item)
            .await?;
        if listing.deposit > Money::ZERO {
            let reason = SinkReason::ForfeitedDeposit {
                listing: listing.id,
            };
            self.sink
                .absorb(op(), listing.house, listing.deposit, reason)
                .await?;
        }
        Ok(())
    }

    /// One buyout attempt on an already loaded open listing. `Ok(false)` on version conflict.
    async fn try_buyout(&self, listing: Listing, buyer: &PlayerRef) -> Result<bool, AuctionError> {
        if &listing.seller == buyer {
            return Err(AuctionError::SellerCannotBuy);
        }
        let unit_price = listing.buyout_unit_price.ok_or(AuctionError::NoBuyout)?;
        let total = listing.total(unit_price).ok_or(AuctionError::Overflow)?;
        self.wallet.debit(op(), buyer, total).await?;

        let previous = listing.high_bid.clone();
        let mut sold = listing;
        sold.status = ListingStatus::Sold {
            buyer: buyer.clone(),
            unit_price,
        };
        if !self.save_or_refund(&mut sold, buyer, total).await? {
            return Ok(false);
        }

        if let Some(bid) = &previous {
            self.refund_bid(&sold, bid).await?;
        }
        self.pay_out(&sold, buyer, unit_price).await?;
        Ok(true)
    }

    /// One bid attempt. `Ok(None)` on version conflict.
    async fn try_place_bid(
        &self,
        bidder: &PlayerRef,
        id: ListingId,
        unit_price: Money,
    ) -> Result<Option<BidOutcome>, AuctionError> {
        let listing = self.load_open(id).await?;
        if &listing.seller == bidder {
            return Err(AuctionError::SellerCannotBuy);
        }
        if listing
            .buyout_unit_price
            .is_some_and(|buyout| unit_price >= buyout)
        {
            let bought = self.try_buyout(listing, bidder).await?;
            return Ok(bought.then_some(BidOutcome::Bought));
        }
        if listing
            .high_bid
            .as_ref()
            .is_some_and(|b| &b.bidder == bidder)
        {
            return Err(AuctionError::AlreadyHighBidder);
        }
        let minimum = listing.min_next_bid().ok_or(AuctionError::Overflow)?;
        if unit_price < minimum {
            return Err(AuctionError::BidTooLow { minimum });
        }
        let total = listing.total(unit_price).ok_or(AuctionError::Overflow)?;
        self.wallet.debit(op(), bidder, total).await?;

        let previous = listing.high_bid.clone();
        let mut updated = listing;
        updated.high_bid = Some(Bid {
            bidder: bidder.clone(),
            unit_price,
        });
        if !self.save_or_refund(&mut updated, bidder, total).await? {
            return Ok(None);
        }

        if let Some(bid) = &previous {
            self.refund_bid(&updated, bid).await?;
        }
        Ok(Some(BidOutcome::HighBidder))
    }
}

impl<R, I, W, S, C> AuctionApi for AuctionService<R, I, W, S, C>
where
    R: AuctionRepository,
    I: Inventory,
    W: Wallet,
    S: MoneySink,
    C: Clock,
{
    async fn create_auction_house(&self, config: HouseConfig) -> Result<HouseId, AuctionError> {
        config.validate()?;
        let house = AuctionHouse {
            id: HouseId(Uuid::now_v7()),
            config,
        };
        self.repository
            .insert_house(&house)
            .await
            .map_err(repo_error)?;
        Ok(house.id)
    }

    async fn create_listing(&self, cmd: CreateListing) -> Result<ListingId, AuctionError> {
        let house = self
            .repository
            .get_house(cmd.house)
            .await
            .map_err(repo_error)?
            .ok_or(AuctionError::HouseNotFound)?;

        let quantity = u64::from(cmd.item.quantity);
        if quantity == 0 {
            return Err(AuctionError::InvalidQuantity);
        }
        if cmd.start_unit_price == Money::ZERO {
            return Err(AuctionError::InvalidPrice);
        }
        if cmd
            .buyout_unit_price
            .is_some_and(|b| b < cmd.start_unit_price)
        {
            return Err(AuctionError::BuyoutBelowStart);
        }
        let highest_unit_price = cmd.buyout_unit_price.unwrap_or(cmd.start_unit_price);
        highest_unit_price
            .checked_mul(quantity)
            .ok_or(AuctionError::Overflow)?;

        let option = house
            .config
            .duration_option(cmd.duration)
            .ok_or(AuctionError::DurationNotAllowed)?;
        let deposit = cmd
            .item
            .base_value
            .checked_mul(quantity)
            .ok_or(AuctionError::Overflow)?
            .apply_bps(house.config.deposit)
            .checked_mul(u64::from(option.deposit_multiplier))
            .ok_or(AuctionError::Overflow)?;

        let now = self.clock.now();
        let ends_at = now
            .checked_add(cmd.duration)
            .ok_or(AuctionError::Overflow)?;

        self.inventory
            .withdraw_item(op(), &cmd.seller, &cmd.item)
            .await?;
        if deposit > Money::ZERO
            && let Err(e) = self.wallet.debit(op(), &cmd.seller, deposit).await
        {
            self.inventory
                .deposit_item(op(), &cmd.seller, &cmd.item)
                .await?;
            return Err(e.into());
        }

        let listing = Listing {
            id: ListingId(Uuid::now_v7()),
            house: house.id,
            seller: cmd.seller,
            item: cmd.item,
            start_unit_price: cmd.start_unit_price,
            buyout_unit_price: cmd.buyout_unit_price,
            commission: house.config.commission,
            min_increment: house.config.min_increment,
            increment: house.config.increment,
            deposit,
            created_at: now,
            ends_at,
            high_bid: None,
            status: ListingStatus::Active,
            version: 0,
        };
        self.repository
            .insert_listing(&listing)
            .await
            .map_err(repo_error)?;
        Ok(listing.id)
    }

    async fn cancel_listing(&self, seller: PlayerRef, id: ListingId) -> Result<(), AuctionError> {
        for _ in 0..MAX_ATTEMPTS {
            let mut listing = self.load_open(id).await?;
            if listing.seller != seller {
                return Err(AuctionError::NotSeller);
            }
            if listing.high_bid.is_some() {
                return Err(AuctionError::HasBids);
            }
            listing.status = ListingStatus::Cancelled;
            if self.save(&mut listing).await? {
                return self.return_unsold(&listing).await;
            }
        }
        Err(AuctionError::Conflict)
    }

    async fn place_bid(
        &self,
        bidder: PlayerRef,
        id: ListingId,
        unit_price: Money,
    ) -> Result<BidOutcome, AuctionError> {
        for _ in 0..MAX_ATTEMPTS {
            if let Some(outcome) = self.try_place_bid(&bidder, id, unit_price).await? {
                return Ok(outcome);
            }
        }
        Err(AuctionError::Conflict)
    }

    async fn buyout(&self, buyer: PlayerRef, id: ListingId) -> Result<(), AuctionError> {
        for _ in 0..MAX_ATTEMPTS {
            let listing = self.load_open(id).await?;
            if self.try_buyout(listing, &buyer).await? {
                return Ok(());
            }
        }
        Err(AuctionError::Conflict)
    }

    async fn settle_expired(&self) -> Result<usize, AuctionError> {
        let now = self.clock.now();
        let expired = self
            .repository
            .find_expired(now, SETTLE_BATCH)
            .await
            .map_err(repo_error)?;

        let mut settled = 0;
        for mut listing in expired {
            let winner = listing.high_bid.clone();
            listing.status = match &winner {
                Some(bid) => ListingStatus::Sold {
                    buyer: bid.bidder.clone(),
                    unit_price: bid.unit_price,
                },
                None => ListingStatus::Expired,
            };
            // A conflict means another caller changed it: it will show up again if still due.
            if !self.save(&mut listing).await? {
                continue;
            }
            match &winner {
                Some(bid) => self.pay_out(&listing, &bid.bidder, bid.unit_price).await?,
                None => self.return_unsold(&listing).await?,
            }
            settled += 1;
        }
        Ok(settled)
    }

    async fn search(&self, query: SearchQuery) -> Result<Page<Listing>, AuctionError> {
        if query.limit == 0 || query.limit > MAX_PAGE {
            return Err(AuctionError::InvalidPage { max: MAX_PAGE });
        }
        self.repository.search(&query).await.map_err(repo_error)
    }
}

fn op() -> OpId {
    OpId(Uuid::now_v7())
}

fn repo_error(error: RepoError) -> AuctionError {
    match error {
        RepoError::Conflict => AuctionError::Conflict,
        RepoError::Other(message) => AuctionError::Repository(message),
    }
}
