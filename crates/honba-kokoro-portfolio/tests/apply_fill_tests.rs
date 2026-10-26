//! External tests (AGENTS.md parallel `tests/` convention) for the two real
//! bugs in `apply_fill` (src/lib.rs, around lines 836-876):
//!
//! 1. The `PositionSide::Short` arm was a stub that always returned
//!    `realized_pnl = 0` regardless of the fill.
//! 2. The Long "full fill + reverse" branch realized PnL on the overfilled
//!    quantity (as if it were already closed) but never actually opened the
//!    new reversed short leg — the position was left at quantity 0 instead
//!    of flipping side with the correct remaining quantity and entry price.
//!
//! These tests exercise the fix: short PnL is now computed symmetrically to
//! long, and a reversal correctly realizes PnL only on the portion that
//! closed while opening a new leg on the opposite side for the remainder.

use honba_core::{Exchange, InstrumentId};
use honba_portfolio::{HeldPosition, PositionSide, Price, Quantity, apply_fill};
use rust_decimal::Decimal;

fn reliance() -> InstrumentId {
    InstrumentId::new(Exchange::Nse, "RELIANCE")
}

#[test]
fn test_short_position_realized_pnl_is_computed() {
    // Short 10 @ 2000. Buy back (fill) 10 @ 1950: price dropped, so the
    // short should realize a PROFIT of 10 * (2000 - 1950) = 500.
    let mut pos = HeldPosition::new(
        reliance(),
        PositionSide::Short,
        Quantity::new(Decimal::from(10)),
        Price::from_rupees("2000").unwrap(),
    );

    let pnl = apply_fill(
        &mut pos,
        PositionSide::Short,
        10,
        Price::from_rupees("1950").unwrap(),
    );

    assert_eq!(pnl.amount, Decimal::from(500));
    assert_eq!(pos.quantity.0, Decimal::ZERO);
}

#[test]
fn test_short_position_partial_close_realized_pnl() {
    // Short 10 @ 2000. Partial buy-back of 4 @ 1950: profit on 4 units.
    let mut pos = HeldPosition::new(
        reliance(),
        PositionSide::Short,
        Quantity::new(Decimal::from(10)),
        Price::from_rupees("2000").unwrap(),
    );

    let pnl = apply_fill(
        &mut pos,
        PositionSide::Short,
        4,
        Price::from_rupees("1950").unwrap(),
    );

    // 4 * (2000 - 1950) = 200
    assert_eq!(pnl.amount, Decimal::from(200));
    // Remaining short quantity: 6
    assert_eq!(pos.quantity.0, Decimal::from(6));
    assert_eq!(pos.side, PositionSide::Short);
}

#[test]
fn test_short_position_realized_pnl_is_negative_on_loss() {
    // Short 10 @ 2000. Buy back 10 @ 2050 (price rose): this is a LOSS.
    let mut pos = HeldPosition::new(
        reliance(),
        PositionSide::Short,
        Quantity::new(Decimal::from(10)),
        Price::from_rupees("2000").unwrap(),
    );

    let pnl = apply_fill(
        &mut pos,
        PositionSide::Short,
        10,
        Price::from_rupees("2050").unwrap(),
    );

    // 10 * (2000 - 2050) = -500: a real loss must show as negative, not be
    // clamped to zero.
    assert_eq!(pnl.amount, Decimal::from(-500));
}

/// Regression guard for the existing, already-correct Long partial-close
/// behavior: must remain unchanged by the bug fix.
#[test]
fn test_long_position_partial_close_pnl() {
    let mut pos = HeldPosition::new(
        reliance(),
        PositionSide::Long,
        Quantity::new(Decimal::from(10)),
        Price::from_rupees("2000").unwrap(),
    );

    let pnl = apply_fill(
        &mut pos,
        PositionSide::Long,
        4,
        Price::from_rupees("2015").unwrap(),
    );

    // 4 * (2015 - 2000) = 60
    assert_eq!(pnl.amount, Decimal::from(60));
    assert_eq!(pos.quantity.0, Decimal::from(6));
    assert_eq!(pos.side, PositionSide::Long);
}

#[test]
fn test_long_to_short_reversal_opens_new_short_leg() {
    // Long 10 @ 2000. Fill a SELL of 15 @ 2100: closes the 10 long units
    // (realizing PnL on 10 units only) and opens a new SHORT leg of 5 units
    // at the fill price (2100) -- the overfilled remainder.
    let mut pos = HeldPosition::new(
        reliance(),
        PositionSide::Long,
        Quantity::new(Decimal::from(10)),
        Price::from_rupees("2000").unwrap(),
    );

    let pnl = apply_fill(
        &mut pos,
        PositionSide::Long,
        15,
        Price::from_rupees("2100").unwrap(),
    );

    // Only the closed 10 units are realized: 10 * (2100 - 2000) = 1000.
    // The other 5 units are NOT realized PnL -- they are the basis of the
    // new short leg.
    assert_eq!(pnl.amount, Decimal::from(1000));

    // The position must have flipped to Short with the remaining 5 units,
    // entered at the fill price.
    assert_eq!(pos.side, PositionSide::Short);
    assert_eq!(pos.quantity.0, Decimal::from(5));
    assert_eq!(pos.entry_price.0, Decimal::from(2100));
}

#[test]
fn test_short_to_long_reversal_symmetric_case() {
    // Short 10 @ 2000. Fill a BUY of 15 @ 1900: closes the 10 short units
    // (realizing profit on 10 units, since price fell) and opens a new LONG
    // leg of 5 units at the fill price (1900).
    let mut pos = HeldPosition::new(
        reliance(),
        PositionSide::Short,
        Quantity::new(Decimal::from(10)),
        Price::from_rupees("2000").unwrap(),
    );

    let pnl = apply_fill(
        &mut pos,
        PositionSide::Short,
        15,
        Price::from_rupees("1900").unwrap(),
    );

    // Closed 10 units: 10 * (2000 - 1900) = 1000.
    assert_eq!(pnl.amount, Decimal::from(1000));

    assert_eq!(pos.side, PositionSide::Long);
    assert_eq!(pos.quantity.0, Decimal::from(5));
    assert_eq!(pos.entry_price.0, Decimal::from(1900));
}

#[test]
fn test_long_full_close_exact_quantity_still_zeroes_out() {
    // Regression guard: an exact full close (fill_qty == pos.quantity) must
    // still zero out the position rather than accidentally hitting the
    // reversal branch.
    let mut pos = HeldPosition::new(
        reliance(),
        PositionSide::Long,
        Quantity::new(Decimal::from(10)),
        Price::from_rupees("2000").unwrap(),
    );

    let pnl = apply_fill(
        &mut pos,
        PositionSide::Long,
        10,
        Price::from_rupees("2050").unwrap(),
    );

    assert_eq!(pnl.amount, Decimal::from(500));
    assert_eq!(pos.quantity.0, Decimal::ZERO);
    assert_eq!(pos.side, PositionSide::Long);
}
