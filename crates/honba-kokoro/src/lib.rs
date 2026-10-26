//! `honba-kokoro`: Pure Domain Heart of the Honba Quantitative Trading Ecosystem.
//!
//! Subsystem: Heart (Domain Heart)
//! Primary Responsibility: Order Management System (`oms`), FIFO tax-lot engine (`portfolio`),
//! double-entry general ledger (`ledger`), pure domain repository traits (`repository`),
//! and dynamic position sizing service hook (`sizing`).
//!
//! Architectural Invariants:
//! - Pure zero-I/O: No sockets, no filesystem I/O, no database drivers.
//! - Zero floats: Exact 128-bit decimal arithmetic (`rust_decimal::Decimal`).
//! - Temporal determinism: All calculations receive explicit `DateTime<Utc>`.
//! - Strict Dependency Inversion: Domain layer owns repository traits; infrastructure implements them.

pub use honba_kokoro_ledger as ledger;
pub use honba_kokoro_oms as oms;
pub use honba_portfolio as portfolio;

pub mod repository;
pub mod sizing;

pub use repository::{
    AccountCode, AccountId, AccountRecord, AccountRepository, AlgoId, ClientOrderId, DomainError,
    FillReport, InMemoryAccountRepository, InMemoryLedgerRepository, InMemoryOrderRepository,
    InMemoryPositionRepository, JournalTransaction, LedgerRepository, MatchedLot, OrderRecord,
    OrderRepository, PositionRecord, PositionRepository, PostingEntry, PostingSide, TaxLot,
    TransactionId,
};
pub use sizing::{Portfolio, evaluate_risk_budget};
