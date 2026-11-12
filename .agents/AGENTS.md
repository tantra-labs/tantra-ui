# `tantra-ui` Agent Guidelines & Architecture Invariants

**Subsystem**: Face / UI (WebAssembly Strategy Studio, Native TUI & CLI)
**Language**: Rust (Leptos / Ratatui)
**Primary Responsibility**: Leptos 0.8 WebAssembly workstation (`tantra-ui-app`), native terminal UI (`tantra-tui`), and developer CLI (`tantra-cli`).

---

## 1. Role & Architectural Hints
* `tantra-ui` consolidates the full workstation presentation layer:
  1. **Leptos 0.8 WebAssembly**: High-performance client-side rendered Strategy Studio and Futuristic Neural AST Screener compiled directly to `wasm32-unknown-unknown`.
  2. **Ratatui Terminal UI (`tantra-tui`)**: Ultra-low-latency desktop terminal monitoring active orders, risk telemetry, and market depth.
  3. **CLI (`tantra-cli`)**: Developer command-line interface for backtest runs, strategy validation, and broker auth flows.
  4. **UI Presentational Purity**: Components maintain clean separation between layout/rendering atoms and reactive state signals.
* **Idea Inheritance**:
  - **StockSharp**: Odysseus visual strategy designer and Studio workbench.
  - **Bitfinex UI / OpenAlgo**: Financial terminal docking, depth meters, and scanner matrix.

---

## 2. Coding Guidelines (DDD & TDD)
* **WASM Targetable**: Web crates compile cleanly to `wasm32-unknown-unknown`. Never use native OS threads or synchronous blocking filesystem calls in WASM modules.
* **Zero Floats Policy**: All prices, ratios, returns, bounds, and metrics strictly use `rust_decimal::Decimal`.
* **TDD & Parallel Test Directory Rule (MANDATORY)**:
  - All tests must live in `tests/` parallel to `src/`, or in a `src/tests/` module directory.
  - **STRICTLY FORBIDDEN**: In-file `#[test]` inside production `src/` files.
* **Check .agents/HANDOFF.md**: Always verify handoff status before starting work.

---

## 3. Development & CI Workflow
* **Linting & Formatting**:
  ```bash
  cargo fmt --check
  cargo clippy --all-targets -- -D warnings
  ```
* **Testing**:
  ```bash
  cargo test --all
  ```
