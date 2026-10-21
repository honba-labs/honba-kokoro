//! Honba Indian equities order management system (OMS).
//! Uses honba-core canonical types for Decimal-accurate order processing.
//! References fills, marks, and portfolio state from honba-kokoro-portfolio.
//! The engine never places an order directly: it validates, prices, and forwards
//! order requests to brokers (ADR 0032 §7).

use honba_core::{InstrumentId, OrderSide, OrderType, Price, Quantity, TimeInForce};
use rust_decimal::Decimal;
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;
use chrono::{Utc, NaiveDate};

/// Order state machine (pre-existing foundation from Phase 1).
#[derive(Debug, Clone)]
pub struct Order {
    pub id: Uuid,
    pub idempotency_key: String,
    pub instrument: InstrumentId,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: Quantity,
    pub tif: TimeInForce,
    pub limit_price: Option<Price>,
    pub state: OrderState,
}

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

/// Order acknowledgment (Phase 6 addition).
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
    pub created_at: Utc,
    pub filled_quantity: u64,
    pub average_fill_price: Option<Price>,
    pub commission: Money,
    pub fees: Money,
}

/// Order status enum (Phase 6 addition).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    Pending, Received, Partial, Filled, Cancelled, Rejected, Expired,
}

/// Manual error implementation (no thiserror dependency on OrderError itself,
/// but thiserror is used for TransitionError above).
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

impl std::fmt::Display for OrderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
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

/// Core order validation logic.
pub fn check_risk_limits(
    request: &OrderRequest,
    portfolio: &PortfolioRiskState,
) -> RiskCheckResult {
    let mut approved = true;
    let mut reason: Option<String> = None;
    let mut adjusted_quantity = None;

    if request.quantity > 1000000 {
        approved = false;
        reason = Some("Quantity exceeds maximum per order".to_string());
        adjusted_quantity = None;
    }

    if portfolio.today_pnl < -portfolio.max_daily_loss {
        approved = false;
        reason = Some("Daily loss limit exceeded".to_string());
        adjusted_quantity = None;
    }

    RiskCheckResult {
        approved,
        reason,
        adjusted_quantity,
    }
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
    pub created_at: Utc,
    pub cash_flow: i64,
}

/// Core order construction logic - returns OrderAck directly.
fn _build_order_ack_inner(
    request: &OrderRequest,
    portfolio: &PortfolioRiskState,
) -> OrderAck {
    let order_id = format!("ORDER_{}", Uuid::new_v4());
    let client_order_id = format!("CLIENT_{}", Uuid::new_v4());
    let created_at = Utc::now();

    OrderAck {
        order_id,
        client_order_id,
        instrument: request.instrument,
        side: request.side,
        quantity: request.quantity,
        order_type: request.order_type,
        time_in_force: request.time_in_force,
        limit_price: request.limit_price,
        status: OrderStatus::Pending,
        created_at,
        filled_quantity: 0,
        average_fill_price: None,
        commission: Money {
            amount: Decimal::from(0i64),
            currency: honba_core::Currency::Inr,
        },
        fees: Money {
            amount: Decimal::from(0i64),
            currency: honba_core::Currency::Inr,
        },
    }
}

/// Validate order parameters (returns error string if invalid, ok if valid).
pub fn validate_order_params(
    request: &OrderRequest,
    portfolio: &PortfolioRiskState,
) -> Result<(), String> {
    // Check quantity limit
    if request.quantity > 1000000 {
        return Err("Quantity exceeds maximum per order".to_string());
    }

    // Check daily loss limit
    if portfolio.today_pnl < -portfolio.max_daily_loss {
        return Err("Daily loss limit exceeded".to_string());
    }

    Ok(())
}

/// Submit an order to the broker after risk validation.
/// In a production system, this would call the broker API.
/// For now, it validates and returns an acknowledgment.
pub async fn submit_order(
    request: OrderRequest,
    portfolio: &PortfolioRiskState,
) -> Result<OrderAck, OrderError> {
    // Perform risk checks
    let risk = check_risk_limits(&request, portfolio);

    if !risk.approved {
        return Err(OrderError::RiskLimitCheckFailed(
            risk.reason.unwrap_or_default(),
        ));
    }

    // Generate order acknowledgment
    let ack = _build_order_ack_inner(&request, portfolio);

    // In production: call broker API here
    // For now, return the pending acknowledgment
    Ok(ack)
}

