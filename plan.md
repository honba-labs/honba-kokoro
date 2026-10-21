# Master Plan: 本場・心 honba-kokoro

> **The heart of the real market.**  
> Pure, deterministic domain state, institutional Order Management System (OMS), and multi-account portfolio accounting engine for Indian equities and derivatives (NSE/BSE/NFO/MCX). Zero I/O, zero clock dependency, 100% exact decimal financial arithmetic (`rust_decimal::Decimal`), SEBI regulatory compliance, and verifiable mathematical correctness.

---

## 0. Executive Summary & Architectural Audit

### 0.1 Current State vs Target Architecture

`honba-kokoro` was initially conceived as the "heart" containing types, risk, portfolio, and OMS. However, during early development, legacy baggage (`honba-messages`, `honba-entities`, `honba-ports`) was ported over from older experiments, and standalone crates (`honba-core` and `honba-mon`) were created. This resulted in severe duplication, type bifurcation (`f64` vs `Decimal`), and fragmented responsibilities:

| Area | Current State in `honba-kokoro` | Target State (Master Architecture) | Severity |
| :--- | :--- | :--- | :--- |
| **Domain Types** | Duplicated across `honba-kokoro-types` (minimal), legacy `honba-entities` / `honba-messages` (with `f64`), and external `honba-core`. | **100% unified on `honba-core`**: Kokoro depends on `honba-core` for `Price`, `Quantity`, `Money`, `InstrumentId`, `OrderRequest`, `OrderStatus`. Legacy `types`, `messages`, and `entities` are fully deprecated and excised. | **P0 (Critical)** |
| **Risk Gate Separation** | `honba-kokoro-risk` implements `RiskStage` (collars, rate limits, freeze quantities), while `honba-mon` implements `RiskEngine` (SEBI algo ID, OTR circuit breaker, price bands). | **Consolidate Risk into `honba-mon`**: `honba-mon` is the authoritative synchronous pre-trade risk gate. Kokoro's OMS evaluates internal state-transition validity, delegating compliance checks directly to `honba-mon`. | **P0 (Critical)** |
| **Ports & Interfaces** | `honba-kokoro-ports` defines `ExecutionPort` and `MarketDataFeed`, conflicting with `honba-me-core::MarketDataFeed` and `honba-te-core::BrokerExecutor`. | **Excise `honba-kokoro-ports`**: Kokoro is strictly zero-IO. Ingestion interfaces belong in `honba-me-core`; execution traits belong in `honba-te-core`. | **P0 (Critical)** |
| **Order Management System (OMS)** | `honba-kokoro-oms` is a 43-line stub with basic transitions. Lacks idempotency, multi-leg orders, algo ID tracking, and cancellation queues. | **Industrial-Grade OMS (`honba-kokoro-oms`)**: Deterministic state machine, client order ID (clOrdId) deduplication ledger, multi-leg / bracket / OCO order coordination, and formal `proptest` reachability proofs. | **P0 (Critical)** |
| **Portfolio & Holdings Engine** | `honba-kokoro-portfolio` uses legacy `f64` prices/quantities and minor-unit hacks, lacking Demat delivery vs intraday product separation. | **Tax-Lot FIFO Engine (`honba-kokoro-portfolio`)**: Decimal-accurate FIFO lot matching, Realized & Mark-to-Market (MTM) PnL, Indian corporate actions adjustment (splits, bonuses, dividends), and SEBI T+1 Demat settlement lifecycle. | **P0 (Critical)** |
| **Financial Ledger & Audit** | No double-entry ledger. Cash, margins, and charges are computed ad-hoc across simulation modules. | **Immutable Double-Entry Ledger (`honba-kokoro-ledger`) [NEW]**: Append-only transaction journal tracking cash balance, margin allocations (SPAN + Exposure), STT, GST, stamp duty, and exchange turnover charges. | **P1 (High)** |

---

## 1. Architectural Principles & System Integration

