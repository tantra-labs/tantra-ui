# `tantra-ui` — Migration Map: `honba-kao` → `tantra-ui`

Dismantled from the former `docs/roadmap/`. The migration record lives here; the original
`docs/migration-guide.md` is retained at `docs/migration-guide.md` as the historical tier-5A
guide and is linked from `docs/plans/0001`.

## Source → Target mapping

| Source crate (`honba-kao/crates/`) | Target crate (`tantra-ui/crates/`) | Notes |
| :--- | :--- | :--- |
| `honba-kao-ui-core` | `tantra-ui-core` | UI atoms/molecules + view models + strategy primitives. |
| `honba-kao-assets` | `tantra-ui-assets` | Embedded assets, themes, fonts, fixtures, audio. |
| `honba-kao-client` | `tantra-ui-client` | In-memory push `EventStream`/`Subscription` bus. |
| `openalgo-charts-rs` | `tantra-ui-charts` | Decimal chart geometry primitives (rendering deferred to consumers). |
| `honba-kao-api-wasm` | `tantra-ui-api-wasm` | Pure-Decimal analytics/indicators/margin/pairing/payoff/pricing/replay/screener. |
| `honba-kao-app` | `tantra-ui-app` | WASM-safe feature-state models (router, screener, studio, simulator, system, watchlist, backtest, portfolio, algos). |
| *(new)* | `tantra-highlight` | Zero-I/O lexer (`Language`, `HighlightSpan`, `highlight_code`), no tree-sitter. |
| *(new)* | `tantra-tui` | `TuiApp`, 7-tab `ActiveTab`, keybindings, `run_terminal` event loop. |
| *(new)* | `tantra-cli` | `CliArgs` + 15 `Command` variants, `run`/`run_with_provider`, `JobsRuntime`. |

## Key migration decisions

1. **Fresh implementation**: `honba-workspace` was absent during migration, so all nine crates
   were written fresh against the contracts in `docs/migration-guide.md` and
   `docs/plans/0001` rather than ported line-for-line.
2. **Strict WASM isolation**: `tokio`/`ratatui`/`crossterm`/`clap` are depended on only by
   `tantra-{cli,tui}`; the six WASM crates carry zero async/native deps. `wasm32-unknown-unknown`
   RNG for `uuid` v4 is enabled via workspace `uuid` `rng-getrandom` + `getrandom` `wasm_js`
   features, activated from `tantra-ui-app`.
3. **Screener canonicality**: the UI builds only against `tantra_core::screener` wire contracts,
   evaluated by `tantra-ui-api-wasm::screener::apply_scan`; the
   `tantra-market-indicators::screener` module is a standalone duplicate not used by
   `tantra-ui` or `tantra-agent` (resolved in `docs/plans/0005` §2 item 2).
4. **Zero floats at the wire boundary**: all public wire types are `rust_decimal::Decimal`; the
   `f64` internals of `tantra-ui-core::strategy` math (norm_cdf, sqrt, DSR, synthetic bars,
   sparkline normalization) are the workspace's documented indicator/optimizer seam class.

## Verification commands

```bash
grep -rn "#\[cfg(test)\]" crates/*/src/   # zero inline tests in production source
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace --profile prod
cargo check -p tantra-ui-app --target wasm32-unknown-unknown
```
