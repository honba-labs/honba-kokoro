//! Property-based tests verifying double-entry ledger invariants.
//!
//! Enforces:
//! 1. Universal Double-Entry Identity: sum(Debits) == sum(Credits)
//! 2. Unbalanced transactions are strictly rejected.
//! 3. Fundamental Balance Sheet Identity:
//!    Cash + Holdings == Opening Capital + Realized PnL - Statutory Charges - Brokerage
//!    held over extensive randomized buy/sell trade sequences.

use chrono::Utc;
use honba_kokoro_ledger::{
    DoubleEntryLedger, Exchange, LedgerError, ProductType, TaxCalculator, TradeSide, Transaction,
};
use proptest::prelude::*;
use rust_decimal::Decimal;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
enum LedgerAction {
    Buy {
        price_rupees: u32,
        quantity: u32,
        is_delivery: bool,
        exchange: Exchange,
    },
    Sell {
        price_rupees: u32,
        quantity: u32,
        cost_basis_rupees: u32,
        is_delivery: bool,
        exchange: Exchange,
        is_first_sell: bool,
    },
}

prop_compose! {
    fn arb_action()(
        is_buy in any::<bool>(),
        price_rupees in 10u32..5000u32,
        quantity in 1u32..100u32,
        cost_basis_rupees in 10u32..5000u32,
        is_delivery in any::<bool>(),
        is_nse in any::<bool>(),
        is_first_sell in any::<bool>(),
    ) -> LedgerAction {
        let exchange = if is_nse { Exchange::Nse } else { Exchange::Bse };
        if is_buy {
            LedgerAction::Buy {
                price_rupees,
                quantity,
                is_delivery,
                exchange,
            }
        } else {
            LedgerAction::Sell {
                price_rupees,
                quantity,
                cost_basis_rupees,
                is_delivery,
                exchange,
                is_first_sell,
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn test_proptest_double_entry_balance_identity(
        leg_amount_1 in 100u64..1_000_000u64,
        leg_amount_2 in 100u64..1_000_000u64,
        leg_amount_3 in 100u64..1_000_000u64,
    ) {
        let mut ledger = DoubleEntryLedger::new();
        let amount1 = Decimal::from(leg_amount_1);
        let amount2 = Decimal::from(leg_amount_2);
        let amount3 = Decimal::from(leg_amount_3);
        let total_debit = amount1 + amount2;
        let total_credit = total_debit;

        let mut debits = BTreeMap::new();
        debits.insert("Assets:Cash:BrokerAccount".to_string(), amount1);
        debits.insert("Assets:Holdings:EquityDelivery".to_string(), amount2);

        let mut credits = BTreeMap::new();
        credits.insert("Equity:OpeningCapital".to_string(), total_credit);

        let tx = Transaction {
            id: "tx_balanced".to_string(),
            timestamp: Utc::now(),
            debits,
            credits,
            description: "Balanced transaction".to_string(),
        };

        prop_assert!(ledger.post(tx).is_ok());
        prop_assert!(ledger.verify_double_entry_balance());
        prop_assert_eq!(ledger.total_debits(), ledger.total_credits());

        // Posting a second balanced transaction
        let mut debits2 = BTreeMap::new();
        debits2.insert("Expenses:Brokerage".to_string(), amount3);
        let mut credits2 = BTreeMap::new();
        credits2.insert("Assets:Cash:BrokerAccount".to_string(), amount3);

        let tx2 = Transaction {
            id: "tx_balanced_2".to_string(),
            timestamp: Utc::now(),
            debits: debits2,
            credits: credits2,
            description: "Second balanced transaction".to_string(),
        };

        prop_assert!(ledger.post(tx2).is_ok());
        prop_assert!(ledger.verify_double_entry_balance());
        prop_assert_eq!(ledger.total_debits(), ledger.total_credits());
    }

    #[test]
    fn test_proptest_unbalanced_transactions_rejected(
        debit_amt in 100u64..500_000u64,
        diff in 1u64..10_000u64,
    ) {
        let mut ledger = DoubleEntryLedger::new();
        let debit_dec = Decimal::from(debit_amt);
        let credit_dec = Decimal::from(debit_amt + diff);

        let mut debits = BTreeMap::new();
        debits.insert("CashAtBroker".to_string(), debit_dec);

        let mut credits = BTreeMap::new();
        credits.insert("OpeningCapital".to_string(), credit_dec);

        let tx = Transaction {
            id: "tx_unbalanced".to_string(),
            timestamp: Utc::now(),
            debits,
            credits,
            description: "Unbalanced test transaction".to_string(),
        };

        let result = ledger.post(tx);
        prop_assert!(matches!(result, Err(LedgerError::InvalidAmount)));
    }

    #[test]
    fn test_proptest_accounting_identity_invariants(
        opening_capital_lakhs in 10u64..1000u64,
        actions in prop::collection::vec(arb_action(), 1..30),
    ) {
        let mut ledger = DoubleEntryLedger::new();
        let opening_capital = Decimal::from(opening_capital_lakhs * 100_000);

        prop_assert!(ledger
            .record_opening_capital("tx_opening", Utc::now(), opening_capital)
            .is_ok());

        // Verify initial identity
        prop_assert!(ledger.verify_double_entry_balance());
        prop_assert!(ledger.verify_accounting_identity().is_ok());

        let mut tx_counter = 0;
        for action in actions {
            tx_counter += 1;
            let tx_id = format!("tx_{}", tx_counter);

            match action {
                LedgerAction::Buy {
                    price_rupees,
                    quantity,
                    is_delivery,
                    exchange,
                } => {
                    let price = Decimal::from(price_rupees);
                    let qty = Decimal::from(quantity);
                    let notional = price * qty;
                    let product = if is_delivery {
                        ProductType::Delivery
                    } else {
                        ProductType::Intraday
                    };
                    let charges = TaxCalculator::compute_charges(
                        TradeSide::Buy,
                        product,
                        exchange,
                        price,
                        qty,
                        false,
                    );

                    let res = ledger.record_buy(&tx_id, Utc::now(), notional, &charges);
                    prop_assert!(res.is_ok());
                }
                LedgerAction::Sell {
                    price_rupees,
                    quantity,
                    cost_basis_rupees,
                    is_delivery,
                    exchange,
                    is_first_sell,
                } => {
                    let price = Decimal::from(price_rupees);
                    let qty = Decimal::from(quantity);
                    let proceeds = price * qty;
                    let cost_basis = Decimal::from(cost_basis_rupees) * qty;
                    let product = if is_delivery {
                        ProductType::Delivery
                    } else {
                        ProductType::Intraday
                    };
                    let charges = TaxCalculator::compute_charges(
                        TradeSide::Sell,
                        product,
                        exchange,
                        price,
                        qty,
                        is_first_sell,
                    );

                    let res = ledger.record_sell(
                        &tx_id,
                        Utc::now(),
                        proceeds,
                        cost_basis,
                        &charges,
                    );
                    prop_assert!(res.is_ok());
                }
            }

            // Universal Double-entry invariant: sum(Debits) == sum(Credits)
            prop_assert!(
                ledger.verify_double_entry_balance(),
                "Debits ({}) != Credits ({}) after {}",
                ledger.total_debits(),
                ledger.total_credits(),
                tx_id
            );

            // Accounting identity invariant:
            // Cash + Holdings == Opening Capital + Realized PnL - Statutory Charges - Brokerage
            let identity_res = ledger.verify_accounting_identity();
            prop_assert!(
                identity_res.is_ok(),
                "Accounting identity failed at {}: left_hand_side != right_hand_side: {:?}",
                tx_id,
                identity_res.err()
            );
        }
    }
}

#[test]
fn test_extensive_randomized_accounting_invariants_10k_ops() {
    let mut ledger = DoubleEntryLedger::new();
    let opening_capital = Decimal::from(100_000_000); // 10 Crore initial capital
    ledger
        .record_opening_capital("tx_opening", Utc::now(), opening_capital)
        .unwrap();

    let mut seed: u64 = 0xdeadbeefcafebabe;
    let mut pseudo_rand = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        seed
    };

    for i in 0..10_000 {
        let is_buy = (pseudo_rand() % 2) == 0;
        let price = Decimal::from(50 + (pseudo_rand() % 3000));
        let qty = Decimal::from(1 + (pseudo_rand() % 500));
        let is_nse = (pseudo_rand() % 2) == 0;
        let exchange = if is_nse { Exchange::Nse } else { Exchange::Bse };
        let is_delivery = (pseudo_rand() % 2) == 0;
        let product = if is_delivery {
            ProductType::Delivery
        } else {
            ProductType::Intraday
        };

        let tx_id = format!("stress_tx_{}", i);
        if is_buy {
            let notional = price * qty;
            let charges = TaxCalculator::compute_charges(
                TradeSide::Buy,
                product,
                exchange,
                price,
                qty,
                false,
            );
            ledger
                .record_buy(&tx_id, Utc::now(), notional, &charges)
                .unwrap();
        } else {
            let proceeds = price * qty;
            let cost_basis = Decimal::from(50 + (pseudo_rand() % 3000)) * qty;
            let is_first = (pseudo_rand() % 2) == 0;
            let charges = TaxCalculator::compute_charges(
                TradeSide::Sell,
                product,
                exchange,
                price,
                qty,
                is_first,
            );
            ledger
                .record_sell(&tx_id, Utc::now(), proceeds, cost_basis, &charges)
                .unwrap();
        }

        assert!(ledger.verify_double_entry_balance());
        assert!(ledger.verify_accounting_identity().is_ok());
    }

    assert_eq!(ledger.transactions.len(), 10_001);
    assert!(ledger.total_debits() == ledger.total_credits());
    assert!(ledger.verify_accounting_identity().is_ok());
}

#[test]
fn test_mutation_rigor_legacy_dp_charges() {
    // 13.50 + floor(13.50 * 0.18) = 13.50 + 2 = 15.50
    let dp = TaxCalculator::dp_charges(true);
    assert_eq!(dp, Decimal::new(155, 1));
}

#[test]
fn test_mutation_rigor_ledger_balance_failures() {
    let mut ledger = DoubleEntryLedger::new();
    ledger
        .debit_balances
        .insert("Assets:Cash:BrokerAccount".to_string(), Decimal::from(100));
    ledger
        .credit_balances
        .insert("Equity:OpeningCapital".to_string(), Decimal::from(50));

    // Debits (100) != Credits (50) -> must return false
    assert!(!ledger.verify_double_entry_balance());

    // Accounting identity lhs (100 + 0) != rhs (50 + 0 - 0 - 0) -> must return Err
    assert!(ledger.verify_accounting_identity().is_err());
}

#[test]
fn test_mutation_rigor_account_name_aggregation() {
    let mut ledger = DoubleEntryLedger::new();
    // Cash
    ledger
        .debit_balances
        .insert("Assets:Cash:BrokerAccount".to_string(), Decimal::from(100));
    ledger
        .debit_balances
        .insert("CashAtBroker".to_string(), Decimal::from(50));
    assert_eq!(ledger.cash_balance(), Decimal::from(150));

    // Holdings
    ledger.debit_balances.insert(
        "Assets:Holdings:EquityDelivery".to_string(),
        Decimal::from(200),
    );
    ledger
        .debit_balances
        .insert("HoldingsEquityDelivery".to_string(), Decimal::from(80));
    assert_eq!(ledger.holdings_balance(), Decimal::from(280));

    // Opening Capital
    ledger
        .credit_balances
        .insert("Equity:OpeningCapital".to_string(), Decimal::from(300));
    ledger
        .credit_balances
        .insert("OpeningCapital".to_string(), Decimal::from(70));
    assert_eq!(ledger.opening_capital(), Decimal::from(370));

    // Realized PnL
    ledger
        .credit_balances
        .insert("Income:RealizedPnL".to_string(), Decimal::from(400));
    ledger
        .credit_balances
        .insert("IncomeRealizedPnL".to_string(), Decimal::from(60));
    assert_eq!(ledger.realized_pnl(), Decimal::from(460));

    // Brokerage
    ledger
        .debit_balances
        .insert("Expenses:Brokerage".to_string(), Decimal::from(20));
    ledger
        .debit_balances
        .insert("ExpensesBrokerage".to_string(), Decimal::from(10));
    assert_eq!(ledger.brokerage(), Decimal::from(30));

    // Statutory Charges (all 6 pairs)
    ledger
        .debit_balances
        .insert("Expenses:Taxes:STT".to_string(), Decimal::from(1));
    ledger
        .debit_balances
        .insert("ExpensesTaxesSTT".to_string(), Decimal::from(2));
    ledger
        .debit_balances
        .insert("Expenses:Taxes:StampDuty".to_string(), Decimal::from(3));
    ledger
        .debit_balances
        .insert("ExpensesTaxesStampDuty".to_string(), Decimal::from(4));
    ledger
        .debit_balances
        .insert("Expenses:ExchangeFees".to_string(), Decimal::from(5));
    ledger
        .debit_balances
        .insert("ExpensesExchangeFees".to_string(), Decimal::from(6));
    ledger
        .debit_balances
        .insert("Expenses:SEBIFees".to_string(), Decimal::from(7));
    ledger
        .debit_balances
        .insert("ExpensesSEBIFees".to_string(), Decimal::from(8));
    ledger
        .debit_balances
        .insert("Expenses:Taxes:GST".to_string(), Decimal::from(9));
    ledger
        .debit_balances
        .insert("ExpensesTaxesGST".to_string(), Decimal::from(10));
    ledger
        .debit_balances
        .insert("Expenses:DPCharges".to_string(), Decimal::from(11));
    ledger
        .debit_balances
        .insert("ExpensesDPCharges".to_string(), Decimal::from(12));
    assert_eq!(ledger.statutory_charges(), Decimal::from(78));
}

#[test]
fn test_mutation_rigor_zero_charges_handling() {
    let mut ledger = DoubleEntryLedger::new();
    let zero_charges = honba_kokoro_ledger::TaxBreakdown::default();

    // Buy with 0 charges
    let buy_res = ledger.record_buy("tx_b0", Utc::now(), Decimal::from(1000), &zero_charges);
    assert!(buy_res.is_ok());

    // Sell with 0 charges at profit
    let sell_gain_res = ledger.record_sell(
        "tx_s0",
        Utc::now(),
        Decimal::from(1200),
        Decimal::from(1000),
        &zero_charges,
    );
    assert!(sell_gain_res.is_ok());

    // Sell with 0 charges at loss
    let sell_loss_res = ledger.record_sell(
        "tx_s1",
        Utc::now(),
        Decimal::from(800),
        Decimal::from(1000),
        &zero_charges,
    );
    assert!(sell_loss_res.is_ok());

    // Sell with 0 charges at break-even (pnl == 0)
    let sell_breakeven_res = ledger.record_sell(
        "tx_s2",
        Utc::now(),
        Decimal::from(1000),
        Decimal::from(1000),
        &zero_charges,
    );
    assert!(sell_breakeven_res.is_ok());
}
