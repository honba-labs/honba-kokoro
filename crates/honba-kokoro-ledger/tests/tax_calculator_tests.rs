//! External test suite (per AGENTS.md's parallel `tests/` convention) for the
//! NEW canonical `TaxCalculator` surface added to close gaps against
//! `honba-ya-backtest::costs::IndianCostModel`.
//!
//! These tests exercise only the NEW methods (`brokerage`, `nse_turnover_charges`,
//! `bse_turnover_charges`, `sebi_fee`, `stt_charge`, `stamp_duty_charge`,
//! `dp_charge`, `gst_charge`, `compute_charges`, `TaxBreakdown`). The legacy
//! buggy methods (`stt`, `GST`, `stamp_duty`, `sebi_turnover_charges`,
//! `dp_charges`) and their existing inline `#[cfg(test)] mod tests` in
//! `src/lib.rs` are intentionally left untouched — see AGENTS.md exception.
//!
//! Expected values for the parity tests are hand-derived from
//! `honba-ya-backtest/src/costs.rs::IndianCostModel::compute_fees`'s documented
//! formulas (NSE 0.00297% exchange turnover, SEBI ₹10/crore, 18% GST on
//! brokerage+exchange+SEBI+DP, brokerage 0.03% capped at ₹20, STT/stamp-duty
//! rates per product type) since `honba-kokoro-ledger` must not depend on the
//! `honba-ya` workspace (separate repo).

use honba_kokoro_ledger::{Exchange, ProductType, TaxBreakdown, TaxCalculator, TradeSide};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

// ---------------------------------------------------------------------------
// NSE / BSE exchange turnover charges (did not exist before this task)
// ---------------------------------------------------------------------------

#[test]
fn test_nse_turnover_charges_matches_reference_rate() {
    // 100,000 * 0.00297% = 2.97 (exact; matches IndianCostModel's own pinned test value)
    let charge = TaxCalculator::nse_turnover_charges(dec!(100_000.00));
    assert_eq!(charge, dec!(2.97));
}

#[test]
fn test_bse_turnover_charges_matches_reference_rate() {
    // 100,000 * 0.00375% = 3.75
    let charge = TaxCalculator::bse_turnover_charges(dec!(100_000.00));
    assert_eq!(charge, dec!(3.75));
}

#[test]
fn test_rounding_pin_nse_turnover_midpoint_banker_rounding() {
    // 50,000 * 0.00297% = 1.485 exactly -- a genuine rounding tie.
    // Canonical rounding rule is `Decimal::round_dp(2)` (banker's /
    // round-half-to-even), same as IndianCostModel::round_inr, so 1.485
    // rounds DOWN to 1.48 (1.48 has an even final digit).
    let charge = TaxCalculator::nse_turnover_charges(dec!(50_000.00));
    assert_eq!(charge, dec!(1.48));
}

// ---------------------------------------------------------------------------
// Brokerage (did not exist before this task): 0.03% of notional, capped ₹20.
// ---------------------------------------------------------------------------

#[test]
fn test_brokerage_percentage_under_cap() {
    // 50,000 * 0.03% = 15.00, under the ₹20 cap.
    let brokerage = TaxCalculator::brokerage(dec!(50_000.00));
    assert_eq!(brokerage, dec!(15.00));
}

#[test]
fn test_brokerage_capped_at_twenty() {
    // 100,000 * 0.03% = 30.00, capped at ₹20.
    let brokerage = TaxCalculator::brokerage(dec!(100_000.00));
    assert_eq!(brokerage, dec!(20.00));
}

// ---------------------------------------------------------------------------
// STT side-gating regression (the real bug): only SELL is taxed for
// Intraday/Futures/Options; Delivery is taxed on BOTH sides.
// ---------------------------------------------------------------------------

#[test]
fn test_stt_intraday_buy_is_zero() {
    let stt = TaxCalculator::stt_charge(TradeSide::Buy, ProductType::Intraday, dec!(100_000.00));
    assert_eq!(stt, dec!(0.00));
}

#[test]
fn test_stt_intraday_sell_is_charged() {
    // 100,000 * 0.025% = 25.00
    let stt = TaxCalculator::stt_charge(TradeSide::Sell, ProductType::Intraday, dec!(100_000.00));
    assert_eq!(stt, dec!(25.00));
}

#[test]
fn test_stt_delivery_charged_on_both_sides() {
    let buy = TaxCalculator::stt_charge(TradeSide::Buy, ProductType::Delivery, dec!(100_000.00));
    let sell = TaxCalculator::stt_charge(TradeSide::Sell, ProductType::Delivery, dec!(100_000.00));
    assert_eq!(buy, dec!(100.00));
    assert_eq!(sell, dec!(100.00));
}

#[test]
fn test_stt_futures_sell_only() {
    let buy = TaxCalculator::stt_charge(TradeSide::Buy, ProductType::Futures, dec!(100_000.00));
    let sell = TaxCalculator::stt_charge(TradeSide::Sell, ProductType::Futures, dec!(100_000.00));
    assert_eq!(buy, dec!(0.00));
    // 100,000 * 0.0125% = 12.50
    assert_eq!(sell, dec!(12.50));
}

