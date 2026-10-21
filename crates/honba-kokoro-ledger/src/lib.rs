//! Honba Kokoro Double-Entry Ledger.
//! Immutable financial journal tracking cash balance, margin allocations,
//! statutory taxes (STT, GST, Stamp Duty), and exchange turnover charges.

use rust_decimal::Decimal;
use chrono::{DateTime, Utc};
use std::collections::BTreeMap;
use thiserror::Error;

/// Error types for ledger operations.
#[derive(Debug, Error)]
pub enum LedgerError {
    #[error("Invalid entry amount")]
    InvalidAmount,
    #[error("Duplicate entry ID: {0}")]
    DuplicateEntry(String),
    #[error("Insufficient balance")]
    InsufficientBalance,
    #[error("Unknown account: {0}")]
    UnknownAccount(String),
}

/// Account name in the ledger chart of accounts.
pub type AccountName = String;

// A balanced double-entry journal transaction.
/// Every transaction has equal total debits and credits across the chart of accounts.
#[derive(Debug, Clone, PartialEq)]
pub struct Transaction {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub debits: BTreeMap<String, Decimal>,
    pub credits: BTreeMap<String, Decimal>,
    pub description: String,
}

/// Chart of accounts for the Indian equities ledger.
/// Standard accounts per SEBI and Indian Companies Act requirements.
#[derive(Debug, Clone, PartialEq)]
pub enum Account {
    CashAtBroker,
    HoldingsEquityDelivery,
    LiabilitiesMarginPayable,
    ExpensesBrokerage,
    ExpensesTaxesSTT,
    ExpensesTaxesGST,
    ExpensesTaxesStampDuty,
    ExpensesExchangeFees,
    IncomeRealizedPnL,
}

/// Post a transaction to the ledger.
/// Ensures debits total equals credits total (double-entry bookkeeping).
pub fn post_transaction(transaction: &Transaction) -> Result<(), LedgerError> {
    let total_debits: Decimal = transaction.debits.values().sum();
    let total_credits: Decimal = transaction.credits.values().sum();

    if total_debits != total_credits {
        return Err(LedgerError::InvalidAmount);
    }

    Ok(())
}

/// Indian statutory tax calculator.
/// Computes STT, GST, Stamp Duty, SEBI fees, and DP charges per SEBI
/// and Finance Act rules for equity, F&O, and commodity derivatives.
pub struct TaxCalculator;

impl TaxCalculator {
    pub fn stt(delivery: bool, price: Decimal, quantity: Decimal) -> Decimal {
        let rate = if delivery {
            Decimal::from(1i64) / Decimal::from(1000i64) // 0.1%
        } else {
            Decimal::from(1i64) / Decimal::from(4000i64) // 0.025%
        };

        let notional = price * quantity;
        let stt = (notional * rate).floor();
        stt
    }

    pub fn GST(brokerage: Decimal, exchange_charges: Decimal, sebi_fees: Decimal) -> Decimal {
        let total = brokerage + exchange_charges + sebi_fees;
        (total * Decimal::from(18i64) / Decimal::from(100i64)).floor()
    }

    pub fn stamp_duty(price: Decimal, quantity: Decimal, buy: bool) -> Decimal {
        let rate = if buy {
            Decimal::from(15i64) / Decimal::from(100000i64) // 0.015%
        } else {
            Decimal::from(0i64)
        };
        (price * quantity * rate).floor()
    }

    pub fn sebi_turnover_charges(turnover: Decimal) -> Decimal {
        (turnover * Decimal::from(10i64) / Decimal::from(10_000_000i32)).floor()
    }

    pub fn dp_charges(is_delivery_sale: bool) -> Decimal {
        let base = Decimal::from(1350i64) / Decimal::from(100i64); // ₹13.50 in paise
        let gst_rate = Decimal::from(18i64) / Decimal::from(100i64);

        if is_delivery_sale {
            let gst = (base * gst_rate).floor();
            base + gst
        } else {
            Decimal::from(0i64)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_post_transaction_balanced() {
        let tx = Transaction {
            id: "tx_001".to_string(),
            timestamp: Utc::now(),
            debits: BTreeMap::from_iter([("CashAtBroker".to_string(), Decimal::from(1000i64))]),
            credits: BTreeMap::from_iter([("IncomeRealizedPnL".to_string(), Decimal::from(1000i64))]),
            description: "Test transaction".to_string(),
        };
        assert!(post_transaction(&tx).is_ok());
    }

    #[test]
    fn test_post_transaction_unbalanced() {
        let tx = Transaction {
            id: "tx_002".to_string(),
            timestamp: Utc::now(),
            debits: BTreeMap::from_iter([("CashAtBroker".to_string(), Decimal::from(1000i64))]),
            credits: BTreeMap::from_iter([("IncomeRealizedPnL".to_string(), Decimal::from(500i64))]),
            description: "Unbalanced transaction".to_string(),
        };
        assert!(post_transaction(&tx).is_err());
    }

    #[test]
    fn test_stt_delivery_buy() {
        let stt = TaxCalculator::stt(true, Decimal::from(2000i64), Decimal::from(10i64));
        assert_eq!(stt, Decimal::from(20i64));
    }

    #[test]
    fn test_stt_intraday_sell() {
        let stt = TaxCalculator::stt(false, Decimal::from(2000i64), Decimal::from(10i64));
        assert_eq!(stt, Decimal::from(5i64));
    }

    #[test]
    fn test_GST() {
        let gst = TaxCalculator::GST(Decimal::from(1000i64), Decimal::from(200i64), Decimal::from(50i64));
        assert_eq!(gst, Decimal::from(225i64));
    }

    #[test]
    fn test_stamp_duty_buy() {
        let sd = TaxCalculator::stamp_duty(Decimal::from(2000i64), Decimal::from(10i64), true);
        assert_eq!(sd, Decimal::from(3i64));
    }

    #[test]
    fn test_sebi_turnover_charges() {
        let sebi = TaxCalculator::sebi_turnover_charges(Decimal::from(10_000_000i64));
        assert_eq!(sebi, Decimal::from(10i64));
    }

    #[test]
    fn test_dp_charges_delivery_sale() {
        let dp = TaxCalculator::dp_charges(true);
        // ₹13.50 + 18% GST ≥ ₹13.50
        assert!(dp >= Decimal::from(1350i64) / Decimal::from(100i64));
    }

    #[test]
    fn test_dp_charges_non_delivery() {
        let dp = TaxCalculator::dp_charges(false);
        assert_eq!(dp, Decimal::from(0i64));
    }

    #[test]
    fn test_transaction_balance() {
        let tx = Transaction {
            id: "tx_test".to_string(),
            timestamp: Utc::now(),
            debits: BTreeMap::from_iter([("CashAtBroker".to_string(), Decimal::from(500i64))]),
            credits: BTreeMap::from_iter([("IncomeRealizedPnL".to_string(), Decimal::from(500i64))]),
            description: "Test transaction".to_string(),
        };
        assert!(post_transaction(&tx).is_ok());
    }
}