/// Cancel an existing order
/// In production: call broker API with order_id
/// For now, returns a canceled acknowledgment
pub async fn cancel_order(
    order_id: String,
    client_order_id: String,
) -> Result<OrderAck, OrderError> {
    // In production: call broker API to cancel order
    // For now, return a canceled acknowledgment
    Ok(OrderAck {
        order_id,
        client_order_id,
        instrument: InstrumentId::new(honba_core::Exchange::Nse, "UNKNOWN"),
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
            currency: honba_core::Currency::Inr,
        },
        fees: Money {
            amount: Decimal::from(0i64),
            currency: honba_core::Currency::Inr,
        },
    })
}

/// Process a fill report from the broker
/// This integrates with the portfolio engine's T+1 settlement and tax system
pub fn process_fill_report(fill: FillReport) -> Result<(), OrderError> {
    // In production: integrate with honba-kokoro-portfolio::settle_trades()
    // and TaxSummary::from_fills()
    // For now, just validate the fill report

    if fill.fill_quantity == 0 {
        return Err(OrderError::InvalidParameters);
    }

    if fill.commission.amount < Decimal::from(0i64) {
        return Err(OrderError::InvalidParameters);
    }

    // Successfully processed fill
    Ok(())
}

/// Market data snapshot for order pricing
#[derive(Debug, Clone)]
pub struct MarketData {
    pub instrument: InstrumentId,
    pub bid: Price,
    pub ask: Price,
    pub last: Price,
    pub volume: u64,
    pub timestamp: Utc,
}

/// OMS state and configuration
#[derive(Debug, Clone)]
pub struct OMSConfig {
    /// Maximum notional per order (in INR)
    pub max_notional_per_order: Option<Decimal>,
    /// Maximum quantity per order
    pub max_quantity_per_order: Option<u64>,
    /// Maximum daily loss (in INR)
    pub max_daily_loss: Option<Money>,
    /// Price collar in basis points (e.g., 100 = 1%)
    pub price_collar_bps: Option<u32>,
    /// Default time in force for market orders
    pub default_time_in_force: TimeInForce,
}

/// OMS state snapshot
#[derive(Debug, Clone)]
pub struct OMSSnapshot {
    pub total_orders: u64,
    pub total_filled: u64,
    pub total_cancelled: u64,
    pub total_commission: Money,
    pub total_fees: Money,
    pub today_pnl: Money,
}

/// Fill report from broker
#[derive(Debug, Clone)]
pub struct FillReport {
    pub trade: Trade,
    pub fill_price: Price,
    pub fill_quantity: u64,
    pub commission: Money,
    pub fees: Money,
    pub status: OrderStatus,
}

/// Trade execution record
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

/// Cost basis lot for tax computation
#[derive(Debug, Clone)]
pub struct CostBasisLot {
    pub lot_id: String,
    pub entry_price: Price,
    pub entry_date: NaiveDate,
    pub quantity: Quantity,
    pub side: OrderSide,
}

