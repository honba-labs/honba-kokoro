# 本場・心 honba-kokoro

> *The heart of the real thing.*

Pure domain types, risk primitives, portfolio, and OMS for the honba Indian-equities (NSE/BSE) trading platform. No I/O.

## Crates
- `honba-kokoro-types` — Price, Quantity, InstrumentId, enums
- `honba-kokoro-risk` — RiskManager, RiskLimits, SEBI OTR
- `honba-kokoro-portfolio` — INR holdings, FIFO PnL, T+1
- `honba-kokoro-oms` — order state machine

## Test
    cargo test --workspace

License: AGPL-3.0.
