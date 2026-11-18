# 0009 — FIRE Planning Workstation Views (`tantra-ui-core`, Leptos CSR, `tantra fire` CLI, TUI)

**Status**: Proposed (2026-11-18) — Track **T19.6**

Master plan: root [`docs/plans/0013-fire-planning-and-cashflow-platform.md`](../../../docs/plans/0013-fire-planning-and-cashflow-platform.md).
Depends on: [`tantra-ledger` plan 0006](../../../tantra-ledger/docs/plans/0006-fire-bucket-glidepath-and-swr-metrics.md), [`tantra-runtime` plan 0008](../../../tantra-runtime/docs/plans/0008-fire-planning-persistence-and-rest-api.md).

---

## 0. Executive Summary & Objective

Surface `fireapp`'s FIRE simulator and 3-bucket cashflow views in the native Rust UI ecosystem (`tantra-ui-core`, `tantra-ui-app` Leptos CSR, `tantra-cli`, and `tantra-tui`) powered by `tantra-ledger-allocation::fire`, without touching `tantra-core` or breaking `wasm32-unknown-unknown`, Zero-Floats, or `#![deny(unsafe_code)]`.

---

## 1. Crate-by-Crate Design

1. **`tantra-ui-core` (`src/fire.rs`, `wasm32`-safe)**:
   - Add `tantra-ledger-allocation` workspace dependency (pure `Decimal`, zero I/O, `wasm32`-clean).
   - Pure view models mapping `FireSimulationReport` into presentational primitives:
     - `FireHeroSummary`, `FireArchetypeCardVm`, `FireChartSeries` (`EquityPoint`-style nominal vs real series + crossover marker), `CashflowMixVm`, `FirePlannerViewModel`.
   - Helper formatting INR in Lakhs/Crores using `rust_decimal::Decimal` cleanly.
2. **`tantra-ui-app` (`src/fire.rs` + `AppRoute::Fire`)**:
   - Reactive `FireFeature` holding `FirePlanConfig`, `CashflowBucketConfig`, active persona preset, nominal/real toggle, and `wasm32`-compatible deterministic computation via `tantra_ledger_allocation::fire` directly in-browser (no server required for instant slider responsiveness, plus `tantra-ui-client` sync when connected to `tantra serve`).
   - Leptos 0.8 CSR view rendering the 6-step workflow (`Expenses`, `Multiplier`, `Runway`, `Chart + Math Disclosure`, `5 Archetypes`, `Post-FIRE Cashflow + 3 Pillars`).
3. **`tantra-cli` (`tantra fire`)**:
   - Subcommands: `tantra fire simulate`, `tantra fire from-portfolio --account <id>`, `tantra fire export`.
   - Renders using `block_render.rs` ANSI `DisplayBlock` (`MetricGrid` + trajectory `Table` + `--json`).
4. **`tantra-tui`**:
   - Add a `Fire` tab / panel in `TuiApp` showing live target corpus, years to freedom, archetype progress bars, and cashflow coverage hydrated via `LiveData`.

---

## 2. Tests & Verification

- `tantra-ui-core/tests/fire_view_tests.rs`, `tantra-ui-app/tests/fire_feature_tests.rs`, `tantra-cli/tests/fire_cli_tests.rs`, `tantra-tui/tests/fire_tui_tests.rs`.
- Gate: `cargo check -p tantra-ui-core -p tantra-ui-app --target wasm32-unknown-unknown --profile prod`, per-crate `--profile prod` tests, `cargo fmt --check`, `cargo clippy --workspace --all-targets --profile prod -- -D warnings`, `make prod`.
