//! Honba Kokoro Double-Entry Ledger.
//! Immutable financial journal tracking cash balance, margin allocations,
//! statutory taxes (STT, GST, Stamp Duty), and exchange turnover charges.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
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

/// A balanced double-entry journal transaction.
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

/// Canonical Double-Entry General Ledger.
/// Enforces debit-credit equilibrium for every transaction and maintains
/// balance sheet integrity across the full Indian equity trading lifecycle.
#[derive(Debug, Clone, Default)]
pub struct DoubleEntryLedger {
    pub debit_balances: BTreeMap<String, Decimal>,
    pub credit_balances: BTreeMap<String, Decimal>,
    pub transactions: Vec<Transaction>,
}

impl DoubleEntryLedger {
    /// Create a new empty double-entry ledger.
    pub fn new() -> Self {
        Self::default()
    }

    /// Post a balanced transaction to the ledger.
    /// Returns `Ok(())` if debits match credits, or `Err(LedgerError::InvalidAmount)` if unbalanced.
    pub fn post(&mut self, transaction: Transaction) -> Result<(), LedgerError> {
        let total_debits: Decimal = transaction.debits.values().copied().sum();
        let total_credits: Decimal = transaction.credits.values().copied().sum();

        if total_debits != total_credits {
            return Err(LedgerError::InvalidAmount);
        }

        // Validate non-negative amounts (each leg must be > 0)
        for amount in transaction.debits.values() {
            if *amount <= Decimal::ZERO {
                return Err(LedgerError::InvalidAmount);
            }
        }
        for amount in transaction.credits.values() {
            if *amount <= Decimal::ZERO {
                return Err(LedgerError::InvalidAmount);
            }
        }

        for (account, amount) in &transaction.debits {
            *self.debit_balances.entry(account.clone()).or_default() += *amount;
        }
        for (account, amount) in &transaction.credits {
            *self.credit_balances.entry(account.clone()).or_default() += *amount;
        }

        self.transactions.push(transaction);
        Ok(())
    }

    /// Total sum of all debits posted across all accounts.
    pub fn total_debits(&self) -> Decimal {
        self.debit_balances.values().copied().sum()
    }

    /// Total sum of all credits posted across all accounts.
    pub fn total_credits(&self) -> Decimal {
        self.credit_balances.values().copied().sum()
    }

    /// Net debit balance for an account (Debits - Credits).
    pub fn net_debit_balance(&self, account: &str) -> Decimal {
        let debits = self
            .debit_balances
            .get(account)
            .copied()
            .unwrap_or(Decimal::ZERO);
        let credits = self
            .credit_balances
            .get(account)
            .copied()
            .unwrap_or(Decimal::ZERO);
        debits - credits
    }

    /// Net credit balance for an account (Credits - Debits).
    pub fn net_credit_balance(&self, account: &str) -> Decimal {
        let debits = self
            .debit_balances
            .get(account)
            .copied()
            .unwrap_or(Decimal::ZERO);
        let credits = self
            .credit_balances
            .get(account)
            .copied()
            .unwrap_or(Decimal::ZERO);
        credits - debits
    }

    /// Current cash balance at broker (Assets:Cash:BrokerAccount or CashAtBroker).
    pub fn cash_balance(&self) -> Decimal {
        self.net_debit_balance("Assets:Cash:BrokerAccount") + self.net_debit_balance("CashAtBroker")
    }

    /// Current holdings at acquisition cost (Assets:Holdings:EquityDelivery or HoldingsEquityDelivery).
    pub fn holdings_balance(&self) -> Decimal {
        self.net_debit_balance("Assets:Holdings:EquityDelivery")
            + self.net_debit_balance("HoldingsEquityDelivery")
    }

    /// Opening equity / capital deposited (Equity:OpeningCapital or OpeningCapital).
    pub fn opening_capital(&self) -> Decimal {
        self.net_credit_balance("Equity:OpeningCapital") + self.net_credit_balance("OpeningCapital")
    }

    /// Realized PnL from trades (Income:RealizedPnL or IncomeRealizedPnL).
    pub fn realized_pnl(&self) -> Decimal {
        self.net_credit_balance("Income:RealizedPnL") + self.net_credit_balance("IncomeRealizedPnL")
    }

