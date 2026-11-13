# Gap 0007 — Charts Lack Level / Zone / Annotation Primitives

**Status**: open — tracked by [plan 0006](../plans/0006-elephant-edge-strategy-studio.md)

## What's Missing

`tantra-ui-charts` is geometry-only: it can draw candles and basic shapes but
has **no semantic primitives** for a level-mapped strategy chart:

- **Horizontal level lines** with attached labels/values (PDH/PDL, gamma flip,
  max pain, strike lines).
- **Shaded zone bands** (Elephant support/resistance percentile bands, Order
  Block boxes) with a fill + score color.
- **Marker glyphs** (Buy/Sell signal arrows) anchored to a price + timestamp.
- **Entry/SL/TSL line sets** that re-render on `StopUpdate` events (a
  "stop line" is a level that *moves*).
- Annotation labels/tooltips (level kind, score, confidence).

## Why

Plan 0006 (EE+ Strategy Studio) is the first real view-layer consumer and
needs these primitives on the chart before any level-map screen can render.
Building them per-screen would duplicate geometry logic and break the
presentational-purity invariant (`.agents/AGENTS.md` / `tantra-ui` §1.4) —
they belong in `tantra-ui-charts` as reusable, unit-tested primitives.

## How to Close It

1. `LevelLine`, `ZoneBand`, `SignalMarker`, `StopLine` primitives in
   `tantra-ui-charts` (pure geometry over `Decimal` coordinates; no floats).
2. `StopLine` supports an `update(new_stop)` path emitting a re-render — the
   chart side of the TSL `modify_stop` flow (`tantra-exec` gap 0003 /
   `tantra-ledger` gap 0012).
3. Tooltip/annotation rendering + focus/hover hit-testing for the level map.
4. Tests in `tests/` (TDD): geometry golden vectors, band overlap, marker
   anchoring, stop-line update determinism; `wasm32` build clean.

## Verification

`cargo test --workspace --profile prod` green in `tantra-ui`; the primitives
render the golden level-map fixture; `make prod` green end-of-turn
(build-discipline rule).
