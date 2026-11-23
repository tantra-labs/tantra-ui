# 0008 — Synthetic sine-wave bars drive strategy run/tune/suggest in `tantra-ui-core`

**Status**: open
**Filed**: 2026-11-23 — workspace design review (root plan 0021) · **Kind**: design-flaw · **Severity**: high
**Plan**: [0011](../plans/0011-design-review-2026-11.md)

## What's stubbed or hardcoded

- `crates/tantra-ui-core/src/strategy.rs:782-815` `generate_market_bars` builds bars from `f64` `sin()` cycles (`((i as f64) * 0.12).sin() * 0.015` at `:798`) and converts with `Decimal::from_f64(...).unwrap_or(Decimal::new(100,0))`; base prices are hard-coded per symbol (`"RELIANCE" => 2800.0`).
- `crates/tantra-ui-core/src/strategy.rs:842,875,897,1018` — `run`, `tune` and `suggest_trade` all call `generate_market_bars(symbol,120)`.
- The same module calls `Utc::now()` at `:849`, `:904`, `:948`, `:1122`, `:1345`, although the file header (l.3) claims "zero-floats ... pure".
- `crates/tantra-ui-core/src/strategy.rs:423,485-501` Sharpe / deflated-Sharpe maths via `to_f64`/`f64`.
- `crates/tantra-ui-core/src/strategy.rs:520-523,529,570` CWD-relative manifest search and `std::fs` inside a crate documented as WASM-safe and pure.
- No gap or plan citation sits near any of these stand-ins.

## Why

A user tuning or asking for a suggestion on `RELIANCE` receives numbers derived from a sine wave and presented as strategy results. Float rounding pollutes Decimal metrics, `Utc::now()` breaks determinism, and manifest discovery depends on the process working directory. Breaks `.agents/rules/interface-first-and-modularity.md` §2 (zero-clock, zero-floats), the "no silent fake defaults" rule in §1.3, and `.agents/rules/gap-tracking.md` §2 (uncited stand-in).

## How to close it

1. Route `run`/`tune`/`suggest_trade` through `tantra-runtime-client` backtests (real bars), or require an explicit `bars` parameter.
2. Move `generate_market_bars` to a fixture under `crates/tantra-ui-core/tests/`; every non-fixture caller returns an `Unavailable`-style error instead of synthetic data.
3. Inject `now: DateTime<Utc>` into the entry points that currently call `Utc::now()`.
4. Move Sharpe/DSR maths to `tantra-core` analytics (Decimal / fixed-point), or document a float-metric carve-out.
5. Put manifest scanning behind a `ManifestSource` trait implemented in `tantra-cli`.

**Verification**: golden vector for `tune` on a recorded tape fixture; deterministic-clock test; a test asserting no-bars calls return `Unavailable`; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `make check`, `cargo test -p tantra-ui-core --profile prod`, `make prod`, wasm32 check of `tantra-ui-core`.

## Related

- Root gap 0012 (fabricated or synthetic data presented as real).
- [0011](0011-ui-core-purity.md) (same file: filesystem, clock, text-to-float parsing).
- Root plan 0019 P5.3 touches `scene_projector.rs` only; nothing covers `strategy.rs`.