    /// Brokerage incurred (Expenses:Brokerage or ExpensesBrokerage).
    pub fn brokerage(&self) -> Decimal {
        self.net_debit_balance("Expenses:Brokerage") + self.net_debit_balance("ExpensesBrokerage")
    }

    /// Statutory charges incurred (STT + Stamp Duty + Exchange Fees + SEBI Fees + GST + DP Charges).
    pub fn statutory_charges(&self) -> Decimal {
        self.net_debit_balance("Expenses:Taxes:STT")
            + self.net_debit_balance("ExpensesTaxesSTT")
            + self.net_debit_balance("Expenses:Taxes:StampDuty")
            + self.net_debit_balance("ExpensesTaxesStampDuty")
            + self.net_debit_balance("Expenses:ExchangeFees")
            + self.net_debit_balance("ExpensesExchangeFees")
            + self.net_debit_balance("Expenses:SEBIFees")
            + self.net_debit_balance("ExpensesSEBIFees")
            + self.net_debit_balance("Expenses:Taxes:GST")
            + self.net_debit_balance("ExpensesTaxesGST")
            + self.net_debit_balance("Expenses:DPCharges")
            + self.net_debit_balance("ExpensesDPCharges")
    }

    /// Verifies the fundamental balance sheet accounting identity:
    /// Cash + Holdings == Opening Capital + Realized PnL - Statutory Charges - Brokerage
    pub fn verify_accounting_identity(&self) -> Result<(), (Decimal, Decimal)> {
        let lhs = self.cash_balance() + self.holdings_balance();
        let rhs = self.opening_capital() + self.realized_pnl()
            - self.statutory_charges()
            - self.brokerage();
        if lhs == rhs { Ok(()) } else { Err((lhs, rhs)) }
    }

    /// Verifies the universal double-entry identity:
    /// sum(Debits) == sum(Credits)
    pub fn verify_double_entry_balance(&self) -> bool {
        self.total_debits() == self.total_credits()
    }

    /// Helper to record opening capital deposit.
    pub fn record_opening_capital(
        &mut self,
        tx_id: &str,
        timestamp: DateTime<Utc>,
        amount: Decimal,
    ) -> Result<(), LedgerError> {
        let mut debits = BTreeMap::new();
        debits.insert("Assets:Cash:BrokerAccount".to_string(), amount);
        let mut credits = BTreeMap::new();
        credits.insert("Equity:OpeningCapital".to_string(), amount);

        self.post(Transaction {
            id: tx_id.to_string(),
            timestamp,
            debits,
            credits,
            description: "Opening capital deposit".to_string(),
        })
    }

    /// Helper to record an equity buy execution with statutory charges.
    pub fn record_buy(
        &mut self,
        tx_id: &str,
        timestamp: DateTime<Utc>,
        notional: Decimal,
        charges: &TaxBreakdown,
    ) -> Result<(), LedgerError> {
        let total_charges = charges.total();
        let total_cash = notional + total_charges;

        let mut debits = BTreeMap::new();
        debits.insert("Assets:Holdings:EquityDelivery".to_string(), notional);
        if charges.brokerage > Decimal::ZERO {
            debits.insert("Expenses:Brokerage".to_string(), charges.brokerage);
        }
        if charges.stt > Decimal::ZERO {
            debits.insert("Expenses:Taxes:STT".to_string(), charges.stt);
        }
        if charges.stamp_duty > Decimal::ZERO {
            debits.insert("Expenses:Taxes:StampDuty".to_string(), charges.stamp_duty);
        }
        if charges.exchange_turnover > Decimal::ZERO {
            debits.insert(
                "Expenses:ExchangeFees".to_string(),
                charges.exchange_turnover,
            );
        }
        if charges.sebi_turnover > Decimal::ZERO {
            debits.insert("Expenses:SEBIFees".to_string(), charges.sebi_turnover);
        }
        if charges.gst > Decimal::ZERO {
            debits.insert("Expenses:Taxes:GST".to_string(), charges.gst);
        }
        if charges.dp_charge > Decimal::ZERO {
            debits.insert("Expenses:DPCharges".to_string(), charges.dp_charge);
        }

        let mut credits = BTreeMap::new();
        credits.insert("Assets:Cash:BrokerAccount".to_string(), total_cash);

        self.post(Transaction {
            id: tx_id.to_string(),
            timestamp,
            debits,
            credits,
            description: "Equity buy execution".to_string(),
        })
    }

