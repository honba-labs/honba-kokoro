//! Pure Domain Repository Traits & DDD Interfaces (ADR 0032, Phase 7.1).
//!
//! Enforces strict Dependency Inversion:
//! - Pure domain interfaces and Value Objects are owned strictly by the Domain Heart (`honba-kokoro`).
//! - Storage engines (PostgreSQL, SQLite, in-memory) implement these traits without leaking
//!   persistence/driver abstractions (e.g. `sqlx`) into domain business logic.
//! - Traits are object-safe (`Send + Sync`) with pure zero-I/O semantics.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use honba_core::{
    Currency, InstrumentId, Money, OrderSide, OrderStatus, OrderType, PositionSide, Price,
    Quantity, TimeInForce,
};

// ——————————————————————————————————————————————————————————————————————————
// Domain Identifiers (Value Objects)
// ——————————————————————————————————————————————————————————————————————————

/// Strongly-typed Client Order Identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ClientOrderId(pub String);

impl ClientOrderId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ClientOrderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for ClientOrderId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for ClientOrderId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<uuid::Uuid> for ClientOrderId {
    fn from(u: uuid::Uuid) -> Self {
        Self(u.to_string())
    }
}

/// Strongly-typed Algorithm Identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AlgoId(pub String);

impl AlgoId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AlgoId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for AlgoId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for AlgoId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Strongly-typed Trading Account Identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AccountId(pub String);

impl AccountId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for AccountId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for AccountId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Strongly-typed Ledger Transaction Identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TransactionId(pub String);

impl TransactionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TransactionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for TransactionId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for TransactionId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Canonical ledger account code in the Chart of Accounts.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AccountCode(pub String);

impl AccountCode {
    pub const CASH_AT_BROKER: &'static str = "Assets:Cash:BrokerAccount";
    pub const HOLDINGS_EQUITY_DELIVERY: &'static str = "Assets:Holdings:EquityDelivery";
    pub const LIABILITIES_MARGIN_PAYABLE: &'static str = "Liabilities:MarginPayable";
    pub const OPENING_CAPITAL: &'static str = "Equity:OpeningCapital";
    pub const EXPENSES_BROKERAGE: &'static str = "Expenses:Brokerage";
    pub const EXPENSES_TAXES_STT: &'static str = "Expenses:Taxes:STT";
    pub const EXPENSES_TAXES_GST: &'static str = "Expenses:Taxes:GST";
    pub const EXPENSES_TAXES_STAMP_DUTY: &'static str = "Expenses:Taxes:StampDuty";
    pub const EXPENSES_EXCHANGE_FEES: &'static str = "Expenses:ExchangeFees";
    pub const EXPENSES_SEBI_FEES: &'static str = "Expenses:SEBIFees";
    pub const EXPENSES_DP_CHARGES: &'static str = "Expenses:DPCharges";
    pub const INCOME_REALIZED_PNL: &'static str = "Income:RealizedPnL";

    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AccountCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for AccountCode {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for AccountCode {
    fn from(s: String) -> Self {
        Self(s)
    }
}

// ——————————————————————————————————————————————————————————————————————————
// Domain Error
// ——————————————————————————————————————————————————————————————————————————

/// Comprehensive domain-layer error enumeration.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum DomainError {
    #[error("Entity not found: {0}")]
    NotFound(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Unbalanced ledger: debits total {0} != credits total {1}")]
    UnbalancedLedger(Decimal, Decimal),
    #[error("Insufficient balance: {0}")]
    InsufficientBalance(String),
    #[error("Illegal state transition: {0}")]
    IllegalStateTransition(String),
    #[error("Concurrency collision: {0}")]
    ConcurrencyConflict(String),
    #[error("Storage failure: {0}")]
    Storage(String),
}

// ——————————————————————————————————————————————————————————————————————————
// Domain Aggregates & Records
// ——————————————————————————————————————————————————————————————————————————

/// Canonical order record stored and queried via [`OrderRepository`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderRecord {
    pub client_order_id: ClientOrderId,
    pub broker_order_id: Option<String>,
    pub algo_id: Option<AlgoId>,
    pub account_id: AccountId,
    pub instrument: InstrumentId,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub time_in_force: TimeInForce,
    pub price: Option<Price>,
    pub quantity: Quantity,
    pub filled_quantity: Quantity,
    pub avg_fill_price: Option<Price>,
    pub status: OrderStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl OrderRecord {
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::PendingNew | OrderStatus::New | OrderStatus::PartiallyFilled
        )
    }
}

/// Canonical account record stored and queried via [`AccountRepository`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountRecord {
    pub account_id: AccountId,
    pub algo_id: Option<AlgoId>,
    pub cash: Money,
    pub status: String,
    pub updated_at: DateTime<Utc>,
}

