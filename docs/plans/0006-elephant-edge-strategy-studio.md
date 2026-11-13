# 0006 — Elephant Edge (EE+) Strategy Studio

**Status**: Proposed
**Target Scope**: `tantra-ui-app` (Leptos 0.8 WASM), `tantra-tui`, `tantra-cli`,
`tantra-ui-charts`
**Depends on**: `tantra-ui` gap 0004 (ui-app has no view layer), gap 0007
(charts lack level/zone/annotation primitives); `tantra-core` plan 0003
(wire types); `tantra-runtime-api-rest` (level-map + OI/options endpoints)
**Master plan**: root [`docs/plans/0005-elephant-edge-strategy-platform.md`](../../../docs/plans/0005-elephant-edge-strategy-platform.md)

## 0. Objective

Add an **Elephant Edge Strategy Studio** to the web workstation (and a compact
TUI/CLI readout): a visual workspace where a trader sees the day's full level
map, the strategy's live signals with their SL/TSL lines, and the proposed
positioning — with the same 1:1 wire types the Python strategy and Rust REST
API already emit (`tantra-core` plan 0003).

## 1. Screens

### 1.1 Level-Map Canvas (`tantra-ui-app` + `tantra-ui-charts`)
- **Session anchor**: the first 1-minute candle (09:15) highlighted; its
  open/high/low drawn as reference lines.
- **Elephant zones**: the two support + two resistance percentile bands drawn
  as shaded **zones** (not lines), re-colored by confluence score (0–4).
- **Overlays**: OI walls (top-2 CE/PE) as vertical markers, gamma-flip line,
  max-pain strike, PDH/PDL/PDC, round-number strike lines, and **Order Block**
  boxes (bullish OB below / bearish OB above).
- **Regime badge**: long-gamma (pin/mean-revert) vs short-gamma
  (trend/breakout) chip from the gamma flip vs spot.

### 1.2 Signals & Stop Panel
- Signal markers (Buy/Sell, kind = Breakout/Reversal/Retest) at the level
  where the trigger candle confirmed; entry line, **initial SL line**, and
  **trailing stop line** drawn and updated as `StopUpdate`s arrive (WS/SSE
  from `tantra-runtime-api-rest`).
- Per-signal card: `ref_levels`, confluence score, confidence, risk/R:R vs the
  IV-implied expected move.

### 1.3 Positioning Table
- `PositionIntent` rows: instrument, side, suggested quantity (whole lots),
  risk ₹, SL distance, max-position-fraction utilisation, per-day trade count;
  square-off countdown to 15:10.
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
- **Phase 3**: signals + stop lines + positioning table, live-wired.
- **Phase 4**: TUI Positioning tab + CLI `elephant` report; docs (root T12).

## 4. Quality Gates

- `cargo fmt --check`, `cargo clippy --workspace --all-targets --profile prod
  -- -D warnings`, `cargo test --workspace --profile prod`, `make prod`
  end-of-turn.
- Tests in `tests/` only; charts geometry golden vectors; no floats.

## 5. Related

- Chart gap: [`docs/gaps/0007-chart-level-and-annotation-primitives.md`](../gaps/0007-chart-level-and-annotation-primitives.md)
- Strategy spec: [`tantra-strategies/docs/plans/0001-elephant-edge-strategy.md`](../../../tantra-strategies/docs/plans/0001-elephant-edge-strategy.md)
- Research (level-map legend & OI/gamma layers): [`docs/research/elephant-edge-strategy.md`](../../../docs/research/elephant-edge-strategy.md)