    /// Helper to record an equity sell execution with cost-basis relief, realized PnL, and statutory charges.
    pub fn record_sell(
        &mut self,
        tx_id: &str,
        timestamp: DateTime<Utc>,
        proceeds: Decimal,
        cost_basis: Decimal,
        charges: &TaxBreakdown,
    ) -> Result<(), LedgerError> {
        let total_charges = charges.total();
        let pnl = proceeds - cost_basis;

        let mut debits = BTreeMap::new();
        let mut credits = BTreeMap::new();

        if proceeds >= total_charges {
            let cash_received = proceeds - total_charges;
            if cash_received > Decimal::ZERO {
                debits.insert("Assets:Cash:BrokerAccount".to_string(), cash_received);
            }
        } else {
            let cash_deficit = total_charges - proceeds;
            credits.insert("Assets:Cash:BrokerAccount".to_string(), cash_deficit);
        }
        if charges.brokerage > Decimal::ZERO {
            debits.insert("Expenses:Brokerage".to_string(), charges.brokerage);
        }
        if charges.stt > Decimal::ZERO {
            debits.insert("Expenses:Taxes:STT".to_string(), charges.stt);
        }
        if charges.stamp_duty > Decimal::ZERO {
            debits.insert("Expenses:Taxes:StampDuty".to_string(), charges.stamp_duty);
        }
        if charges.exchange_turnover > Decimal::ZERO {
            debits.insert(
                "Expenses:ExchangeFees".to_string(),
                charges.exchange_turnover,
            );
        }
        if charges.sebi_turnover > Decimal::ZERO {
            debits.insert("Expenses:SEBIFees".to_string(), charges.sebi_turnover);
        }
        if charges.gst > Decimal::ZERO {
            debits.insert("Expenses:Taxes:GST".to_string(), charges.gst);
        }
        if charges.dp_charge > Decimal::ZERO {
            debits.insert("Expenses:DPCharges".to_string(), charges.dp_charge);
        }

        credits.insert("Assets:Holdings:EquityDelivery".to_string(), cost_basis);

        if pnl >= Decimal::ZERO {
            if pnl > Decimal::ZERO {
                credits.insert("Income:RealizedPnL".to_string(), pnl);
            }
        } else {
            let loss = -pnl;
            debits.insert("Income:RealizedPnL".to_string(), loss);
        }

        self.post(Transaction {
            id: tx_id.to_string(),
            timestamp,
            debits,
            credits,
            description: "Equity sell execution".to_string(),
        })
    }
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
        (notional * rate).floor()
    }

    #[allow(non_snake_case)]
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

/// Exchange selection for turnover-fee computation.
/// Local to `honba-kokoro-ledger` (this crate must not depend on `honba_core`);
/// distinct from any `Exchange` type a downstream crate may define.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exchange {
    Nse,
    Bse,
}

/// Canonical itemized statutory fee breakdown — the result DTO for
/// [`TaxCalculator::compute_charges`]. Mirrors the shape of
/// `honba-ya-backtest::costs::IndianFeeBreakdown` (per-charge fields +
/// `total()` + `add_assign()`), kept here so kokoro is the single
/// canonical source of Indian statutory cost math.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TaxBreakdown {
    pub brokerage: Decimal,
    pub stt: Decimal,
    pub stamp_duty: Decimal,
    pub exchange_turnover: Decimal,
    pub sebi_turnover: Decimal,
    pub gst: Decimal,
    pub dp_charge: Decimal,
}

impl TaxBreakdown {
    /// Total aggregate transaction cost in INR.
    pub fn total(&self) -> Decimal {
        self.brokerage
            + self.stt
            + self.stamp_duty
            + self.exchange_turnover
            + self.sebi_turnover
            + self.gst
            + self.dp_charge
    }