/// An individual tax-lot tracked via strict FIFO settlement accounting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxLot {
    pub lot_id: String,
    pub account_id: AccountId,
    pub instrument: InstrumentId,
    pub side: PositionSide,
    pub entry_price: Price,
    pub quantity: Quantity,
    pub remaining_quantity: Quantity,
    pub entry_date: NaiveDate,
    pub created_at: DateTime<Utc>,
}

impl TaxLot {
    pub fn is_exhausted(&self) -> bool {
        self.remaining_quantity.value() <= Decimal::ZERO
    }
}

/// Aggregate position record representing an account's net exposure in an instrument.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PositionRecord {
    pub account_id: AccountId,
    pub instrument: InstrumentId,
    pub side: PositionSide,
    pub quantity: Quantity,
    pub avg_price: Price,
    pub realized_pnl: Money,
    pub unrealized_pnl: Money,
    pub lots: Vec<TaxLot>,
    pub updated_at: DateTime<Utc>,
}

/// Fill report confirming execution of an order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FillReport {
    pub trade_id: String,
    pub order_id: ClientOrderId,
    pub account_id: AccountId,
    pub instrument: InstrumentId,
    pub side: OrderSide,
    pub price: Price,
    pub quantity: Quantity,
    pub fee: Money,
    pub timestamp: DateTime<Utc>,
}

/// Realized gain/loss details for a matched tax lot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchedLot {
    pub lot_id: String,
    pub matched_quantity: Quantity,
    pub entry_price: Price,
    pub exit_price: Price,
    pub realized_pnl: Money,
    pub holding_period_days: i64,
    pub is_long_term: bool,
}

/// Double-entry posting side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PostingSide {
    Debit,
    Credit,
}

/// Double-entry posting entry (debit or credit leg).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostingEntry {
    pub account: AccountCode,
    pub amount: Money,
}

/// A balanced double-entry financial journal transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalTransaction {
    pub id: TransactionId,
    pub timestamp: DateTime<Utc>,
    pub description: String,
    pub debits: Vec<PostingEntry>,
    pub credits: Vec<PostingEntry>,
}

impl JournalTransaction {
    pub fn new(
        id: impl Into<TransactionId>,
        timestamp: DateTime<Utc>,
        description: impl Into<String>,
        debits: Vec<PostingEntry>,
        credits: Vec<PostingEntry>,
    ) -> Result<Self, DomainError> {
        let tx = Self {
            id: id.into(),
            timestamp,
            description: description.into(),
            debits,
            credits,
        };
        tx.validate()?;
        Ok(tx)
    }

    pub fn validate(&self) -> Result<(), DomainError> {
        if self.debits.is_empty() || self.credits.is_empty() {
            return Err(DomainError::Validation(
                "Transaction must contain at least one debit and one credit leg".to_string(),
            ));
        }

        let mut total_debit = Decimal::ZERO;
        for leg in &self.debits {
            if leg.amount.amount <= Decimal::ZERO {
                return Err(DomainError::Validation(
                    "Debit amount must be strictly positive".to_string(),
                ));
            }
            total_debit += leg.amount.amount;
        }

        let mut total_credit = Decimal::ZERO;
        for leg in &self.credits {
            if leg.amount.amount <= Decimal::ZERO {
                return Err(DomainError::Validation(
                    "Credit amount must be strictly positive".to_string(),
                ));
            }
            total_credit += leg.amount.amount;
        }

        if total_debit != total_credit {
            return Err(DomainError::UnbalancedLedger(total_debit, total_credit));
        }

        Ok(())
    }
}

// ——————————————————————————————————————————————————————————————————————————
// Pure Domain Repository Traits (Dependency Inversion)
// ——————————————————————————————————————————————————————————————————————————

/// Pure domain repository managing order lifecycle and state retrieval.
pub trait OrderRepository: Send + Sync {
    /// Retrieve an order by its canonical client order ID.
    fn get_order(&self, id: &ClientOrderId) -> Result<Option<OrderRecord>, DomainError>;

    /// Save or update an order record idempotently.
    fn save_order(&mut self, record: &OrderRecord) -> Result<(), DomainError>;

    /// List all currently active (unfilled / partially filled) orders for a given algo ID.
    fn list_active_orders(&self, algo_id: &AlgoId) -> Result<Vec<OrderRecord>, DomainError>;
}

/// Pure domain repository managing trading accounts.
pub trait AccountRepository: Send + Sync {
    /// Retrieve an account by its canonical identifier.
    fn get_account(&self, id: &AccountId) -> Result<Option<AccountRecord>, DomainError>;

