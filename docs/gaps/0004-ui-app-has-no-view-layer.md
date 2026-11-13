# 0004 — `tantra-ui-app` has feature-state models but no Leptos/CSR view layer

**Status**: open

## What's stubbed or hardcoded

`crates/tantra-ui-app/` contains a complete, well-tested set of **feature-state models**
(`Router`/`AppRoute`, `ScreenerFeature`, `StudioCanvas`, `SimulatorFeature`, `SystemFeature`,
`WatchlistFeature`, `BacktestFeature`, `PortfolioFeature`, `AlgosFeature`), but there is **no
Leptos dependency anywhere in the workspace** (zero matches in `Cargo.lock`) and therefore no
client-side-rendered browser view layer. The old `docs/design/architecture.md` described
`tantra-ui-app` as a "Leptos 0.8 CSR full browser workstation" — that view layer has never been
written. The crate is WASM-safe by design (CI runs `cargo check -p tantra-ui-app
--target wasm32-unknown-unknown`).

## Why

The tier-5A migration guide scoped `tantra-ui-app` to the feature/state layer and left the view
layer as a follow-up (recorded in `.agents/HANDOFF.md` "Immediate Next Steps" item 1). The
features are presentational-purity-ready: view models are `serde`/`Decimal`-only and carry no
I/O, so a future Leptos 0.8 CSR layer can bind directly to them.

## How to close it

- Add a `leptos` (0.8 CSR) dependency surface inside `tantra-ui-app` (or a new
  `tantra-ui-app-web` crate) that renders the eight `AppRoute` views from the existing feature
  state models.
- Keep the WASM invariant: no `tokio`/`reqwest`/native-only deps in the crate; use the
  workspace `uuid` `rng-getrandom` + `getrandom` `wasm_js` feature path already wired for
  `wasm32-unknown-unknown`.
- Render the screener via `ScreenerFeature::run_scan` (which already calls
  `tantra_ui_api_wasm::screener::apply_scan`) and the studio via `StudioCanvas` (cycle-detecting
  node graph).
- Verify `cargo check -p tantra-ui-app --target wasm32-unknown-unknown` stays green and add
  view-layer tests in `tests/` parallel to `src/`.