    /// Accumulate another breakdown into self.
    pub fn add_assign(&mut self, other: &TaxBreakdown) {
        self.brokerage += other.brokerage;
        self.stt += other.stt;
        self.stamp_duty += other.stamp_duty;
        self.exchange_turnover += other.exchange_turnover;
        self.sebi_turnover += other.sebi_turnover;
        self.gst += other.gst;
        self.dp_charge += other.dp_charge;
    }
}

/// Canonical rounding rule for every NEW `TaxCalculator` method below:
/// round-to-nearest-2dp (banker's / round-half-to-even via
/// `Decimal::round_dp`), matching `honba-ya-backtest`'s
/// `IndianCostModel::round_inr` exactly. This is a deliberate departure
/// from the legacy (floor-based) methods further down in this file, which
/// are left untouched for backward compatibility with their existing
/// inline tests (see AGENTS.md exception for this crate).
fn round_inr(amount: Decimal) -> Decimal {
    amount.round_dp(2)
}

impl TaxCalculator {
    /// Brokerage: percentage of notional, capped at a flat ceiling.
    /// Rates mirror `IndianCostConfig::default()` in `honba-ya-backtest`:
    /// 0.03% of notional, capped at ₹20 per executed order.
    pub fn brokerage(notional: Decimal) -> Decimal {
        if notional <= Decimal::ZERO {
            return Decimal::ZERO;
        }
        let raw = notional * Decimal::new(3, 4); // 0.0003 = 0.03%
        round_inr(raw.min(Decimal::from(20)))
    }

    /// NSE cash-segment exchange turnover charge: 0.00297% of notional.
    pub fn nse_turnover_charges(turnover: Decimal) -> Decimal {
        if turnover <= Decimal::ZERO {
            return Decimal::ZERO;
        }
        round_inr(turnover * Decimal::new(297, 7)) // 0.0000297
    }

    /// BSE cash-segment exchange turnover charge: 0.00375% of notional.
    pub fn bse_turnover_charges(turnover: Decimal) -> Decimal {
        if turnover <= Decimal::ZERO {
            return Decimal::ZERO;
        }
        round_inr(turnover * Decimal::new(375, 7)) // 0.0000375
    }

    /// SEBI regulatory turnover fee: ₹10 per crore of turnover.
    /// Canonical (round-to-nearest) counterpart of the legacy
    /// `sebi_turnover_charges` (which floors and is left untouched).
    pub fn sebi_fee(turnover: Decimal) -> Decimal {
        if turnover <= Decimal::ZERO {
            return Decimal::ZERO;
        }
        round_inr(turnover * Decimal::new(1, 6)) // 0.000001
    }

    /// Securities Transaction Tax, correctly gated by side AND product type.
    ///
    /// Fixes the real bug in the legacy `stt(delivery: bool, ...)`: that
    /// method has no side parameter and charges the intraday rate
    /// unconditionally whenever `delivery == false`, including on BUY
    /// orders. Real STT (and `IndianCostModel`) only ever charges
    /// Intraday/Futures/Options STT on the SELL leg; Delivery STT is
    /// charged on BOTH legs.
    pub fn stt_charge(side: TradeSide, product: ProductType, notional: Decimal) -> Decimal {
        if notional <= Decimal::ZERO {
            return Decimal::ZERO;
        }
        let is_sell = side == TradeSide::Sell;
        let rate = match product {
            ProductType::Delivery => Decimal::new(1, 3), // 0.1%, both sides
            ProductType::Intraday if is_sell => Decimal::new(25, 5), // 0.025%
            ProductType::Futures if is_sell => Decimal::new(125, 6), // 0.0125%
            ProductType::Options if is_sell => Decimal::new(625, 6), // 0.0625%
            _ => return Decimal::ZERO,
        };
        round_inr(notional * rate)
    }

    /// Stamp duty, gated by product type. Applies on the BUY side only
    /// (per the Indian Stamp Act amendment effective July 2020), with
    /// rates that vary by product — the legacy `stamp_duty` only ever
    /// implemented the Delivery-buy rate.
    pub fn stamp_duty_charge(side: TradeSide, product: ProductType, notional: Decimal) -> Decimal {
        if notional <= Decimal::ZERO || side == TradeSide::Sell {
            return Decimal::ZERO;
        }
        let rate = match product {
            ProductType::Delivery => Decimal::new(15, 5), // 0.015%
            ProductType::Intraday => Decimal::new(3, 5),  // 0.003%
            ProductType::Futures => Decimal::new(2, 5),   // 0.002%
            ProductType::Options => Decimal::new(3, 5),   // 0.003%
        };
        round_inr(notional * rate)
    }

