use agora_api::{AuctionApi, BidOutcome, CreateListing};
use agora_domain::{
    AuctionError, AuctionHouse, Bid, HouseConfig, HouseId, ItemStack, Listing, ListingId, ListingStatus,
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

    /// Saves after `payer` was debited `paid` by the call `debit`. Refunds the payer if the
    /// save does not go through.
    async fn save_or_refund(
        &self,
        listing: &mut Listing,
        payer: &PlayerRef,
        paid: Money,
        debit: OpId,
    ) -> Result<bool, AuctionError> {
        let saved = self.save(listing).await;
        if !matches!(saved, Ok(true)) {
            self.wallet
                .credit(compensation_op(debit), payer, paid)
                .await?;
        }
        saved
    }

    async fn refund_bid(&self, listing: &Listing, bid: &Bid) -> Result<(), AuctionError> {
        let amount = listing
            .total(bid.unit_price)
            .ok_or(AuctionError::Overflow)?;
        self.wallet
            .credit(effect_op(listing, REFUND_BID), &bid.bidder, amount)
            .await?;
        Ok(())
    }

    /// Delivers the sold items to the buyer, pays the seller and sends the commission to the
    /// sink. `listing` is the saved listing.
    async fn pay_out(&self, listing: &Listing, sale: &Sale<'_>) -> Result<(), AuctionError> {
        let total = sale
            .unit_price
            .checked_mul(u64::from(sale.quantity))
            .ok_or(AuctionError::Overflow)?;
        let commission = total.apply_bps(listing.commission);
        let proceeds = total
            .checked_sub(commission)
            .and_then(|m| m.checked_add(sale.deposit_refund))
            .ok_or(AuctionError::Overflow)?;
        let delivered = ItemStack {
            quantity: sale.quantity,
            ..listing.item.clone()
        };
        self.inventory
            .deposit_item(effect_op(listing, DELIVER_ITEM), sale.buyer, &delivered)
            .await?;
        self.wallet
            .credit(effect_op(listing, PAY_SELLER), &listing.seller, proceeds)
            .await?;
        if commission > Money::ZERO {
            let reason = SinkReason::Commission {
                listing: listing.id,
            };
            self.sink
                .absorb(
                    effect_op(listing, COMMISSION),
                    listing.house,
                    commission,
                    reason,
                )
                .await?;
        }
        Ok(())
    }

    /// Returns the unsold items to the seller and forfeits the deposit still held.
    async fn return_unsold(&self, listing: &Listing) -> Result<(), AuctionError> {
        self.inventory
            .deposit_item(
                effect_op(listing, RETURN_ITEM),
                &listing.seller,
                &listing.item,
            )
            .await?;
        if listing.deposit > Money::ZERO {
            let reason = SinkReason::ForfeitedDeposit {
                listing: listing.id,
            };
            self.sink
                .absorb(
                    effect_op(listing, FORFEIT_DEPOSIT),
                    listing.house,
                    listing.deposit,
                    reason,
                )
                .await?;
        }
        Ok(())
    }

    /// One buyout attempt of `quantity` items on an already loaded open listing. `Ok(false)` on
    /// version conflict.
    async fn try_buyout(
        &self,
        listing: Listing,
        buyer: &PlayerRef,
        quantity: u32,
    ) -> Result<bool, AuctionError> {
        if &listing.seller == buyer {
            return Err(AuctionError::SellerCannotBuy);
        }
        let unit_price = listing.buyout_unit_price.ok_or(AuctionError::NoBuyout)?;
        let available = listing.item.quantity;
        if listing.item.commodity {
            if quantity == 0 || quantity > available {
                return Err(AuctionError::QuantityOutOfRange { available });
            }
        } else if quantity != available {
            return Err(AuctionError::PartialNotAllowed { available });
        }
        let total = unit_price
            .checked_mul(u64::from(quantity))
            .ok_or(AuctionError::Overflow)?;
        let debit = new_op();
        self.wallet.debit(debit, buyer, total).await?;

        let previous = listing.high_bid.clone();
        let sale = Sale {
            buyer,
            unit_price,
            quantity,
            deposit_refund: listing.deposit_share(quantity),
        };
        let mut sold = listing;
        if quantity == available {
            sold.status = ListingStatus::Sold {
                buyer: buyer.clone(),
                unit_price,
            };
        } else {
            // Partial sale: the listing stays open with what is left and the deposit that
            // covers it.
            sold.item.quantity = available - quantity;
            sold.deposit = sold
                .deposit
                .checked_sub(sale.deposit_refund)
                .ok_or(AuctionError::Overflow)?;
        }
        if !self.save_or_refund(&mut sold, buyer, total, debit).await? {
            return Ok(false);
        }

        if let Some(bid) = &previous {
            self.refund_bid(&sold, bid).await?;
        }
        self.pay_out(&sold, &sale).await?;
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
        if listing.item.commodity {
            return Err(AuctionError::BidsNotAllowed);
        }
        if &listing.seller == bidder {
            return Err(AuctionError::SellerCannotBuy);
        }
        if listing
            .buyout_unit_price
            .is_some_and(|buyout| unit_price >= buyout)
        {
            let quantity = listing.item.quantity;
            let bought = self.try_buyout(listing, bidder, quantity).await?;
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
        let debit = new_op();
        self.wallet.debit(debit, bidder, total).await?;

        let previous = listing.high_bid.clone();
        let mut updated = listing;
        updated.high_bid = Some(Bid {
            bidder: bidder.clone(),
            unit_price,
        });
        if !self
            .save_or_refund(&mut updated, bidder, total, debit)
            .await?
        {
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
        if cmd.item.commodity && cmd.buyout_unit_price.is_none() {
            return Err(AuctionError::CommodityNeedsBuyout);
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

        let withdraw = new_op();
        self.inventory
            .withdraw_item(withdraw, &cmd.seller, &cmd.item)
            .await?;
        if deposit > Money::ZERO
            && let Err(e) = self.wallet.debit(new_op(), &cmd.seller, deposit).await
        {
            self.inventory
                .deposit_item(compensation_op(withdraw), &cmd.seller, &cmd.item)
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

    async fn buyout(
        &self,
        buyer: PlayerRef,
        id: ListingId,
        quantity: u32,
    ) -> Result<(), AuctionError> {
        for _ in 0..MAX_ATTEMPTS {
            let listing = self.load_open(id).await?;
            if self.try_buyout(listing, &buyer, quantity).await? {
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
                Some(bid) => {
                    let sale = Sale {
                        buyer: &bid.bidder,
                        unit_price: bid.unit_price,
                        quantity: listing.item.quantity,
                        deposit_refund: listing.deposit,
                    };
                    self.pay_out(&listing, &sale).await?;
                }
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

/// What a buyer takes out of a listing: the items, their price and the share of the
/// deposit that goes back to the seller.
struct Sale<'a> {
    buyer: &'a PlayerRef,
    unit_price: Money,
    quantity: u32,
    deposit_refund: Money,
}

// Effects of a committed listing change, named in their `OpId`.
pub(crate) const REFUND_BID: &str = "refund_bid";
pub(crate) const DELIVER_ITEM: &str = "deliver_item";
pub(crate) const PAY_SELLER: &str = "pay_seller";
pub(crate) const COMMISSION: &str = "commission";
pub(crate) const RETURN_ITEM: &str = "return_item";
pub(crate) const FORFEIT_DEPOSIT: &str = "forfeit_deposit";

/// Id of a call that starts a new attempt, such as the debit of a bid. A retried use case
/// is a new attempt, so it gets a new id.
pub(crate) fn new_op() -> OpId {
    OpId(Uuid::now_v7())
}

/// Id of one effect of a committed listing change. `listing` is the saved listing: its id
/// and version name the change, so every replay of the effect gets the same id.
pub(crate) fn effect_op(listing: &Listing, effect: &str) -> OpId {
    let name = format!("{}/{effect}", listing.version);
    OpId(Uuid::new_v5(&listing.id.0, name.as_bytes()))
}

/// Id of the call that undoes the call `op`, so a replayed compensation is ignored too.
pub(crate) fn compensation_op(op: OpId) -> OpId {
    OpId(Uuid::new_v5(&op.0, b"compensation"))
}

fn repo_error(error: RepoError) -> AuctionError {
    match error {
        RepoError::Conflict => AuctionError::Conflict,
        RepoError::Other(message) => AuctionError::Repository(message),
    }
}
