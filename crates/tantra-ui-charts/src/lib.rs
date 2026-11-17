//! # व्यापार・face tantra-ui-charts
//!
//! Chart geometry primitives for the Tantra workstation. This is the
//! re-export alias for the `openalgo-charts-rs` heritage (migration-guide.md
//! §1: *"Rename crate `openalgo-charts-rs` → `tantra-ui-charts` (or re-export
//! alias `openalgo_charts_rs`)"*).
//!
//! Contracts:
//! - **Zero-floats**: all price/value math is `rust_decimal::Decimal`.
//!   Pixel geometry is computed from Decimal through exact rounding into
//!   integer coordinates (`u16`), never through `f32`/`f64` intermediaries.
//! - **WASM-safe**: no I/O, no `tokio`, compiles for `wasm32-unknown-unknown`.
//! - **Presentational purity**: these types describe *geometry*; rendering to
//!   canvas/ratatui happens in the consumer.
//! - **Tests in `tests/`** (7 chart suites per migration-guide).

pub mod axis;
pub mod candle;
pub mod levels;
pub mod markers;
pub mod series;
pub mod stops;
pub mod theme;
pub mod zones;

pub use axis::{AxisTicks, ChartRange, tick_positions};
pub use candle::{Candle, CandlestickChart, GeometryError};
pub use levels::{LevelLine, LevelLineStyle};
pub use markers::{MarkerKind, MarkerSide, SignalMarker};
pub use series::{ChartPoint, ChartSeries, LineChart};
pub use stops::{StopKind, StopLine};
pub use theme::ChartPalette;
pub use zones::ZoneBand;
