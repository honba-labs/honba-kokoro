//! Risk-Aware Position Sizing Service Hook (ADR 0032, Phase 7.4).
//!
//! Bridges portfolio valuation state with dynamic alpha sizing (`honba-core::sizing`).
//! Evaluates instantaneous free equity, gross leverage ceilings, and single-scrip
//! concentration limits to compute deterministic, risk-budgeted position constraints.

use std::ops::Deref;

use rust_decimal::Decimal;

use honba_core::sizing::RiskBudget;
use honba_core::{Money, Quantity};
use honba_portfolio::PortfolioState;

/// Rich Domain Aggregate representing a risk-governed portfolio.
#[derive(Debug, Clone)]
pub struct Portfolio {
    /// Internal multi-account valuation state.
    pub state: PortfolioState,
    /// Maximum allowed gross portfolio leverage (e.g. 4.0x for intraday MIS, 1.0x for CNC).
    pub max_gross_leverage: Decimal,
    /// Maximum single-scrip concentration limit as fraction of portfolio equity (e.g. 0.25 for 25%).
    pub max_scrip_concentration_pct: Decimal,
    /// Default instrument minimum lot size.
    pub min_lot_size: Quantity,
}

impl Portfolio {
    /// Creates a new risk-governed portfolio with default institutional risk limits
    /// (4.0x gross leverage ceiling, 25% single scrip concentration cap).
    pub fn new(state: PortfolioState) -> Self {
        Self {
            state,
            max_gross_leverage: Decimal::from(4),
            max_scrip_concentration_pct: Decimal::new(25, 2), // 0.25 = 25%
            min_lot_size: Quantity::from_u64(1),
        }
    }

    /// Creates a portfolio with explicit leverage and concentration limits.
    pub fn with_limits(
        state: PortfolioState,
        max_gross_leverage: Decimal,
        max_scrip_concentration_pct: Decimal,
    ) -> Self {
        Self {
            state,
            max_gross_leverage,
            max_scrip_concentration_pct,
            min_lot_size: Quantity::from_u64(1),
        }
    }

    /// Set minimum lot size for sizing calculations.
    pub fn with_min_lot_size(mut self, min_lot: Quantity) -> Self {
        self.min_lot_size = min_lot;
        self
    }
}

impl Deref for Portfolio {
    type Target = PortfolioState;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl From<PortfolioState> for Portfolio {
    fn from(state: PortfolioState) -> Self {
        Self::new(state)
    }
}

/// Pure deterministic service hook calculating the [`RiskBudget`] for `honba-core::sizing`.
///
/// Invariants:
/// 1. Instantaneous free equity = total uncommitted cash across all accounts.
/// 2. Gross leverage = total gross market exposure / total portfolio equity.
/// 3. Scrip concentration limit = `equity * max_scrip_concentration_pct`.
/// 4. Single-order notional is capped by the minimum of concentration limit and remaining leverage headroom.
pub fn evaluate_risk_budget(portfolio: &Portfolio, risk_pct: Decimal) -> RiskBudget {
    let currency = portfolio.state.base_currency;

    let mut total_cash = Decimal::ZERO;
    let mut total_open_notional = Decimal::ZERO;
    let mut total_gross_exposure = Decimal::ZERO;

    for account in portfolio.state.accounts.values() {
        total_cash += account.cash.amount;
        for pos in &account.positions {
            let leg_notional = pos.entry_price.0 * pos.quantity.0;
            total_open_notional += leg_notional;
            total_gross_exposure += leg_notional.abs();
        }
    }

    let free_equity = total_cash.max(Decimal::ZERO);
    let total_equity = (total_cash + total_open_notional).max(Decimal::ZERO);

    // Current gross leverage ratio
    let current_leverage = if total_equity > Decimal::ZERO {
        total_gross_exposure / total_equity
    } else {
        Decimal::ZERO
    };

    // Remaining leverage headroom before hitting max_gross_leverage ceiling
    let leverage_headroom =
        if total_equity > Decimal::ZERO && current_leverage < portfolio.max_gross_leverage {
            (portfolio.max_gross_leverage - current_leverage) * total_equity
        } else {
            Decimal::ZERO
        };

    // Scrip concentration ceiling
    let concentration_cap = total_equity * portfolio.max_scrip_concentration_pct;

    // The maximum permissible position notional is the tighter of the concentration cap
    // and the available leverage headroom
    let max_position_notional_amount = concentration_cap.min(leverage_headroom).max(Decimal::ZERO);

    // Default aggregate portfolio risk percentage (e.g. 5x single trade risk)
    let max_portfolio_risk = (risk_pct * Decimal::from(5)).min(Decimal::ONE);

    RiskBudget {
        equity: Money {
            amount: free_equity,
            currency,
        },
        risk_pct,
        max_position_notional: Some(Money {
            amount: max_position_notional_amount,
            currency,
        }),
        max_position_qty: None,
        max_portfolio_risk_pct: Some(max_portfolio_risk),
        min_lot_size: portfolio.min_lot_size,
        freeze_limit_qty: None,
    }
}
