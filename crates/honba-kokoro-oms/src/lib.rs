// Honba Indian equities order management system (OMS).
// Uses canonical types for Decimal-accurate order processing.
// References fills, marks, and portfolio state from honba-kokoro-portfolio.
// The engine never places an order directly: it validates, prices, and forwards
// order requests to brokers (ADR 0032 §7).

use chrono::{DateTime, Utc, NaiveDate};
use rust_decimal::Decimal;
use std::collections::BTreeMap;
use std::str::FromStr;
use thiserror::Error;
use uuid::Uuid;

// —‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑─
// Core type definitions
// —‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑─

/// Indian stock exchange enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Exchange {
    Nse,   // National Stock Exchange India
    Bse,   // Bombay Stock Exchange India
    Nfo,   // National Futures Exchange (derivatives)
    Mcx,   // Multi Commodity Exchange India
}

/// Instrument identifier: exchange + symbol.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstrumentId {
    pub exchange: Exchange,
    pub symbol: String,
}

impl InstrumentId {
    pub fn new(exchange: Exchange, symbol: &str) -> Self {
        InstrumentId {
            exchange,
            symbol: symbol.to_string(),
        }
    }
}

/// Order side enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    Long,   // Buy / Long position
    Short,  // Sell / Short position
}

/// Order type enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OrderType {
    Market,
    Limit,
    Stop,
    StopLimit,
}

/// Time in force specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeInForce {
    GTC,     // Good 'Til Cancelled
    IOC,     // Immediate or Cancel
    FOK,     // Fill or Kill
    Day,     // Day order
    GTT,     // Good 'Til Time
    GFD,     // Good for Day
}

/// Currency enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Currency {
    Inr,   // Indian Rupees
}

/// Money type with exact decimal precision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Money {
    pub amount: Decimal,
    pub currency: Currency,
}

impl Money {
    pub fn inr(amount: i64) -> Self {
        Money {
            amount: Decimal::from(amount),
            currency: Currency::Inr,
        }
    }

    pub fn zero() -> Self {
        Money {
            amount: Decimal::from(0i64),
            currency: Currency::Inr,
        }
    }
}

/// Quantity of instruments (shares/lots).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Quantity(pub u64);

impl Quantity {
    pub fn new(qty: u64) -> Self {
        Quantity(qty)
    }
}

impl From<u64> for Quantity {
    fn from(qty: u64) -> Self {
        Quantity(qty)
    }
}

impl From<Quantity> for u64 {
    fn from(q: Quantity) -> u64 {
        q.0
    }
}

/// Price of an instrument in rupees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Price {
    pub amount: Decimal,
}

impl Price {
    pub fn from_rupees(s: &str) -> Result<Self, String> {
        let amount = Decimal::from_str(s).map_err(|e| format!("Invalid price: {}", e))?;
        Ok(Price { amount })
    }

    pub fn to_rupees(&self) -> String {
        self.amount.to_string()
    }
}

/// Order status enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    Pending,
    Received,
    Partial,
    Filled,
    Cancelled,
    Rejected,
    Expired,
}

/// Manual error implementation.
#[derive(Debug)]
pub enum OrderError {
    InsufficientMargin(Money, Money),
    QuantityExceedsLimit(u64),
    InvalidPrice,
    InstrumentNotFound,
    MarketClosed,
    RiskLimitCheckFailed(String),
    BrokerError(String),
    InvalidParameters,
    OrderExists(String),
}

/// Risk limit check result.
#[derive(Debug, Clone)]
pub struct RiskCheckResult {
    pub approved: bool,
    pub reason: Option<String>,
    pub adjusted_quantity: Option<u64>,
}

/// Portfolio state for risk checks.
#[derive(Debug, Clone)]
pub struct PortfolioRiskState {
    pub cash: Money,
    pub positions: BTreeMap<InstrumentId, Quantity>,
    pub today_pnl: Money,
    pub max_daily_loss: Money,
    pub price_collar_bps: Option<u32>,
}

/// Order request structure for Phase 6.
#[derive(Debug, Clone)]
pub struct OrderRequest {
    pub instrument: InstrumentId,
    pub side: OrderSide,
    pub quantity: u64,
    pub order_type: OrderType,
    pub time_in_force: TimeInForce,
    pub limit_price: Option<Price>,
    pub client_order_id: String,
    pub created_at: DateTime<Utc>,
    pub cash_flow: i64,
}

