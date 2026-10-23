//! External tests (AGENTS.md parallel `tests/` convention) for injecting an
//! external holiday set into `ClearingCalendar` instead of duplicating a
//! specific exchange's holiday table inside this crate.
//!
//! `ClearingCalendar::new()` keeps its existing hardcoded NSE 2024-2026
//! table unchanged (the existing inline tests in src/lib.rs depend on it).
//! `ClearingCalendar::with_holidays(&[NaiveDate])` is the new constructor:
//! it builds a calendar purely from an injected holiday list, with no
//! hardcoded table at all. In sub-step 4 (honba-ya), the real NSE holiday
//! *data* owned by `NseTradingCalendar` gets passed in here; this crate-side
//! test only proves the injection mechanism with a synthetic holiday list.

use chrono::NaiveDate;
use honba_portfolio::ClearingCalendar;

/// A synthetic holiday that is NOT one of NSE's real declared holidays, to
/// prove `with_holidays` is driven purely by the injected list and not by
/// any hardcoded table.
fn synthetic_holiday() -> NaiveDate {
    // A Wednesday, otherwise an ordinary trading day.
    NaiveDate::from_ymd_opt(2027, 3, 10).unwrap()
}

#[test]
fn test_injected_holiday_is_not_a_trading_day() {
    let cal = ClearingCalendar::with_holidays(&[synthetic_holiday()]);
    assert!(!cal.is_trading_day(synthetic_holiday()));
}

#[test]
fn test_injected_holiday_is_not_a_settlement_day() {
    let cal = ClearingCalendar::with_holidays(&[synthetic_holiday()]);
    assert!(!cal.is_settlement_day(synthetic_holiday()));
}

#[test]
fn test_weekday_not_in_injected_list_is_still_a_trading_day() {
    let cal = ClearingCalendar::with_holidays(&[synthetic_holiday()]);
    // Thursday, Mar 11 2027: an ordinary weekday, not in the injected list.
    let ordinary_day = NaiveDate::from_ymd_opt(2027, 3, 11).unwrap();
    assert!(cal.is_trading_day(ordinary_day));
}

#[test]
fn test_weekend_excluded_even_with_empty_injected_holiday_list() {
    let cal = ClearingCalendar::with_holidays(&[]);
    // Saturday, Mar 13 2027.
    let saturday = NaiveDate::from_ymd_opt(2027, 3, 13).unwrap();
    assert!(!cal.is_trading_day(saturday));
}

#[test]
fn test_settlement_date_skips_over_injected_holiday() {
    let holiday = synthetic_holiday(); // Wednesday, Mar 10 2027
    let cal = ClearingCalendar::with_holidays(&[holiday]);

    // Trade on Tuesday Mar 9 2027: T+1 would normally be Wednesday Mar 10,
    // but that's the injected holiday, so settlement rolls to Thursday Mar 11.
    let tuesday = NaiveDate::from_ymd_opt(2027, 3, 9).unwrap();
    let settle = cal.settlement_date(tuesday);
    assert_eq!(settle, NaiveDate::from_ymd_opt(2027, 3, 11).unwrap());
}

#[test]
fn test_date_not_in_an_empty_injected_list_is_a_real_nse_holiday_elsewhere() {
    // Sanity check that `with_holidays` truly has NO hardcoded table: a real
    // NSE holiday from ClearingCalendar::new()'s own hardcoded 2026 table
    // (Republic Day, Jan 26 2026) must NOT be a holiday here unless injected.
    let cal = ClearingCalendar::with_holidays(&[]);
    let republic_day_2026 = NaiveDate::from_ymd_opt(2026, 1, 26).unwrap(); // Monday
    assert!(cal.is_trading_day(republic_day_2026));
}
