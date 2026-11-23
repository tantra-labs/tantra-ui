# 0011 — `tantra-ui-core` is not pure: filesystem, wall clock, floats, CWD paths

**Status**: open
**Filed**: 2026-11-23 — workspace design review (root plan 0021) · **Kind**: design-flaw · **Severity**: medium
**Plan**: [0011](../plans/0011-design-review-2026-11.md)

## What's stubbed or hardcoded

- `crates/tantra-ui-core/src/strategy.rs:6-7` `#[cfg(not(target_arch="wasm32"))] use std::fs;`; `:520-523` relative manifest directories; `:529,570` `read_dir`/`read_to_string`.
- `Utc::now()` at `:849,904,948,1122,1345` of the same module.
- `crates/tantra-cli/src/backtest_gen.rs:67-91` parses user text into `f64` and then calls `Decimal::from_f64_retain(num * 100_000.0)`: money derived from a float (`"1.1" lakh` yields `110000.00000000000727`-style artefacts).
- `crates/tantra-tui/src/strategy_lab.rs` uses `f64` for rendering coordinates (44 hits); this is acceptable and is not in scope.
- `tantra-ui-core` is documented as WASM-safe but is compiled for wasm only through cfg gates, so web/wasm and native diverge: wasm returns a builtin-only strategy list.

## Why

Capital and amount parsing from text produces non-exact Decimals that reach order sizing, violating `.agents/rules/interface-first-and-modularity.md` §2.3 (zero-floats). Filesystem access and the wall clock break §2.1/§2.2 and the WASM targetability rule; native and wasm surfaces list different strategy sets.

## How to close it

1. Parse "N lakh/cr/k" with `Decimal::from_str` and scale by `Decimal` constants; remove `from_f64_retain` from `backtest_gen.rs`.
2. Introduce a `StrategySource` port with a native implementation in `tantra-cli` (filesystem) and a wasm implementation (fetch); remove `std::fs` from `tantra-ui-core`.
3. Inject `now: DateTime<Utc>` at the boundary (see [0008](0008-synthetic-bars-in-run-tune-suggest.md)).
4. Add a wasm32 build to CI for `tantra-ui-core`.

**Verification**: proptest that "N lakh/cr/k" parsing round-trips exactly against `Decimal`; test that native and wasm sources list the same builtin set; `cargo check -p tantra-ui-core --target wasm32-unknown-unknown`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test -p tantra-ui-core --profile prod`, `make prod`.

## Related

- [0008](0008-synthetic-bars-in-run-tune-suggest.md) (same file `strategy.rs`).
- Root plan 0019 P5.3 (module moves in `tantra-ui-core`).
