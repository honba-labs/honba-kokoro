//! Portfolio Valuation Benchmarks using Divan
//!
//! Targets (from plan.md §5.3):
//! - FIFO tax-lot fill match: < 80 ns
//! - Full portfolio MTM revaluation (100 active positions): < 5 μs

use divan::{black_box, Bencher};
use honba_portfolio::{
    apply_fill_to_account, AccountState, HeldPosition, Marks, PortfolioState,
    PositionSide as PortfolioPositionSide, TaxSummary, compute_capital_gains,
};
use honba_core::{Currency, InstrumentId, Exchange, Money, Price, Quantity, chrono::NaiveDate};
use rust_decimal::Decimal;
use std::collections::BTreeMap;

/// Benchmark: FIFO Tax-Lot Fill Match (Single Fill)
/// Target: < 80 ns per fill match
#[divan::bench]
fn fifo_taxlot_fill_match(bencher: Bencher) {
    let mut portfolio = PortfolioState::new(Currency::Inr);
    let account = AccountState::new(Currency::Inr, vec![]);
    portfolio.accounts.insert("bench".to_string(), account);
    
    let id = InstrumentId::new(Exchange::Nse, "RELIANCE");
    let pos = HeldPosition::new(
        id.clone(),
        PortfolioPositionSide::Long,
        Quantity::new(Decimal::from(100)),
        Price::from_rupees("2000").unwrap(),
    );
    portfolio.accounts.get_mut("bench").unwrap().positions.push(pos);
    
    bencher.bench_local(|| {
        let pnl = apply_fill_to_account(
            portfolio.accounts.get_mut("bench").unwrap(),
            id.clone(),
            PortfolioPositionSide::Long,
            10,
            Price::from_rupees("2015").unwrap(),
        );
        black_box(pnl);
    });
}

/// Benchmark: Capital Gains Computation
/// Target: ~50-100 ns per computation
#[divan::bench]
fn capital_gains_computation(bencher: Bencher) {
    let id = InstrumentId::new(Exchange::Nse, "RELIANCE");
    let pos = HeldPosition::new(
        id.clone(),
        PortfolioPositionSide::Long,
        Quantity::new(Decimal::from(100)),
        Price::from_rupees("1500").unwrap(),
    );
    
    let sale_date = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
    
    bencher.bench(|| {
        let (pnl, gain_type) = compute_capital_gains(
            &pos,
            10,
            Price::from_rupees("2000").unwrap(),
            sale_date,
        );
        black_box((pnl, gain_type));
    });
}

/// Benchmark: Portfolio MTM Valuation (10 positions)
/// Target: < 5 μs for full revaluation
#[divan::bench]
fn portfolio_mtm_valuation_10_positions(bencher: Bencher) {
    let mut portfolio = PortfolioState::new(Currency::Inr);
    let mut marks = Marks::new();
    
    let mut account = AccountState::new(Currency::Inr, vec![]);
    for i in 0..10 {
        let id = InstrumentId::new(Exchange::Nse, &format!("STOCK{}", i));
        let pos = HeldPosition::new(
            id.clone(),
            PortfolioPositionSide::Long,
            Quantity::new(Decimal::from(100)),
            Price::from_rupees("2000").unwrap(),
        );
        account.positions.push(pos);
        marks.marks.insert(id, Price::from_rupees("2015").unwrap());
    }
    portfolio.accounts.insert("bench".to_string(), account);
    
    bencher.bench_local(|| {
        let mut total_market_value = Money::inr(Decimal::ZERO);
        let mut total_unrealized = Money::inr(Decimal::ZERO);
        
        if let Some(account) = portfolio.accounts.get("bench") {
            for pos in &account.positions {
                if let Some(mark) = marks.marks.get(&pos.instrument) {
                    let market_value = mark.0 * pos.quantity.0;
                    let cost_basis = pos.entry_price.0 * pos.quantity.0;
                    let unrealized = market_value - cost_basis;
                    
                    total_market_value = Money {
                        amount: total_market_value.amount + market_value,
                        currency: Currency::Inr,
                    };
                    total_unrealized = Money {
                        amount: total_unrealized.amount + unrealized,
                        currency: Currency::Inr,
                    };
                }
            }
        }
        
        black_box((total_market_value, total_unrealized));
    });
}

