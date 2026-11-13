# `tantra-ui-charts` — Decimal Chart Geometry Primitives

Portable / WASM. Deps: `tantra-core`, `rust_decimal`, `serde`. Not a port of the original
`openalgo-charts-rs` canvas renderer — the crate was written fresh as a **geometry** kernel;
rendering to canvas/ratatui happens in the consumer.

## Modules

| Module | Contents |
| :--- | :--- |
| `candle.rs` | `Candle` (Decimal OHLC), `CandlestickChart` with `from_bars`, `price_range`, `y_for_price`/`x_for_candle` (Decimal→u16 pixel mapping with exact rounding), `GeometryError::{EmptySeries, ZeroViewport, FlatPriceRange}`. |
| `series.rs` | `ChartPoint`, `ChartSeries`, `LineChart` (curve geometry). |
| `axis.rs` | `ChartRange`, `AxisTicks`, `tick_positions`. |
| `theme.rs` | `ChartPalette`. |

## Design notes

- **Geometry only**: the lib.rs doc states these types describe geometry; rendering is the
  consumer's job. There is no depth-chart module and no canvas engine in this crate (the old
  `architecture.md` claim of a "HTML5 Canvas candlestick, depth, curve, and indicator rendering
  engine" was wrong for this crate).
- Zero floats at the API: OHLC, price ranges, and pixel mapping all go through
  `rust_decimal::Decimal`.

## Tests

`tests/candle_test.rs`, `series_test.rs`, `axis_test.rs`, `palette_test.rs`, `edge_cases_test.rs`,
`roundtrip_test.rs`, `geometry_test.rs` (33 `#[test]` functions).
