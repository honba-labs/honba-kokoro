//! Order state machine.
use honba_kokoro_types::{InstrumentId, OrderSide, OrderType, Price, Quantity, TimeInForce};
use thiserror::Error;
use uuid::Uuid;

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