/// Benchmark: Portfolio MTM Valuation (100 positions)
/// Target: < 5 μs for full revaluation
#[divan::bench]
fn portfolio_mtm_valuation_100_positions(bencher: Bencher) {
    let mut portfolio = PortfolioState::new(Currency::Inr);
    let mut marks = Marks::new();
    
    let mut account = AccountState::new(Currency::Inr, vec![]);
    for i in 0..100 {
        let id = InstrumentId::new(Exchange::Nse, &format!("STOCK{}", i));
        let pos = HeldPosition::new(
            id.clone(),
            PortfolioPositionSide::Long,
            Quantity::new(Decimal::from(100)),
            Price::from_rupees("2000").unwrap(),
        );
        account.positions.push(pos);
        marks.marks.insert(id, Price::from_rupees("2015").unwrap());
    }
    portfolio.accounts.insert("bench".to_string(), account);
    
    bencher.bench_local(|| {
        let mut total_market_value = Money::inr(Decimal::ZERO);
        let mut total_unrealized = Money::inr(Decimal::ZERO);
        
        if let Some(account) = portfolio.accounts.get("bench") {
            for pos in &account.positions {
                if let Some(mark) = marks.marks.get(&pos.instrument) {
                    let market_value = mark.0 * pos.quantity.0;
                    let cost_basis = pos.entry_price.0 * pos.quantity.0;
                    let unrealized = market_value - cost_basis;
                    
                    total_market_value = Money {
                        amount: total_market_value.amount + market_value,
                        currency: Currency::Inr,
                    };
                    total_unrealized = Money {
                        amount: total_unrealized.amount + unrealized,
                        currency: Currency::Inr,
                    };
                }
            }
        }
        
        black_box((total_market_value, total_unrealized));
    });
}

/// Benchmark: Tax Summary Generation
/// Target: ~10-50 μs for full tax summary
#[divan::bench]
fn tax_summary_generation(bencher: Bencher) {
    use honba_portfolio::TaxSummary;
    
    let mut portfolio = PortfolioState::new(Currency::Inr);
    let mut account = AccountState::new(Currency::Inr, vec![]);
    
    let id = InstrumentId::new(Exchange::Nse, "RELIANCE");
    let pos = HeldPosition::new(
        id.clone(),
        PortfolioPositionSide::Long,
        Quantity::new(Decimal::from(100)),
        Price::from_rupees("1500").unwrap(),
    );
    account.positions.push(pos);
    portfolio.accounts.insert("bench".to_string(), account);
    
    let fills = [(
        &portfolio.accounts["bench"],
        id.clone(),
        PortfolioPositionSide::Long,
        10,
        Price::from_rupees("2000").unwrap(),
        NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
    )];
    
    bencher.bench(|| {
        let summary = TaxSummary::from_fills(&fills, 
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
            &[]
        );
        black_box(summary);
    });
}

/// Benchmark: Corporate Action Application
#[divan::bench]
fn corporate_action_application(bencher: Bencher) {
    use honba_portfolio::{
        CorporateAction, adjust_for_split, adjust_for_bonus, record_dividend
    };
    
    let id = InstrumentId::new(Exchange::Nse, "RELIANCE");
    let mut pos = HeldPosition::new(
        id.clone(),
        PortfolioPositionSide::Long,
        Quantity::new(Decimal::from(100)),
        Price::from_rupees("2000").unwrap(),
    );
    
    let split = CorporateAction::Split {
        ratio_numerator: 2,
        ratio_denominator: 1,
        ex_date: NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
    };
    
    bencher.bench(|| {
        let mut test_pos = pos.clone();
        adjust_for_split(&mut test_pos, 2, 1);
        black_box(&test_pos);
    });
}