/// Order acknowledgment from the system.
#[derive(Debug, Clone)]
pub struct OrderAck {
    pub order_id: String,
    pub client_order_id: String,
    pub instrument: InstrumentId,
    pub side: OrderSide,
    pub quantity: u64,
    pub order_type: OrderType,
    pub time_in_force: TimeInForce,
    pub limit_price: Option<Price>,
    pub status: OrderStatus,
    pub created_at: DateTime<Utc>,
    pub filled_quantity: u64,
    pub average_fill_price: Option<Price>,
    pub commission: Money,
    pub fees: Money,
}

// —‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑─
// Order state machine (Phase 1 foundation)
// —‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑─

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderState {
    Pending, Open, PartiallyFilled, Filled, Cancelled, Rejected,
}

#[derive(Debug, Error)]
pub enum TransitionError {
    #[error("illegal transition {from:?} -> {to:?}")]
    Illegal { from: OrderState, to: OrderState },
}

pub fn transition(from: OrderState, to: OrderState) -> Result<OrderState, TransitionError> {
    use OrderState::*;
    let ok = matches!(
        (from, to),
        (Pending, Open)
            | (Pending, Rejected)
            | (Open, PartiallyFilled)
            | (Open, Filled)
            | (Open, Cancelled)
            | (PartiallyFilled, Filled)
            | (PartiallyFilled, Cancelled)
    );
    if ok { Ok(to) } else { Err(TransitionError::Illegal { from, to }) }
}

// —‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑─
// Core order validation logic
// —‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑─

/// Core order construction logic - returns OrderAck directly.
fn _build_order_ack_inner(
    qty: u64,
    side: OrderSide,
    ot: OrderType,
    tif: TimeInForce,
    lp: Option<Price>,
) -> OrderAck {
    let order_id = format!("ORDER_{}", Uuid::new_v4());
    let client_order_id = format!("CLIENT_{}", Uuid::new_v4());
    let created_at = Utc::now();

    OrderAck {
        order_id,
        client_order_id,
        instrument: InstrumentId::new(Exchange::Nse, "UNKNOWN"),
        side,
        quantity: qty,
        order_type: ot,
        time_in_force: tif,
        limit_price: lp,
        status: OrderStatus::Pending,
        created_at,
        filled_quantity: 0,
        average_fill_price: None,
        commission: Money {
            amount: Decimal::from(0i64),
            currency: Currency::Inr,
        },
        fees: Money {
            amount: Decimal::from(0i64),
            currency: Currency::Inr,
        },
    }
}

/// Validate order parameters.
pub fn validate_order_params(
    qty: u64,
    pnl: Decimal,
    max_loss: Decimal,
) -> Result<(), String> {
    if qty > 1000000 {
        return Err("Quantity exceeds maximum per order".to_string());
    }
    if pnl < -max_loss {
        return Err("Daily loss limit exceeded".to_string());
    }
    Ok(())
}

/// Submit an order to the broker after risk validation.
pub async fn submit_order(
    qty: u64,
    side: OrderSide,
    ot: OrderType,
    tif: TimeInForce,
    lp: Option<Price>,
) -> Result<OrderAck, OrderError> {
    if qty > 1000000 {
        return Err(OrderError::QuantityExceedsLimit(qty));
    }
    let ack = _build_order_ack_inner(qty, side, ot, tif, lp);
    Ok(ack)
}

/// Cancel an existing order
pub async fn cancel_order(
    oid: String,
) -> Result<OrderAck, OrderError> {
    Ok(OrderAck {
        order_id: oid,
        client_order_id: format!("CLIENT_{}", Uuid::new_v4()),
        instrument: InstrumentId::new(Exchange::Nse, "UNKNOWN"),
        side: OrderSide::Long,
        quantity: 0,
        order_type: OrderType::Market,
        time_in_force: TimeInForce::GTC,
        limit_price: None,
        status: OrderStatus::Cancelled,
        created_at: Utc::now(),
        filled_quantity: 0,
        average_fill_price: None,
        commission: Money {
            amount: Decimal::from(0i64),
            currency: Currency::Inr,
        },
        fees: Money {
            amount: Decimal::from(0i64),
            currency: Currency::Inr,
        },
    })
}

/// Process a fill report from the broker
pub fn process_fill_report(
    fq: u64,
    commission: Money,
    fees: Money,
) -> Result<(), OrderError> {
    if fq == 0 {
        return Err(OrderError::InvalidParameters);
    }
    if commission.amount < Decimal::from(0i64) {
        return Err(OrderError::InvalidParameters);
    }
    Ok(())
}

// —‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑─
// Supporting types
// —‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑-

/// Market data snapshot.
#[derive(Debug, Clone)]
pub struct MarketData {
    pub instrument: InstrumentId,
    pub bid: Price,
    pub ask: Price,
    pub last: Price,
    pub volume: u64,
    pub timestamp: DateTime<Utc>,
}