    /// Save or update an account record idempotently.
    fn save_account(&mut self, account: &AccountRecord) -> Result<(), DomainError>;

    /// List all registered trading accounts.
    fn list_accounts(&self) -> Result<Vec<AccountRecord>, DomainError>;
}

/// Pure domain repository managing multi-account FIFO tax-lot positions and matching.
pub trait PositionRepository: Send + Sync {
    /// Retrieve all open positions for a given trading account.
    fn get_positions(&self, account_id: &AccountId) -> Result<Vec<PositionRecord>, DomainError>;

    /// Save a new cost-basis lot for an account.
    fn save_lot(&mut self, account_id: &AccountId, lot: &TaxLot) -> Result<(), DomainError>;

    /// Match an incoming fill report against open tax-lots using strict FIFO matching.
    fn match_lots(&mut self, fill: &FillReport) -> Result<Vec<MatchedLot>, DomainError>;
}

/// Pure domain repository managing immutable double-entry ledger transactions and balances.
pub trait LedgerRepository: Send + Sync {
    /// Post a balanced journal transaction to the general ledger.
    fn post_entry(&mut self, tx: &JournalTransaction) -> Result<TransactionId, DomainError>;

    /// Retrieve the current instantaneous balance of a specific ledger account code.
    fn get_balance(&self, account: &AccountCode) -> Result<Money, DomainError>;
}

// ——————————————————————————————————————————————————————————————————————————
// In-Memory Reference / Mock Implementations
// ——————————————————————————————————————————————————————————————————————————

/// Thread-safe in-memory order repository for pure domain simulation and tests.
#[derive(Debug, Clone, Default)]
pub struct InMemoryOrderRepository {
    orders: HashMap<ClientOrderId, OrderRecord>,
}

impl InMemoryOrderRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl OrderRepository for InMemoryOrderRepository {
    fn get_order(&self, id: &ClientOrderId) -> Result<Option<OrderRecord>, DomainError> {
        Ok(self.orders.get(id).cloned())
    }

    fn save_order(&mut self, record: &OrderRecord) -> Result<(), DomainError> {
        self.orders
            .insert(record.client_order_id.clone(), record.clone());
        Ok(())
    }

    fn list_active_orders(&self, algo_id: &AlgoId) -> Result<Vec<OrderRecord>, DomainError> {
        let mut active = Vec::new();
        for order in self.orders.values() {
            if order.is_active() && order.algo_id.as_ref() == Some(algo_id) {
                active.push(order.clone());
            }
        }
        active.sort_by_key(|a| a.created_at);
        Ok(active)
    }
}

/// In-memory account repository for simulation and tests.
#[derive(Debug, Clone, Default)]
pub struct InMemoryAccountRepository {
    accounts: HashMap<AccountId, AccountRecord>,
}

impl InMemoryAccountRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl AccountRepository for InMemoryAccountRepository {
    fn get_account(&self, id: &AccountId) -> Result<Option<AccountRecord>, DomainError> {
        Ok(self.accounts.get(id).cloned())
    }

    fn save_account(&mut self, account: &AccountRecord) -> Result<(), DomainError> {
        self.accounts
            .insert(account.account_id.clone(), account.clone());
        Ok(())
    }

    fn list_accounts(&self) -> Result<Vec<AccountRecord>, DomainError> {
        let mut list: Vec<_> = self.accounts.values().cloned().collect();
        list.sort_by(|a, b| a.account_id.cmp(&b.account_id));
        Ok(list)
    }
}

/// In-memory position repository with deterministic FIFO tax-lot matching.
#[derive(Debug, Clone, Default)]
pub struct InMemoryPositionRepository {
    lots_by_account: HashMap<AccountId, Vec<TaxLot>>,
    positions_by_account: HashMap<AccountId, HashMap<InstrumentId, PositionRecord>>,
}

impl InMemoryPositionRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl PositionRepository for InMemoryPositionRepository {
    fn get_positions(&self, account_id: &AccountId) -> Result<Vec<PositionRecord>, DomainError> {
        let list = self
            .positions_by_account
            .get(account_id)
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default();
        Ok(list)
    }

