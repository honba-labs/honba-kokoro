//! OMS Benchmarks using Divan
//!
//! Targets:
//! - Order transition validation: < 30 ns

use chrono::Utc;
use divan::{Bencher, black_box};
use honba_kokoro_oms::{OrderState, OrderState::*, OrderStateMachine};

/// Benchmark: Order State Transition Validation
/// Target: < 30 ns per transition
#[divan::bench]
fn order_transition_validation(bencher: Bencher) {
    let from = OrderState::New;
    let to = OrderState::PartiallyFilled;

    bencher.bench_local(|| {
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

/// Benchmark: Idempotency Check
/// Target: < 50 ns per check
#[divan::bench]
fn idempotency_check(bencher: Bencher) {
    bencher.bench_local(|| {
        let mut oms = OrderStateMachine::new();
        let cl_ord_id = "bench_idempotent";

        // First registration
        oms.register_order(cl_ord_id.to_string(), PendingNew, Utc::now());

        // Check idempotency (should fail)
        let result = black_box(oms.check_idempotency(cl_ord_id));
        let _ = black_box(result);
    });
}

/// Benchmark: State Retrieval
/// Target: < 20 ns per lookup
#[divan::bench]
fn state_retrieval(bencher: Bencher) {
    let mut oms = OrderStateMachine::new();
    oms.register_order("cl_001".to_string(), PendingNew, Utc::now());

    bencher.bench_local(|| {
        let state = oms.get_state("cl_001");
        black_box(state);
    });
}

/// Benchmark: Cumulative Fill Tracking
#[divan::bench]
fn cumulative_fill_tracking(bencher: Bencher) {
    let mut oms = OrderStateMachine::new();
    oms.register_order("cl_001".to_string(), PendingNew, Utc::now());

    bencher.bench_local(|| {
        let _ = oms.process_fill("cl_001", 10);
        let cum = oms.get_cumulative_fill("cl_001");
        black_box(cum);
    });
}