```text
               ┌─────────────────────────────────────────────────────────┐
               │              honba-me (Market Data / Eye)               │
               │         Canonical Ticks, Quotes, Bars (honba-core)      │
               └────────────────────────────┬────────────────────────────┘
                                            │ Market Marks
                                            ▼
┌────────────────────────────────────────────────────────────────────────────────────────┐
│ 本場・心 honba-kokoro Subsystem (Deterministic Domain Heart — Zero I/O)                 │
│                                                                                        │
│  ┌────────────────────────┐      ┌────────────────────────┐      ┌──────────────────┐  │
│  │    honba-kokoro-oms    │      │ honba-kokoro-portfolio │      │   honba-kokoro-  │  │
│  │   Idempotent FSM &     │─────▶│  FIFO Tax-Lots, MTM,   │─────▶│      ledger      │  │
│  │   Order Lifecycle      │      │  T+1 Demat Settlement  │      │ Double-Entry Cash│  │
│  └───────────┬────────────┘      └────────────────────────┘      └──────────────────┘  │
│              │ Order Events                                                            │
└──────────────┼─────────────────────────────────────────────────────────────────────────┘
               │ Evaluates Orders via
               ▼
┌───────────────────────────────────────┐          ┌─────────────────────────────────────┐
│      honba-mon (Risk Gate / Door)     │          │      honba-te (Execution / Hand)    │
│  SEBI Algo ID, OTR Breaker, Price Band│◀─────────┤ Broker Adapters (Kite, Upstox, etc.)│
└───────────────────────────────────────┘          └─────────────────────────────────────┘
```

### Key Tenets
1. **Zero-I/O Purity & Temporal Determinism**: `honba-kokoro` never opens a network socket, never reads a database, and never queries the system clock (`std::time::Instant` or `SystemTime::now`). Every transition accepts an explicit `DateTime<Utc>`, making replays, backtesting, and unit tests 100% deterministic and byte-reproducible.
2. **Strict Monetary Purity (`rust_decimal::Decimal`)**: Zero floating-point arithmetic. Prices, quantities, notional turnover, broker commissions, and statutory Indian taxes (STT, GST, SEBI turnover fees, Stamp Duty) are modeled with exact 128-bit decimal precision.
3. **SEBI Regulatory Conformance as First-Class Invariants**: Indian market microstructure is baked into the type system: T+1 settlement cycles, Demat delivery segregation (CNC/Delivery) vs Intraday (MIS) leverage, and mandatory SEBI Algo ID tagging on all state changes.
4. **Formal State Reachability**: All state transitions in the OMS and Portfolio engines are mathematically closed and proven via property-based testing (`proptest`), preventing phantom fills, double-allocations, or invalid lifecycle progressions.

---

## 2. Crate Topology & Dependency Rules

The restructured `honba-kokoro` workspace is streamlined from 7 legacy crates to 3 highly cohesive, zero-I/O crates:

```text
honba-kokoro/
├── Cargo.toml
├── README.md
├── plan.md
└── crates/
    ├── honba-kokoro-oms/        # Idempotent order state machine, bracket/OCO coordinator, fill reconciliation
    ├── honba-kokoro-portfolio/  # Multi-account FIFO tax-lot engine, MTM valuation, T+1 settlement, corporate actions
    └── honba-kokoro-ledger/     # Double-entry cash journal, margin allocation, statutory tax tracking [NEW]
```

### Layering & Dependency Hierarchy
```text
Layer 2: [honba-kokoro-portfolio] ──▶ depends on [honba-kokoro-oms], [honba-kokoro-ledger]
             │
Layer 1: [honba-kokoro-oms] ───────▶ depends on [honba-core]
             │
Layer 0: [honba-core] ─────────────▶ Zero-dependency canonical domain types (Decimal, UTC)
```

### Deprecation & Pruning Directive
- ❌ **`honba-kokoro-types`**: Deleted; merged into canonical `honba-core`.
- ❌ **`honba-kokoro-ports`**: Deleted; I/O ports reside in `honba-me-core` (feeds) and `honba-te-core` (execution).
- ❌ **`honba-kokoro-messages`**: Deleted; replaced by `honba-core` events and `honba-kokoro-oms` transitions.
- ❌ **`honba-kokoro-entities`**: Deleted; legacy `f64` structures replaced by `honba-core`.
- ❌ **`honba-kokoro-risk`**: Merged into `honba-mon` to unify pre-trade risk enforcement.

---

## 3. Deep-Dive Specification & Core Modules

### 3.1 `honba-kokoro-oms`: Order Management & State Transition Engine
- **Formal Order Lifecycle State Machine**:
  - Implements the complete Indian market order lifecycle:
    $$\text{PendingNew} \longrightarrow \text{New} \rightleftarrows \text{PartiallyFilled} \longrightarrow \text{Filled}$$
    $$\text{PendingNew} / \text{New} / \text{PartiallyFilled} \longrightarrow \text{PendingCancel} \longrightarrow \text{Cancelled}$$
    $$\text{PendingNew} / \text{New} \longrightarrow \text{Rejected} \quad | \quad \text{Expired}$$
  - Transition invariants:
    - Fills on terminal orders (`Filled`, `Cancelled`, `Rejected`, `Expired`) are captured as `UnsolicitedLateFill` and segregated for manual/reconciliation handling.
    - Cumulative filled quantity $\sum q_{\text{fill}} \le Q_{\text{order}}$ strictly guaranteed.
    - Modification transitions (`PendingReplace` $\rightarrow$ `Replaced`) maintain queue priority and validate price band adherence.