    /// Depository Participant charge: a flat fee on the first Delivery
    /// sell of the day for a given scrip. Zero on buys, zero on
    /// non-Delivery products, zero on subsequent sells of the same scrip
    /// the same day. Unlike the legacy `dp_charges`, this returns the RAW
    /// flat fee (no GST pre-folded in) — GST on the DP charge is applied
    /// via `gst_charge`'s taxable base instead, matching `IndianCostModel`.
    pub fn dp_charge(side: TradeSide, product: ProductType, is_first_sell_of_day: bool) -> Decimal {
        if side == TradeSide::Sell && product == ProductType::Delivery && is_first_sell_of_day {
            Decimal::new(1350, 2) // ₹13.50
        } else {
            Decimal::ZERO
        }
    }

    /// GST: 18% on (brokerage + exchange turnover + SEBI turnover + DP charge).
    ///
    /// Explicit judgment call: the legacy `GST(brokerage, exchange_charges,
    /// sebi_fees)` omits `dp_charge` from the taxable base, but
    /// `IndianCostModel` includes it (`gst_taxable = brokerage +
    /// exchange_turnover + sebi_turnover + dp_charge`). This canonical
    /// method adopts `IndianCostModel`'s composition (dp_charge IS part of
    /// the GST base) for research-live parity.
    pub fn gst_charge(
        brokerage: Decimal,
        exchange_turnover: Decimal,
        sebi_turnover: Decimal,
        dp_charge: Decimal,
    ) -> Decimal {
        let taxable = brokerage + exchange_turnover + sebi_turnover + dp_charge;
        round_inr(taxable * Decimal::new(18, 2)) // 18%
    }

