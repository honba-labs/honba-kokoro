//! Valuation Latency Benchmarks using Divan
//!
//! Targets (from plan.md §5.3):
//! - FIFO tax-lot fill match: < 80 ns
//! - Full portfolio MTM revaluation (100 active positions): < 5 μs
//! - Order transition validation: < 30 ns

use divan::{black_box, Bencher};
use honba_kokoro_oms::{OrderState, OrderStateMachine, OrderState::*};
use honba_portfolio::{
    apply_fill_to_account, AccountState, HeldPosition, Marks, PortfolioState,
    PositionSide, TaxSummary, compute_capital_gains,
};
use honba_core::{Currency, InstrumentId, Exchange, Money, Price, Quantity, OrderSide, chrono::NaiveDate};
use honba_core::chrono::Utc;
use rust_decimal::Decimal;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Benchmark: Order State Transition Validation
/// Target: < 30 ns per transition
#[divan::bench]
fn order_transition_validation(bencher: Bencher) {
    let from = OrderState::New;
    let to = OrderState::PartiallyFilled;
    
    bencher.bench(|| {
        let result = OrderState::transition(from, to);
        black_box(result)
    });
}

/// Benchmark: Order State Machine - Register Order
/// Target: ~100-200 ns per registration
#[divan::bench]
fn oms_register_order(bencher: Bencher) {
    bencher.bench_local(|| {
        let mut oms = OrderStateMachine::new();
        let cl_ord_id = "bench_cl_ord";
        oms.register_order(cl_ord_id.to_string(), PendingNew, Utc::now());
        black_box(&oms);
    });
}

/// Benchmark: Order State Machine - Process Fill
/// Target: ~100-200 ns per fill
#[divan::bench]
fn oms_process_fill(bencher: Bencher) {
    bencher.bench_local(|| {
        let mut oms = OrderStateMachine::new();
        oms.register_order("bench_cl_ord".to_string(), PendingNew, Utc::now());
        let (state, filled) = oms.process_fill("bench_cl_ord", 10);
        black_box((state, filled));
    });
}

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
        PositionSide::Long,
        Quantity::new(Decimal::from(100)),
        Price::from_rupees("2000").unwrap(),
    );
    portfolio.accounts.get_mut("bench").unwrap().positions.push(pos);
    
    bencher.bench_local(|| {
        let pnl = apply_fill_to_account(
            portfolio.accounts.get_mut("bench").unwrap(),
            id.clone(),
            PositionSide::Long,
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
        PositionSide::Long,
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
    
    // Add 10 positions
    let mut account = AccountState::new(Currency::Inr, vec![]);
    for i in 0..10 {
        let id = InstrumentId::new(Exchange::Nse, &format!("STOCK{}", i));
        let pos = HeldPosition::new(
            id.clone(),
            PositionSide::Long,
            Quantity::new(Decimal::from(100)),
            Price::from_rupees("2000").unwrap(),
        );
        account.positions.push(pos);
        marks.marks.insert(id, Price::from_rupees("2015").unwrap());
    }
    portfolio.accounts.insert("bench".to_string(), account);
    
    bencher.bench_local(|| {
        // Simulate MTM by computing market value for each position
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
    
    // Add 100 positions
    let mut account = AccountState::new(Currency::Inr, vec![]);
    for i in 0..100 {
        let id = InstrumentId::new(Exchange::Nse, &format!("STOCK{}", i));
        let pos = HeldPosition::new(
            id.clone(),
            PositionSide::Long,
            Quantity::new(Decimal::from(100)),
            Price::from_rupees("2000").unwrap(),
        );
        account.positions.push(pos);
        marks.marks.insert(id, Price::from_rupees("2015").unwrap());
    }
    portfolio.accounts.insert("bench".to_string(), account);
    
    bencher.bench_local(|| {
        // Simulate MTM by computing market value for each position
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
        PositionSide::Long,
        Quantity::new(Decimal::from(100)),
        Price::from_rupees("1500").unwrap(),
    );
    account.positions.push(pos);
    portfolio.accounts.insert("bench".to_string(), account);
    
    let fills = [(
        &portfolio.accounts["bench"],
        id.clone(),
        PositionSide::Long,
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

/// Benchmark: Order State Machine - Multiple Operations
/// Simulates a realistic order lifecycle
#[divan::bench]
fn oms_full_lifecycle(bencher: Bencher) {
    
    bencher.bench_local(|| {
        let mut oms = OrderStateMachine::new();
        
        // Register
        oms.register_order("cl_001".to_string(), PendingNew, Utc::now());
        
        // Transition to New
        let _ = OrderState::transition(PendingNew, New);
        
        // Process fills
        let _ = oms.process_fill("cl_001", 50);
        let _ = oms.process_fill("cl_001", 50);
        
        // Final state check
        let state = oms.get_state("cl_001");
        black_box(state);
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
        PositionSide::Long,
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
}}