- **Idempotency & Replay Engine**:
  - Client Order ID (`clOrdId`) hash index guaranteeing at-most-once order dispatch.
  - Inbound fill deduplication using broker execution ID (`exec_id`), preventing double-crediting on network reconnects.
- **Complex & Multi-Leg Order Coordinator**:
  - Bracket Orders (Target + Stop-Loss with auto-cancellation of sibling leg on execution).
  - One-Cancels-Other (OCO) orders.
  - Cover Orders (Market/Limit with mandatory initial Stop-Loss trigger).

### 3.2 `honba-kokoro-portfolio`: FIFO Tax-Lots & Indian Settlement
- **FIFO Tax-Lot Matching Engine**:
  - Open positions maintain a queue of unclosed lots: $\text{Lot}(t, P_{\text{entry}}, Q_{\text{remaining}})$.
  - On closing fill, matching proceeds strictly First-In, First-Out:
    $$\text{Realized PnL} = \sum_{\text{lots}} (P_{\text{exit}} - P_{\text{entry}}) \times Q_{\text{matched}} \quad (\text{adjusted for side})$$
  - Exact capital gains tracking (Short-Term Capital Gains classification based on holding duration).
- **Mark-to-Market (MTM) Real-Time Valuation**:
  - Computes instantaneous equity and unrealized PnL given market `Marks`:
    $$\text{Unrealized PnL} = (P_{\text{mark}} - P_{\text{avg\_cost}}) \times Q_{\text{open}}$$
  - Intraday high-water mark, maximum drawdown, and portfolio beta computation.
- **SEBI T+1 Settlement & Demat Delivery Lifecycle**:
  - Distinguishes between:
    - **Intraday (MIS)**: Must square off before 15:15 IST; zero Demat delivery obligation.
    - **Delivery (CNC)**:
      - Day $T$: Trade executed; cash blocked or shares earmarked.
      - Day $T+1$: Clearing corporation (NCL/ICCL) settlement; shares credited to Demat account; cash settled.
  - Models auction penalties and short-delivery handling.
- **Corporate Actions Adjustment**:
  - Automatic position and cost-basis adjustment on Ex-Date:
    - Stock Splits ($1:N$): Quantity multiplied by $N$, entry price divided by $N$.
    - Bonus Issues ($A:B$): Quantity increased proportionally, cost basis diluted.
    - Cash Dividends: Credited to cash ledger on dividend pay date.

### 3.3 `honba-kokoro-ledger`: Double-Entry Accounting & Statutory Cost Engine
- **Double-Entry Financial Journal**:
  - Debits and credits across chart of accounts:
    - `Assets:Cash:BrokerAccount`
    - `Assets:Holdings:EquityDelivery`
    - `Liabilities:MarginPayable`
    - `Expenses:Brokerage`
    - `Expenses:Taxes:STT`
    - `Expenses:Taxes:GST`
    - `Expenses:Taxes:StampDuty`
    - `Expenses:ExchangeFees`
    - `Income:RealizedPnL`
- **Statutory Indian Tax & Cost Calculator**:
  - Exact computation per SEBI and Finance Act rules:
    - **Securities Transaction Tax (STT)**: 0.1% on delivery buy/sell; 0.025% on intraday equity sell; 0.0125% on options exercise.
    - **Exchange Turnover Charges**: NSE ₹2.97 per crore (equity).
    - **SEBI Turnover Charges**: ₹10 per crore.
    - **Stamp Duty**: 0.015% on buy orders.
    - **Goods & Services Tax (GST)**: 18% on (Brokerage + Exchange Charges + SEBI Fees).
    - **Depository Participant (DP) Charges**: Flat ₹13.50 + GST per debit transaction for delivery sales.

---

## 4. Phase-by-Phase Implementation Roadmap