#[test]
fn test_stt_options_sell_only() {
    let buy = TaxCalculator::stt_charge(TradeSide::Buy, ProductType::Options, dec!(100_000.00));
    let sell = TaxCalculator::stt_charge(TradeSide::Sell, ProductType::Options, dec!(100_000.00));
    assert_eq!(buy, dec!(0.00));
    // 100,000 * 0.0625% = 62.50
    assert_eq!(sell, dec!(62.50));
}

// ---------------------------------------------------------------------------
// Stamp duty by product type (buy-side only; previously only Delivery existed)
// ---------------------------------------------------------------------------

#[test]
fn test_stamp_duty_by_product_type_buy_side() {
    assert_eq!(
        TaxCalculator::stamp_duty_charge(TradeSide::Buy, ProductType::Delivery, dec!(100_000.00)),
        dec!(15.00) // 0.015%
    );
    assert_eq!(
        TaxCalculator::stamp_duty_charge(TradeSide::Buy, ProductType::Intraday, dec!(100_000.00)),
        dec!(3.00) // 0.003%
    );
    assert_eq!(
        TaxCalculator::stamp_duty_charge(TradeSide::Buy, ProductType::Futures, dec!(100_000.00)),
        dec!(2.00) // 0.002%
    );
    assert_eq!(
        TaxCalculator::stamp_duty_charge(TradeSide::Buy, ProductType::Options, dec!(100_000.00)),
        dec!(3.00) // 0.003%
    );
}

#[test]
fn test_stamp_duty_is_zero_on_sell_for_all_products() {
    for product in [
        ProductType::Delivery,
        ProductType::Intraday,
        ProductType::Futures,
        ProductType::Options,
    ] {
        let sd = TaxCalculator::stamp_duty_charge(TradeSide::Sell, product, dec!(100_000.00));
        assert_eq!(
            sd,
            dec!(0.00),
            "product {product:?} should have zero stamp duty on sell"
        );
    }
}

// ---------------------------------------------------------------------------
// DP charges: only on Delivery sell, first sale of the day for the scrip.
// ---------------------------------------------------------------------------

#[test]
fn test_dp_charge_on_first_delivery_sell_of_day() {
    let dp = TaxCalculator::dp_charge(TradeSide::Sell, ProductType::Delivery, true);
    assert_eq!(dp, dec!(13.50));
}

#[test]
fn test_dp_charge_zero_on_subsequent_sell_of_day() {
    let dp = TaxCalculator::dp_charge(TradeSide::Sell, ProductType::Delivery, false);
    assert_eq!(dp, dec!(0.00));
}

#[test]
fn test_dp_charge_zero_on_buy() {
    let dp = TaxCalculator::dp_charge(TradeSide::Buy, ProductType::Delivery, true);
    assert_eq!(dp, dec!(0.00));
}

#[test]
fn test_dp_charge_zero_on_intraday_sell() {
    let dp = TaxCalculator::dp_charge(TradeSide::Sell, ProductType::Intraday, true);
    assert_eq!(dp, dec!(0.00));
}

// ---------------------------------------------------------------------------
// GST taxable-base composition: the explicit judgment call.
// IndianCostModel's `gst_taxable = brokerage + exchange_turnover + sebi_turnover + dp_charge`.
// We adopt the SAME composition (including dp_charge) for the new canonical
// `gst_charge`, unlike the legacy `GST` which omits dp_charge.
// ---------------------------------------------------------------------------

#[test]
fn test_gst_taxable_base_includes_dp_charge() {
    // base = 20.00 (brokerage) + 2.97 (exchange) + 0.10 (sebi) + 13.50 (dp) = 36.57
    // gst = 36.57 * 18% = 6.5826 -> round_dp(2) = 6.58
    let gst = TaxCalculator::gst_charge(dec!(20.00), dec!(2.97), dec!(0.10), dec!(13.50));
    assert_eq!(gst, dec!(6.58));
}

#[test]
fn test_gst_zero_dp_charge_case() {
    // base = 20.00 + 2.97 + 0.10 + 0.00 = 23.07 -> *18% = 4.1526 -> 4.15
    let gst = TaxCalculator::gst_charge(dec!(20.00), dec!(2.97), dec!(0.10), dec!(0.00));
    assert_eq!(gst, dec!(4.15));
}

// ---------------------------------------------------------------------------
// TaxBreakdown DTO: total() and add_assign()
// ---------------------------------------------------------------------------

#[test]
fn test_tax_breakdown_total_sums_all_fields() {
    let breakdown = TaxBreakdown {
        brokerage: dec!(20.00),
        stt: dec!(100.00),
        stamp_duty: dec!(15.00),
        exchange_turnover: dec!(2.97),
        sebi_turnover: dec!(0.10),
        gst: dec!(4.15),
        dp_charge: dec!(0.00),
    };
    assert_eq!(breakdown.total(), dec!(142.22));
}