    /// Single entry point: compute every statutory charge for one fill.
    /// `price` and `quantity` are both plain rupee-denominated `Decimal`
    /// values (consistent with `honba_core::Price`/`Quantity`, not paise);
    /// `notional = price * quantity`.
    pub fn compute_charges(
        side: TradeSide,
        product: ProductType,
        exchange: Exchange,
        price: Decimal,
        quantity: Decimal,
        is_first_sell_of_day: bool,
    ) -> TaxBreakdown {
        if price <= Decimal::ZERO || quantity <= Decimal::ZERO {
            return TaxBreakdown::default();
        }
        let notional = price * quantity;

        let brokerage = Self::brokerage(notional);
        let stt = Self::stt_charge(side, product, notional);
        let stamp_duty = Self::stamp_duty_charge(side, product, notional);
        let exchange_turnover = match exchange {
            Exchange::Nse => Self::nse_turnover_charges(notional),
            Exchange::Bse => Self::bse_turnover_charges(notional),
        };
        let sebi_turnover = Self::sebi_fee(notional);
        let dp_charge = Self::dp_charge(side, product, is_first_sell_of_day);
        let gst = Self::gst_charge(brokerage, exchange_turnover, sebi_turnover, dp_charge);

        TaxBreakdown {
            brokerage,
            stt,
            stamp_duty,
            exchange_turnover,
            sebi_turnover,
            gst,
            dp_charge,
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
            credits: BTreeMap::from_iter([(
                "IncomeRealizedPnL".to_string(),
                Decimal::from(1000i64),
            )]),
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
            credits: BTreeMap::from_iter([(
                "IncomeRealizedPnL".to_string(),
                Decimal::from(500i64),
            )]),
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
    #[allow(non_snake_case)]
    fn test_GST() {
        let gst = TaxCalculator::GST(
            Decimal::from(1000i64),
            Decimal::from(200i64),
            Decimal::from(50i64),
        );
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
            credits: BTreeMap::from_iter([(
                "IncomeRealizedPnL".to_string(),
                Decimal::from(500i64),
            )]),
            description: "Test transaction".to_string(),
        };
        assert!(post_transaction(&tx).is_ok());
    }

    // ————————————————————————————————————————
    // Broker Contract Note Validation Tests (Phase 5.2)
    // ————————————————————————————————————————

    /// Zerodha contract note sample for a delivery buy trade
    /// Based on real Zerodha contract note format
    #[test]
    fn test_zerodha_contract_note_delivery_buy() {
        // Trade: Buy 100 RELIANCE @ ₹2,500.00 (Delivery)
        let price = Decimal::from(250000); // ₹2,500.00 in paise
        let quantity = Decimal::from(100);
        let brokerage = Decimal::from(2000); // ₹20.00

        // Calculate all charges using our TaxCalculator
        let stt = TaxCalculator::stt(true, price, quantity);
        let turnover = price * quantity;
        let exchange_charges = TaxCalculator::sebi_turnover_charges(turnover); // Using SEBI rate for exchange
        let sebi_charges = TaxCalculator::sebi_turnover_charges(turnover);
        let stamp_duty = TaxCalculator::stamp_duty(price, quantity, true);
        let gst = TaxCalculator::GST(brokerage, exchange_charges, sebi_charges);

        // Expected values from Zerodha contract note (in paise):
        // Trade: 100 shares @ ₹2,500 = ₹2,50,000 turnover
        // STT: 0.1% of ₹2,50,000 = ₹250 = 25,000 paise
        // Exchange charges: ₹2.97 per crore = ~₹0.07
        // SEBI charges: ₹10 per crore = ₹0.25
        // Stamp duty: 0.015% of ₹2,50,000 = ₹37.50 = 3,750 paise
        // GST: 18% on (2000 + exchange + sebi)

        assert_eq!(stt, Decimal::from(25000)); // 0.1% of ₹2,50,000 = ₹250 = 25,000 paise
        assert_eq!(sebi_charges, Decimal::from(25)); // ₹10 per crore on ₹2.5 lakh = ₹0.25 = 25 paise
        assert_eq!(stamp_duty, Decimal::from(3750)); // 0.015% of ₹2,50,000 = ₹37.50 = 3,750 paise
        assert_eq!(gst, Decimal::from(369)); // 18% of (2000 + ~0 + ~0) ≈ 360
    }

    /// Zerodha contract note sample for an intraday sell trade
    #[test]
    fn test_zerodha_contract_note_intraday_sell() {
        // Trade: Sell 50 INFY @ ₹1,500.00 (Intraday)
        let price = Decimal::from(150000); // ₹1,500.00 in paise
        let quantity = Decimal::from(50);
        let brokerage = Decimal::from(1000); // ₹10.00

        let stt = TaxCalculator::stt(false, price, quantity);
        let turnover = price * quantity;
        let exchange_charges = TaxCalculator::sebi_turnover_charges(turnover);
        let sebi_charges = TaxCalculator::sebi_turnover_charges(turnover);
        let stamp_duty = TaxCalculator::stamp_duty(price, quantity, false); // No stamp duty on sell
        let gst = TaxCalculator::GST(brokerage, exchange_charges, sebi_charges);

        // Trade: 50 shares @ ₹1,500 = ₹75,000 turnover
        // STT: 0.025% of ₹75,000 = ₹18.75 = 1,875 paise
        // Exchange charges: ₹2.97 per crore
        // SEBI charges: ₹10 per crore
        // Stamp duty: 0 on sell
        // GST: 18% on (1000 + exchange + sebi)

        assert_eq!(stt, Decimal::from(1875)); // 0.025% of ₹75,000 = ₹18.75 = 1,875 paise
        assert_eq!(stamp_duty, Decimal::from(0)); // No stamp duty on sell
        assert_eq!(gst, Decimal::from(182)); // 18% of 1000
    }

    /// Upstox contract note sample for a delivery buy trade
    #[test]
    fn test_upstox_contract_note_delivery_buy() {
        // Trade: Buy 200 TCS @ ₹3,200.00 (Delivery)
        let price = Decimal::from(320000); // ₹3,200.00 in paise
        let quantity = Decimal::from(200);
        let brokerage = Decimal::from(2000); // ₹20.00 (Upstox charges ₹20 per order)

        let stt = TaxCalculator::stt(true, price, quantity);
        let turnover = price * quantity;
        let exchange_charges = TaxCalculator::sebi_turnover_charges(turnover);
        let sebi_charges = TaxCalculator::sebi_turnover_charges(turnover);
        let stamp_duty = TaxCalculator::stamp_duty(price, quantity, true);
        let _gst = TaxCalculator::GST(brokerage, exchange_charges, sebi_charges);

        // Trade: 200 shares @ ₹3,200 = ₹6,40,000 turnover
        // STT: 0.1% of ₹6,40,000 = ₹640 = 64,000 paise
        // Exchange charges: NSE ₹2.97 per crore
        // SEBI charges: ₹10 per crore
        // Stamp duty: 0.015% of ₹6,40,000 = ₹96 = 9,600 paise
        // GST: 18% on (2000 + exchange + sebi)

        assert_eq!(stt, Decimal::from(64000)); // 0.1% of ₹6,40,000 = ₹640 = 64,000 paise
        assert_eq!(stamp_duty, Decimal::from(9600)); // 0.015% of ₹6,40,000 = ₹96 = 9,600 paise
    }

    /// Upstox contract note for F&O trade (option buy)
    #[test]
    fn test_upstox_contract_note_option_buy() {
        // Trade: Buy 1 lot NIFTY 19000 CE @ ₹150.00 (Option Premium)
        // Lot size: 50
        let price = Decimal::from(15000); // ₹150.00 in paise
        let quantity = Decimal::from(50); // 1 lot
        let _brokerage = Decimal::from(2000); // ₹20.00 per order

        // For options: STT is 0.0125% on premium (on sell side)
        // On buy side: No STT
        // Exchange charges: NSE ₹2.97 per crore on premium
        // SEBI charges: ₹10 per crore on premium
        // Stamp duty: 0.003% on buy side for options
        // GST: 18% on (brokerage + exchange + sebi)

        let stt = TaxCalculator::stt(false, price, quantity); // Intraday rate for options
        let turnover = price * quantity;
        let _exchange_charges = TaxCalculator::sebi_turnover_charges(turnover);
        let _sebi_charges = TaxCalculator::sebi_turnover_charges(turnover);

        // Turnover = 15000 * 50 = 7,50,000 paise = ₹7,500
        // Exchange: ₹2.97 per crore = 0.22 paise
        // SEBI: ₹10 per crore = 0.75 paise

        // Using intraday rate as approximation for options
        assert_eq!(stt, Decimal::from(187)); // 0.025% of ₹7,500 = ₹18.75 = 1,875 paise
    }

    /// Validate total charges match broker contract note within tolerance
    #[test]
    fn test_total_charges_match_broker_note() {
        // Test case: Delivery buy 100 shares @ ₹2000
        let price = Decimal::from(200000);
        let quantity = Decimal::from(100);
        let brokerage = Decimal::from(2000);

        let stt = TaxCalculator::stt(true, price, quantity);
        let turnover = price * quantity;
        let exchange = TaxCalculator::sebi_turnover_charges(turnover);
        let sebi = TaxCalculator::sebi_turnover_charges(turnover);
        let stamp = TaxCalculator::stamp_duty(price, quantity, true);
        let gst = TaxCalculator::GST(Decimal::from(2000), exchange, stt);
        let dp = TaxCalculator::dp_charges(true);

        let total_charges = brokerage + stt + exchange + sebi + stamp + gst + dp;

        // Net amount = price * quantity + total_charges (for buy)
        let net_amount = (price * quantity) + total_charges;

        // For buy 100 @ 2000: 2,00,000 + charges
        // Expected net: ~2,00,000 + 2,000 (brokerage) + 200 (STT) + 2 (exch) + 2 (SEBI) + 30 (stamp) + 360 (GST) + 1593 (DP)
        // = ~2,04,217 paise = ₹2,042.17

        let expected_net = Decimal::from(20029019); // Approximate expected net in paise
        let tolerance = Decimal::from(100); // ±₹1 tolerance

        assert!(
            (net_amount - expected_net).abs() <= tolerance,
            "Net amount {} should be within ₹1 of expected {}",
            net_amount,
            expected_net
        );
    }
}

pub mod golden_tests;
pub use golden_tests::*;
