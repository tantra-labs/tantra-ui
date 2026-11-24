# 0006 — Elephant Edge (EE+) Strategy Studio

**Status**: Implemented (2026-11-17) — Phase 1 (`f597915`, gap 0007 closed); Phases 2–4 (T12.9 remainder)
**Target Scope**: `tantra-ui-app` (Leptos 0.8 WASM), `tantra-tui`, `tantra-cli`,
`tantra-ui-charts`
**Depends on**: `tantra-ui` gap 0004 (ui-app has no view layer), gap 0007
(charts lack level/zone/annotation primitives); `tantra-core` plan 0003
(wire types); `tantra-runtime-api-rest` (level-map + OI/options endpoints)
**Master plan**: root [`docs/plans/0005-elephant-edge-strategy-platform.md`](../../../docs/plans/0005-elephant-edge-strategy-platform.md)

## 0. Objective

Add an **Elephant Edge Strategy Studio** to the web workstation (and a compact
TUI/CLI readout): a visual workspace where a trader sees the day's full level
map, the strategy's live signals with their SL/TSL and 3R/5R target lines, and
the proposed positioning — with the same 1:1 wire types the Python strategy and
Rust REST API already emit (`tantra-core` plan 0003).

## 1. Screens

### 1.1 Level-Map Canvas (`tantra-ui-app` + `tantra-ui-charts`)
- **Session anchor**: the first 1-minute candle (09:15–09:16 IST) highlighted;
  its open/high/low/close drawn as reference lines.
- **Elephant zones**: the two support ($S_1, S_2$) + two resistance ($R_1, R_2$)
  bands drawn as shaded **zones** (not lines), re-colored by confluence score
  (0–4) and annotated with snapped OI strike walls where applicable.
- **Overlays**: OI walls (top-2 CE/PE) with $\Delta\text{OI}$ building vs
  short-covering badges, gamma-flip line, max-pain strike, PDH/PDL/PDC,
  round-number strike lines, **Order Block** boxes (bullish OB below / bearish
  OB above), and **Elephant Bar** markers on expansion candles.
- **Regime & Cycle badge**: long-gamma (pin/mean-revert) vs short-gamma
  (trend/breakout) chip from the gamma flip vs spot, Local Range PCR readout,
  and a **Mid-Month Cycle (15th $\pm$ 1d) Reversal Alert** badge when the
  session falls inside the 14th–16th window with a stretched 2-week trend.

### 1.2 Signals & Stop Panel
- Signal markers (Buy/Sell, kind = Breakout/Reversal/Retest) at the level
  where the trigger candle confirmed; entry line, **initial SL line ($1R$)**,
  **$3R$ primary target / $4R\text{--}5R$ runner target lines**, and **trailing
  stop line** drawn and updated as `StopUpdate`s arrive (WS/SSE from
  `tantra-runtime-api-rest`).
- Per-signal card: `ref_levels`, confluence score, confidence, risk/R:R vs the
  IV-implied expected move, and the **6-candle stagnation time-stop** countdown.

### 1.3 Positioning Table
- `PositionIntent` rows: instrument, side, suggested quantity (whole lots),
  risk ₹, SL distance, max-position-fraction utilisation, per-day trade count,
  6-candle hold counter, and square-off countdown to 15:10.
- Same table rendered in `tantra-tui` (Positioning tab) and as a `tantra-cli`
  `elephant` report command (mirroring the canned-report gap 0003 cleanup).

## 2. Data & Wiring

- Consume `GET /level-map/{date}` / `GET /signals/{date}` /
  `GET /option-chain/walls/{instrument}` + `WS /ws/live` (market + OI feeds)
  once `tantra-runtime-api-rest` exposes the `tantra-core` plan 0003 types.
- **Honest empty states**: until the upstream data gaps close, panels render
  `unavailable` with the cited gap link (same honest-degradation pattern used
  across the workspace).
- All prices/quantities `rust_decimal::Decimal`; charts remain
  `wasm32-unknown-unknown`-clean (no OS threads, no sync FS).

## 3. Phases

- **Phase 1**: chart primitives (gap 0007) — horizontal/zone/box/line/marker
  renderers + annotations in `tantra-ui-charts`, unit-tested geometry.
- **Phase 2**: level-map canvas + regime badge on seeded data (gap 0004 view
  layer work — ui-app grows its first real views).
- **Phase 3**: signals + stop/target lines + positioning table, live-wired.
- **Phase 4**: TUI Positioning tab + CLI `elephant` report; docs (root T12).

## 6. Implementation note (Phases 2–4, T12.9 remainder)

Landed as `tantra-ui-core/src/ee.rs` — the single shared pure view-model
source (`StudioViewport`, `studio_ee_bands`/`studio_ee_lines`/
`studio_ee_marks`/`studio_ee_stops`/`studio_ee_regime`/`studio_ee_positions`
+ `ee_seed_*` fixtures + `ee_level_report_rows`/`ee_signal_report_rows`),
consumed by:
- **Web** (`tantra-ui-app/src/studio_ee.rs` `StudioEeFeature` + `StudioView`
  sections in `src/web.rs`): level-map zone/line summary, regime chip,
  signal markers + SL/3R/5R stop rows, positioning table, honest
  `unavailable` note; the existing `StudioCanvas` node canvas is preserved.
- **TUI** (`tantra-tui/src/positioning.rs` `positioning_panel_text` — an EE+
  Positioning section in the Portfolio tab).
- **CLI** (`tantra-cli/src/elephant.rs` — `tantra elephant` report).

`StopKind::Target` was added to `tantra-ui-charts` for the 3R/5R target lines.
Entry = referenced level price (`LevelId` encodes it in the seeds); 1R = 1% of
entry (seeded heuristic). Tests: `tantra-ui-core/tests/ee_test.rs` (8),
`tantra-ui-app/tests/studio_ee_test.rs` (3), `tantra-tui/tests/positioning_test.rs`
(3), `tantra-cli/tests/elephant_test.rs` (5), all Decimal-exact.

## 4. Quality Gates

- `cargo fmt --check`, `cargo clippy --workspace --all-targets --profile prod
  -- -D warnings`, `cargo test --workspace --profile prod`, `make prod`
  end-of-turn.
- Tests in `tests/` only; charts geometry golden vectors; no floats.

## 5. Related

- Chart gap 0007 (see [`docs/gaps/README.md`](../gaps/README.md)) — **closed 2026-11-17 (T12.9)**: `LevelLine`/`ZoneBand`/`SignalMarker`/`StopLine` shipped in `tantra-ui-charts`; this Studio view layer consumes them
- Strategy spec: [`tantra-strategies` plan 0001](../../../tantra-strategies/docs/plans/README.md) (EE+ strategy — **implemented 2026-11-17, T12.4**)
- Research (level-map legend & OI/gamma layers): [`docs/research/elephant-edge-strategy.md`](../../../docs/research/elephant-edge-strategy.md)
