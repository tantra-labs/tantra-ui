# `tantra-ui` — TDD & Engineering Roadmap

## Milestone 1: Portable UI Core, Assets, Client, Charts & API-WASM (P0)
- [ ] Port `tantra-ui-core` (from `honba-kao-ui-core`), `tantra-ui-assets` (from `honba-kao-assets`), `tantra-ui-client` (from `honba-kao-client`), `tantra-ui-charts` (from `openalgo-charts-rs`), and `tantra-ui-api-wasm` (from `honba-kao-api-wasm`).

## Milestone 2: Leptos 0.8 WASM Workstation (`tantra-ui-app`) (P0)
- [ ] Port `tantra-ui-app` (from `honba-kao-app`), including the Futuristic Neural AST Screener (`telemetry`, `signal_matrix`, `constellation`, `filter_bar`, `detail_drawer`) and `/components` showcase route.
- [ ] Verify `cargo check -p tantra-ui-app --target wasm32-unknown-unknown` passes cleanly.

## Milestone 3: Native Terminal Workstation (`tantra-highlight`, `tantra-tui`, `tantra-cli`) (P1)
- [ ] Implement `tantra-highlight` (token-based syntax highlighter for Python/Rust/TOML/AST).
- [ ] Implement `tantra-tui` (Ratatui state model & terminal layout views for Screener, Portfolio, and AI Agent).
- [ ] Implement `tantra-cli` (`Clap` command parser & dispatcher for `tui`, `ask`, `scan`, `version`).
