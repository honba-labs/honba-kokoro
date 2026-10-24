# `honba-kokoro` Agent Guidelines & Architecture Invariants

**Subsystem**: Heart (Domain Heart)
**Language**: Rust
**Primary Responsibility**: Order Management System (`oms`), FIFO tax-lot engine (`portfolio`), and double-entry general ledger.

---

## 1. Role & Architectural Hints
* `honba-kokoro` contains the domain core:
  1. **OMS State Machine (`oms`)**: Validated transitions for `New` -> `Submitted` -> `PartiallyFilled` -> `Filled` / `Canceled` / `Rejected`.
  2. **FIFO Tax-Lot Accounting (`portfolio`)**: Deterministic lot matching, cost basis calculation, unrealized and realized PnL.
  3. **Double-Entry General Ledger (`ledger`)**: Balanced multi-currency journal entries guaranteeing ledger integrity down to the single paisa.
* **Temporal Determinism**: All calculations receive explicit `DateTime<Utc>`. No wall-clock queries (`SystemTime::now()`).
* **Settlement & Cost Model**: Precise Indian tax and fee accounting: STT, stamp duty, exchange transaction charges, SEBI turnover fees, GST, DP charges.
* **Idea Inheritance**:
  - **NautilusTrader**: Double-entry accounting, tax-lot matching, exact research-live parity.
  - **WonderTrader**: Context-aware position caching and position merging.

---

## 2. Coding Guidelines (DDD & TDD)
* **Rich Domain Aggregates**: Invariants are enforced on construction and state mutation (`portfolio.apply_fill(&fill)?`).
* **Zero Floats Policy**: Decimal arithmetic only (`rust_decimal::Decimal`). Zero floats.
* **TDD & Parallel Test Directory Rule (MANDATORY)**:
  - All tests must live in `tests/` parallel to `src/`.
  - **STRICTLY FORBIDDEN**: In-file `#[test]` inside `src/`.
  - Maintain golden accounting vectors in `tests/golden/`.

---

## 3. Retargetability & Portability
* Portfolio and ledger math compile to `wasm32-unknown-unknown` for web/simulator embedding.

---

## 4. Development & CI Workflow
* **Linting & Formatting**:
  ```bash
  cargo fmt --check
  cargo clippy --all-targets -- -D warnings
  ```
* **Testing**:
  ```bash
  cargo test --all
  ```