#[test]
fn test_tax_breakdown_add_assign_accumulates() {
    let mut total = TaxBreakdown::default();
    let a = TaxBreakdown {
        brokerage: dec!(20.00),
        stt: dec!(100.00),
        stamp_duty: dec!(15.00),
        exchange_turnover: dec!(2.97),
        sebi_turnover: dec!(0.10),
        gst: dec!(4.15),
        dp_charge: dec!(0.00),
    };
    let b = TaxBreakdown {
        brokerage: dec!(20.00),
        stt: dec!(100.00),
        stamp_duty: dec!(0.00),
        exchange_turnover: dec!(2.97),
        sebi_turnover: dec!(0.10),
        gst: dec!(6.58),
        dp_charge: dec!(13.50),
    };
    total.add_assign(&a);
    total.add_assign(&b);
    assert_eq!(total.total(), a.total() + b.total());
    assert_eq!(total.brokerage, dec!(40.00));
    assert_eq!(total.dp_charge, dec!(13.50));
}

// ---------------------------------------------------------------------------
// `compute_charges` single entry point: numeric parity against
// IndianCostModel::compute_fees for delivery buy / delivery sell / intraday
// buy / intraday sell, using notional = price(1000.00) * quantity(100) = 100,000.
// Expected values hand-derived from costs.rs's documented formulas.
// ---------------------------------------------------------------------------

#[test]
fn test_compute_charges_parity_delivery_buy() {
    let price = dec!(1000.00);
    let qty = dec!(100);
    let b = TaxCalculator::compute_charges(
        TradeSide::Buy,
        ProductType::Delivery,
        Exchange::Nse,
        price,
        qty,
        false,
    );
    assert_eq!(b.brokerage, dec!(20.00));
    assert_eq!(b.stt, dec!(100.00));
    assert_eq!(b.stamp_duty, dec!(15.00));
    assert_eq!(b.exchange_turnover, dec!(2.97));
    assert_eq!(b.sebi_turnover, dec!(0.10));
    assert_eq!(b.dp_charge, dec!(0.00));
    assert_eq!(b.gst, dec!(4.15));
    assert_eq!(b.total(), dec!(142.22));
}

#[test]
fn test_compute_charges_parity_delivery_sell_first_of_day() {
    let price = dec!(1000.00);
    let qty = dec!(100);
    let b = TaxCalculator::compute_charges(
        TradeSide::Sell,
        ProductType::Delivery,
        Exchange::Nse,
        price,
        qty,
        true,
    );
    assert_eq!(b.brokerage, dec!(20.00));
    assert_eq!(b.stt, dec!(100.00));
    assert_eq!(b.stamp_duty, dec!(0.00));
    assert_eq!(b.exchange_turnover, dec!(2.97));
    assert_eq!(b.sebi_turnover, dec!(0.10));
    assert_eq!(b.dp_charge, dec!(13.50));
    assert_eq!(b.gst, dec!(6.58));
    assert_eq!(b.total(), dec!(143.15));
}

#[test]
fn test_compute_charges_parity_intraday_buy() {
    let price = dec!(1000.00);
    let qty = dec!(100);
    let b = TaxCalculator::compute_charges(
        TradeSide::Buy,
        ProductType::Intraday,
        Exchange::Nse,
        price,
        qty,
        false,
    );
    assert_eq!(b.stt, dec!(0.00));
    assert_eq!(b.stamp_duty, dec!(3.00));
    assert_eq!(b.dp_charge, dec!(0.00));
    assert_eq!(b.gst, dec!(4.15));
    assert_eq!(b.total(), dec!(30.22));
}

#[test]
fn test_compute_charges_parity_intraday_sell() {
    let price = dec!(1000.00);
    let qty = dec!(100);
    let b = TaxCalculator::compute_charges(
        TradeSide::Sell,
        ProductType::Intraday,
        Exchange::Nse,
        price,
        qty,
        true, // first sell of day, but Intraday never gets dp_charge
    );
    assert_eq!(b.stt, dec!(25.00));
    assert_eq!(b.stamp_duty, dec!(0.00));
    assert_eq!(b.dp_charge, dec!(0.00));
    assert_eq!(b.gst, dec!(4.15));
    assert_eq!(b.total(), dec!(52.22));
}

#[test]
fn test_compute_charges_zero_for_non_positive_inputs() {
    let b = TaxCalculator::compute_charges(
        TradeSide::Buy,
        ProductType::Delivery,
        Exchange::Nse,
        Decimal::ZERO,
        dec!(100),
        false,
    );
    assert_eq!(b, TaxBreakdown::default());
}

#[test]
fn test_compute_charges_uses_bse_exchange_selection() {
    let price = dec!(1000.00);
    let qty = dec!(100);
    let b = TaxCalculator::compute_charges(
        TradeSide::Buy,
        ProductType::Delivery,
        Exchange::Bse,
        price,
        qty,
        false,
    );
    // 100,000 * 0.00375% = 3.75 (vs NSE's 2.97)
    assert_eq!(b.exchange_turnover, dec!(3.75));
}
