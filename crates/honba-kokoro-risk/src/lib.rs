//! Pre-trade risk with SEBI OTR awareness.
use async_trait::async_trait;
use rust_decimal::Decimal;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct RiskLimits {
    pub max_position_inr: Decimal,
    pub max_daily_loss_inr: Decimal,
    pub otr_limit_per_day: u32,
}

#[derive(Debug, Error)]
pub enum RiskViolation {
    #[error("position limit exceeded")]
    PositionLimit,
    #[error("daily loss limit exceeded")]
    DailyLoss,
    #[error("OTR limit exceeded")]
    OtrLimit,
}

#[async_trait]
pub trait RiskManager: Send + Sync {
    async fn pre_trade_check(
        &self,
        notional_inr: Decimal,
        scrip: &str,
    ) -> Result<(), RiskViolation>;
}