### Phase 1: Canonical Type Unification & Legacy Decoupling (M0) — Week 1–2
- [x] **1.1 Workspace Dependency Purge** (COMPLETED)
  - [x] Add `honba-core` dependency to `honba-kokoro-oms` and `honba-kokoro-portfolio`.
  - [x] Delete `honba-kokoro-types`, `honba-kokoro-ports`, `honba-kokoro-messages`, and `honba-kokoro-entities`.
  - [x] Migrate `honba-kokoro-portfolio` from `f64` to `rust_decimal::Decimal` and `honba_core::Money`.
- [x] **1.2 Merge Risk into `honba-mon`** (COMPLETED)
  - [x] Move valuable rules from `honba-kokoro-risk` (order rate limits, maximum notional, lot size multiples) into `honba-mon::RiskLimits`.
  - [x] Deprecate `honba-kokoro-risk`.

### Phase 2: Industrial OMS & Idempotent FSM (M1) — Week 3–4
- [ ] **2.1 Comprehensive Order State Machine**
  - [ ] Implement `OrderStateMachine` supporting `PendingNew`, `New`, `PartiallyFilled`, `Filled`, `PendingCancel`, `Cancelled`, `Rejected`, `Expired`.
  - [ ] Implement transition matrix with explicit `TransitionError` variants.
  - [ ] Add client order ID (`clOrdId`) deduplication and fill reconciliation logic.
- [ ] **2.2 Proptest Reachability Suite**
  - [ ] Formulate property-based state fuzzers with `proptest`:
    - Invariant: Terminal states are absorbing.
    - Invariant: Cumulative fill quantity never exceeds original requested quantity.
    - Invariant: State machine cannot deadlock or panic under arbitrary event interleavings.

### Phase 3: Multi-Account FIFO Portfolio Engine (M2) — Week 5–6
- [x] **3.1 Exact FIFO Matching** (COMPLETED)
  - [x] Implement tax-lot matching queue with microsecond resolution matching.
  - [x] Compute realized PnL on every fill event with zero rounding discrepancy.
- [x] **3.2 MTM Valuation & Account Aggregation** (COMPLETED)
  - [x] Implement `value_portfolio(&Portfolio, &Marks) -> PortfolioSnapshot`.
  - [x] Add multi-account partitioning (e.g. Prop Desk vs Client, MIS vs CNC).

### Phase 4: Indian Market T+1 Settlement & Corporate Actions (M3) — Week 7–8
- [ ] **4.1 T+1 Demat Settlement Engine**
  - [ ] Implement Indian clearing calendar (handling NSE trading holidays, clearing holidays).
  - [ ] Model trade date ($T$) to settlement date ($T+1$) delivery conversions.
  - [ ] Track unsettled receivables vs free Demat holdings.
- [ ] **4.2 Corporate Actions Handler**
  - [ ] Implement split, bonus, dividend, and demerger adjustment algorithms for open lots.

### Phase 5: Double-Entry Ledger & Statutory Costs (M4) — Week 9–10
- [ ] **5.1 Immutable Ledger (`honba-kokoro-ledger`)**
  - [ ] Scaffold `honba-kokoro-ledger` crate.
  - [ ] Implement balanced double-entry journal transactions.
- [ ] **5.2 Indian Cost & Tax Matrix**
  - [ ] Implement exact STT, GST, Stamp Duty, SEBI fee, and DP charge calculation.
  - [ ] Validate tax calculation against broker contract notes (Zerodha, Upstox).

### Phase 6: System Integration & End-to-End Conformance (M5) — Week 11–12
- [ ] **6.1 Wire Kokoro into `honba-ya` Runtime**
  - [ ] Connect `honba-kokoro-oms` to `honba-ya-actors::ExecutionActor`.
  - [ ] Feed fills directly from `honba-te` adapters into `honba-kokoro-portfolio`.
  - [ ] Wire `honba-kao` Web terminal to receive byte-identical portfolio snapshots.

---

## 5. Verification, Conformance & Benchmarks

1. **Broker Contract Note Golden Tests**:
   - Replay 1,000 real executed trade sets from Zerodha and Upstox contract notes. Verify that Kokoro's calculated STT, GST, Stamp Duty, and Net PnL match the broker statement down to the exact paisa (₹0.01).
2. **OMS Fuzzing & Model Checking**:
   - Run 10,000,000 randomized state-transition operations via `proptest`. Zero illegal transitions, zero panics, 100% invariant preservation.
3. **Valuation Latency Benchmarks (`criterion`)**:
   - FIFO tax-lot fill match: **$< 80 \text{ ns}$**.
   - Full portfolio MTM revaluation (100 active positions): **$< 5 \mu\text{s}$**.
   - Order transition validation: **$< 30 \text{ ns}$**.
