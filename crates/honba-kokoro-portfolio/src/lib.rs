//! INR portfolio, FIFO PnL, T+1 settlement.
use honba_kokoro_types::{InstrumentId, Price, Quantity};
use rust_decimal::Decimal;

#[derive(Debug, Default, Clone)]
pub struct Portfolio {
    pub cash_inr: Decimal,
    pub settled_cash_inr: Decimal,
    pub holdings: Vec<Holding>,
}

#[derive(Debug, Clone)]
pub struct Holding {
    pub instrument: InstrumentId,
    pub quantity: Quantity,
    pub settled_quantity: Quantity,
    pub avg_cost: Price,
}
