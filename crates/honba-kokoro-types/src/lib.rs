//! Core value types for honba (NSE/BSE).
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Exchange { Nse, Bse }

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InstrumentId {
    pub exchange: Exchange,
    pub symbol: String,
    pub isin: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Price(pub Decimal);

impl Price {
    pub fn from_rupees(s: &str) -> Result<Self, TypeError> {
        let d: Decimal = s.parse().map_err(|_| TypeError::InvalidDecimal)?;
        Ok(Self(d))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quantity(pub u64);

impl Quantity {
    pub fn new(n: u64) -> Self { Self(n) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide { Buy, Sell }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType { Market, Limit(Price) }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeInForce { Day, Ioc }

#[derive(Debug, Error)]
pub enum TypeError {
    #[error("invalid decimal")]
    InvalidDecimal,
}