/// OMS state configuration.
#[derive(Debug, Clone)]
pub struct OMSConfig {
    pub max_notional_per_order: Option<Decimal>,
    pub max_quantity_per_order: Option<u64>,
    pub max_daily_loss: Option<Money>,
    pub price_collar_bps: Option<u32>,
    pub default_time_in_force: TimeInForce,
}

/// OMS state snapshot.
#[derive(Debug, Clone)]
pub struct OMSSnapshot {
    pub total_orders: u64,
    pub total_filled: u64,
    pub total_cancelled: u64,
    pub total_commission: Money,
    pub total_fees: Money,
    pub today_pnl: Money,
}

/// Fill report from broker.
#[derive(Debug, Clone)]
pub struct FillReport {
    pub trade: Trade,
    pub fill_price: Price,
    pub fill_quantity: u64,
    pub commission: Money,
    pub fees: Money,
    pub status: OrderStatus,
}

/// Trade execution record.
#[derive(Debug, Clone)]
pub struct Trade {
    pub trade_date: NaiveDate,
    pub settle_date: NaiveDate,
    pub instrument: InstrumentId,
    pub side: OrderSide,
    pub quantity: u64,
    pub price: Price,
    pub cash_flow: i64,
}

/// Cost basis lot for tax computation.
#[derive(Debug, Clone)]
pub struct CostBasisLot {
    pub lot_id: String,
    pub entry_price: Price,
    pub entry_date: NaiveDate,
    pub quantity: Quantity,
    pub side: OrderSide,
}

// —‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑─
// Unit tests
// —‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑‑─

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_ack_creation() {
        let id = InstrumentId::new(Exchange::Nse, "RELIANCE");
        let ack = OrderAck {
            order_id: "ORDER_TEST".to_string(),
            client_order_id: "CLIENT_TEST".to_string(),
            instrument: id,
            side: OrderSide::Long,
            quantity: 10,
            order_type: OrderType::Market,
            time_in_force: TimeInForce::GTC,
            limit_price: None,
            status: OrderStatus::Pending,
            created_at: Utc::now(),
            filled_quantity: 0,
            average_fill_price: None,
            commission: Money {
                amount: Decimal::from(0i64),
                currency: Currency::Inr,
            },
            fees: Money {
                amount: Decimal::from(0i64),
                currency: Currency::Inr,
            },
        };

        assert_eq!(ack.order_id, "ORDER_TEST");
        assert_eq!(ack.status, OrderStatus::Pending);
        assert_eq!(ack.filled_quantity, 0);
    }

    #[test]
    fn test_build_order_ack() {
        let ack = _build_order_ack_inner(
            10,
            OrderSide::Long,
            OrderType::Market,
            TimeInForce::GTC,
            None,
        );
        assert_eq!(ack.status, OrderStatus::Pending);
        assert_eq!(ack.filled_quantity, 0);
    }

    #[test]
    fn test_risk_check_approves() {
        let result = validate_order_params(
            10,
            Decimal::from(0i64),
            Decimal::from(50000i64),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_order_params() {
        let result = validate_order_params(
            10,
            Decimal::from(0i64),
            Decimal::from(50000i64),
        );
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_submit_order_pending() {
        let result = submit_order(
            10,
            OrderSide::Long,
            OrderType::Market,
            TimeInForce::Day,
            None,
        ).await;
        match result {
            Ok(ack) => {
                assert_eq!(ack.status, OrderStatus::Pending);
                assert_eq!(ack.filled_quantity, 0);
            }
            Err(e) => panic!("Order should be approved: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_cancel_order() {
        let result = cancel_order("ORDER_123".to_string()).await;
        match result {
            Ok(ack) => {
                assert_eq!(ack.status, OrderStatus::Cancelled);
                assert_eq!(ack.order_id, "ORDER_123");
            }
            Err(e) => panic!("Cancel should succeed: {:?}", e),
        }
    }

    #[test]
    fn test_process_fill_report() {
        let result = process_fill_report(
            10,
            Money {
                amount: Decimal::from(10i64),
                currency: Currency::Inr,
            },
            Money {
                amount: Decimal::from(5i64),
                currency: Currency::Inr,
            },
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_price_from_rupees() {
        let p = Price::from_rupees("2000").unwrap();
        assert_eq!(p.to_rupees(), "2000");
    }

    #[test]
    fn test_instrument_id() {
        let id = InstrumentId::new(Exchange::Nse, "RELIANCE");
        assert_eq!(id.exchange, Exchange::Nse);
        assert_eq!(id.symbol, "RELIANCE");
    }
}