    fn save_lot(&mut self, account_id: &AccountId, lot: &TaxLot) -> Result<(), DomainError> {
        self.lots_by_account
            .entry(account_id.clone())
            .or_default()
            .push(lot.clone());

        // Update PositionRecord aggregate
        let account_positions = self
            .positions_by_account
            .entry(account_id.clone())
            .or_default();
        let pos = account_positions
            .entry(lot.instrument.clone())
            .or_insert_with(|| PositionRecord {
                account_id: account_id.clone(),
                instrument: lot.instrument.clone(),
                side: lot.side,
                quantity: Quantity::ZERO,
                avg_price: Price::ZERO,
                realized_pnl: Money::inr(Decimal::ZERO),
                unrealized_pnl: Money::inr(Decimal::ZERO),
                lots: Vec::new(),
                updated_at: lot.created_at,
            });

        pos.quantity = Quantity::new(pos.quantity.value() + lot.quantity.value());
        pos.lots.push(lot.clone());
        pos.updated_at = lot.created_at;
        Ok(())
    }

    fn match_lots(&mut self, fill: &FillReport) -> Result<Vec<MatchedLot>, DomainError> {
        let lots = self
            .lots_by_account
            .entry(fill.account_id.clone())
            .or_default();

        let mut matched_lots = Vec::new();
        let mut remaining_fill = fill.quantity.value();
        let fill_date = fill.timestamp.date_naive();

        for lot in lots.iter_mut() {
            if lot.instrument != fill.instrument || lot.is_exhausted() {
                continue;
            }

            // Only match against opposing side (Long vs Sell, Short vs Buy)
            let is_closing = matches!(
                (lot.side, fill.side),
                (PositionSide::Long, OrderSide::Sell) | (PositionSide::Short, OrderSide::Buy)
            );

            if !is_closing {
                continue;
            }

            let lot_avail = lot.remaining_quantity.value();
            let match_qty_dec = remaining_fill.min(lot_avail);
            if match_qty_dec <= Decimal::ZERO {
                continue;
            }

            let entry_p = lot.entry_price.value();
            let exit_p = fill.price.value();
            let pnl_unit = match lot.side {
                PositionSide::Long => exit_p - entry_p,
                PositionSide::Short => entry_p - exit_p,
            };
            let realized_pnl = Money::inr(pnl_unit * match_qty_dec);

            let holding_days = (fill_date - lot.entry_date).num_days();
            let is_long_term = holding_days > 365;

            matched_lots.push(MatchedLot {
                lot_id: lot.lot_id.clone(),
                matched_quantity: Quantity::new(match_qty_dec),
                entry_price: lot.entry_price,
                exit_price: fill.price,
                realized_pnl,
                holding_period_days: holding_days,
                is_long_term,
            });

            lot.remaining_quantity = Quantity::new(lot_avail - match_qty_dec);
            remaining_fill -= match_qty_dec;

            if remaining_fill <= Decimal::ZERO {
                break;
            }
        }

        // Update PositionRecord aggregate
        if let Some(pos_map) = self.positions_by_account.get_mut(&fill.account_id) {
            if let Some(pos) = pos_map.get_mut(&fill.instrument) {
                let total_matched: Decimal = matched_lots
                    .iter()
                    .map(|m| m.matched_quantity.value())
                    .sum();
                let total_realized_pnl: Decimal =
                    matched_lots.iter().map(|m| m.realized_pnl.amount).sum();

                let new_qty = (pos.quantity.value() - total_matched).max(Decimal::ZERO);
                pos.quantity = Quantity::new(new_qty);
                pos.realized_pnl = Money::inr(pos.realized_pnl.amount + total_realized_pnl);
                pos.lots.retain(|l| !l.is_exhausted());
                pos.updated_at = fill.timestamp;
            }
        }

        Ok(matched_lots)
    }
}

/// In-memory general ledger repository tracking double-entry transactions and balances.
#[derive(Debug, Clone, Default)]
pub struct InMemoryLedgerRepository {
    transactions: Vec<JournalTransaction>,
    balances: BTreeMap<AccountCode, Decimal>,
    base_currency: Currency,
}

impl InMemoryLedgerRepository {
    pub fn new(base_currency: Currency) -> Self {
        Self {
            transactions: Vec::new(),
            balances: BTreeMap::new(),
            base_currency,
        }
    }
}

impl LedgerRepository for InMemoryLedgerRepository {
    fn post_entry(&mut self, tx: &JournalTransaction) -> Result<TransactionId, DomainError> {
        tx.validate()?;

        for deb in &tx.debits {
            *self.balances.entry(deb.account.clone()).or_default() += deb.amount.amount;
        }
        for cred in &tx.credits {
            *self.balances.entry(cred.account.clone()).or_default() -= cred.amount.amount;
        }

        self.transactions.push(tx.clone());
        Ok(tx.id.clone())
    }

    fn get_balance(&self, account: &AccountCode) -> Result<Money, DomainError> {
        let bal = self.balances.get(account).copied().unwrap_or(Decimal::ZERO);
        Ok(Money {
            amount: bal,
            currency: self.base_currency,
        })
    }
}