// Re-export honba-core types for convenience.
pub use honba_core::{Currency, Money, Price, Quantity, InstrumentId, OrderSide, OrderType, TimeInForce};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transition_valid() {
        use OrderState::*;
        let result = transition(Pending, Open);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Open);
    }

    #[test]
    fn test_transition_invalid() {
        use OrderState::*;
        let result = transition(Pending, Filled);
        assert!(result.is_err());
    }

    #[test]
    fn test_order_ack_creation() {
        let id = honba_core::InstrumentId::new(honba_core::Exchange::Nse, "RELIANCE");
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
                currency: honba_core::Currency::Inr,
            },
            fees: Money {
                amount: Decimal::from(0i64),
                currency: honba_core::Currency::Inr,
            },
        };

        assert_eq!(ack.order_id, "ORDER_TEST");
        assert_eq!(ack.status, OrderStatus::Pending);
        assert_eq!(ack.filled_quantity, 0);
    }

    #[test]
    fn test_build_order_ack() {
        let id = honba_core::InstrumentId::new(honba_core::Exchange::Nse, "RELIANCE");
        let portfolio = PortfolioRiskState {
            cash: Money {
                amount: Decimal::from(1000000i64),
                currency: honba_core::Currency::Inr,
            },
            positions: BTreeMap::new(),
            today_pnl: Money {
                amount: Decimal::from(0i64),
                currency: honba_core::Currency::Inr,
            },
            max_daily_loss: Money {
                amount: Decimal::from(50000i64),
                currency: honba_core::Currency::Inr,
            },
            price_collar_bps: Some(100),
        };

        let request = OrderRequest {
            instrument: id,
            side: OrderSide::Long,
            quantity: 10,
            order_type: OrderType::Market,
            time_in_force: TimeInForce::Day,
            limit_price: None,
            client_order_id: "CLIENT_TEST".to_string(),
            created_at: Utc::now(),
            cash_flow: 20000,
        };

        let ack = _build_order_ack_inner(&request, &portfolio);
        assert_eq!(ack.status, OrderStatus::Pending);
        assert_eq!(ack.filled_quantity, 0);
    }

    #[test]
    fn test_risk_check_approves() {
        let id = honba_core::InstrumentId::new(honba_core::Exchange::Nse, "RELIANCE");
        let portfolio = PortfolioRiskState {
            cash: Money {
                amount: Decimal::from(1000000i64),
                currency: honba_core::Currency::Inr,
            },
            positions: BTreeMap::new(),
            today_pnl: Money {
                amount: Decimal::from(0i64),
                currency: honba_core::Currency::Inr,
            },
            max_daily_loss: Money {
                amount: Decimal::from(50000i64),
                currency: honba_core::Currency::Inr,
            },
            price_collar_bps: Some(100),
        };

        let request = OrderRequest {
            instrument: id,
            side: OrderSide::Long,
            quantity: 10,
            order_type: OrderType::Market,
            time_in_force: TimeInForce::Day,
            limit_price: None,
            client_order_id: "CLIENT_TEST".to_string(),
            created_at: Utc::now(),
            cash_flow: 20000,
        };

        let result = check_risk_limits(&request, &portfolio);
        assert!(risk.approved);
    }

    #[test]
    fn test_validate_order_params() {
        let id = honba_core::InstrumentId::new(honba_core::Exchange::Nse, "RELIANCE");
        let portfolio = PortfolioRiskState {
            cash: Money {
                amount: Decimal::from(1000000i64),
                currency: honba_core::Currency::Inr,
            },
            positions: BTreeMap::new(),
            today_pnl: Money {
                amount: Decimal::from(0i64),
                currency: honba_core::Currency::Inr,
            },
            max_daily_loss: Money {
                amount: Decimal::from(50000i64),
                currency: honba_core::Currency::Inr,
            },
            price_collar_bps: Some(100),
        };

        let request = OrderRequest {
            instrument: id,
            side: OrderSide::Long,
            quantity: 10,
            order_type: OrderType::Market,
            time_in_force: TimeInForce::Day,
            limit_price: None,
            client_order_id: "CLIENT_TEST".to_string(),
            created_at: Utc::now(),
            cash_flow: 20000,
        };

        let result = validate_order_params(&request, &portfolio);
        assert!(result.is_ok());
    }

    #[test]
    fn test_submit_order_pending() {
        let id = honba_core::InstrumentId::new(honba_core::Exchange::Nse, "RELIANCE");
        let portfolio = PortfolioRiskState {
            cash: Money {
                amount: Decimal::from(1000000i64),
                currency: honba_core::Currency::Inr,
            },
            positions: BTreeMap::new(),
            today_pnl: Money {
                amount: Decimal::from(0i64),
                currency: honba_core::Currency::Inr,
            },
            max_daily_loss: Money {
                amount: Decimal::from(50000i64),
                currency: honba_core::Currency::Inr,
            },
            price_collar_bps: Some(100),
        };

        let request = OrderRequest {
            instrument: id,
            side: OrderSide::Long,
            quantity: 10,
            order_type: OrderType::Market,
            time_in_force: TimeInForce::Day,
            limit_price: None,
            client_order_id: "CLIENT_TEST".to_string(),
            created_at: Utc::now(),
            cash_flow: 20000,
        };

        let result = submit_order(request, &portfolio);
        match result {
            Ok(ack) => {
                assert_eq!(ack.status, OrderStatus::Pending);
                assert_eq!(ack.filled_quantity, 0);
            }
            Err(e) => panic!("Order should be approved: {:?}", e),
        }
    }

    #[test]
    fn test_cancel_order() {
        let result = cancel_order("ORDER_123".to_string(), "CLIENT_456".to_string());
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
        let id = honba_core::InstrumentId::new(honba_core::Exchange::Nse, "RELIANCE");
        let fill = FillReport {
            trade: Trade {
                trade_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                settle_date: NaiveDate::from_ymd_opt(2026, 1, 2).unwrap(),
                instrument: id,
                side: OrderSide::Long,
                quantity: 10,
                price: Price::from_rupees("2000").unwrap(),
                cash_flow: 20000,
            },
            fill_price: Price::from_rupees("2000").unwrap(),
            fill_quantity: 10,
            commission: Money {
                amount: Decimal::from(10i64),
                currency: honba_core::Currency::Inr,
            },
            fees: Money {
                amount: Decimal::from(5i64),
                currency: honba_core::Currency::Inr,
            },
            status: OrderStatus::Filled,
        };

        let result = process_fill_report(fill);
        assert!(result.is_ok());
    }
}