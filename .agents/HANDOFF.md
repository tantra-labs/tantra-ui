# `tantra-ui` — Inter-Agent Handoff (Layer 5)

> Updated: 2026-11-17 — **PLAN 0006 PHASES 2–4 IMPLEMENTED (T12.9 remainder)**:
> the EE+ Strategy Studio view layer over the shipped chart primitives.
> `tantra-ui-core/src/ee.rs` is the single shared pure view-model source
> (`StudioViewport`; `studio_ee_bands/lines/marks/stops/regime/positions`;
> `ee_seed_*` fixtures; `ee_level_report_rows`/`ee_signal_report_rows`),
> consumed by (a) the web Studio (`tantra-ui-app/src/studio_ee.rs`
> `StudioEeFeature` + EE sections in `StudioView`, honest `unavailable` note,
> existing `StudioCanvas` kept), (b) the TUI (`positioning.rs` EE+ section in
> the Portfolio tab, `TuiApp::ee_positions` seeded), and (c) the CLI
> (`tantra elephant` — new `Command::Elephant`/`CliCommand::Elephant` +
> `CommandOutcome::ElephantReport`, `elephant.rs` report). `StopKind::Target`
> added to `tantra-ui-charts`; `tantra-ui-core` now depends on
> `tantra-ui-charts` (wasm-clean, acyclic). Tests: ui-core +8, app +3, tui +3,
> cli +5 (all Decimal-exact). Plan 0006 status → Phases 2–4 Implemented.
>
> Updated: 2026-11-16 — **T14 P3 IMPLEMENTED (plan 0008, all TDD slices
> U1–U5)**: `tantra-ui-api-wasm/src/paper.rs` `PaperSession` (thin
> single-user wrapper over the WASM-portable `tantra-exec-paper` engine;
> zero-I/O, Decimal, wasm32 check green), `tests/golden/paper_golden.json` U2
> oracle (4 fills, sell VWAP 100.6667, net 0, cash 1 000 000, 7 events),
> `AppRoute::Paper` + CSR `PaperView`/`PaperFeature` (embedded `PaperSession`
> demo + server-mode flag), TUI `/positions` panel on `/ws/live`
> `ClientEvent`s (`live_data.rs` paper projection; offline fixture fallback),
> `tantra paper --mode embedded|server` (CLI, deterministic golden tape).
> Tests: api-wasm +6, app +4 (9 routes), tui +3, cli +4. Workspace gate green
> (check/test/clippy `-D warnings`/fmt/wasm32/`make prod`). Plan 0008
> Implemented + index updated; doc retained (root plan 0007 references it).
> **Next: P4 golden parity** — cross-validate the Python engine against
> `paper_golden.json` (Rust native ↔ WASM ↔ Python) + root plan 0007 T14.15
> paper-trading user guide (with `tantra-docs`).
>
> Updated: 2026-11-15 — **Track T10 complete** (`19b8acd` + `d60d840`):
> T10.1 real CLI reports (`memory`/`plugin`/`skills`/`mcp`/`doctor`/`purge`/`health`
> over real `~/.tantra` stores, `memory_store.rs` JSONL journal, hermetic
> `*_with_home` variants), T10.2 Leptos 0.8 CSR view layer (`tantra-ui-app/src/web.rs`,
> 8 route views, WASM check green — pre-existing `tantra-ui-core` wasm `std::fs`
> break fixed), T10.3 TUI real panels (`live_data.rs` `LiveData` + pure
> `ClientEvent` fold, real Watchlist/Screener/Portfolio/Orders rendering,
> `TuiApp::publish` seam), T10.4 CLI `scan --server` live `GET /screener/scan`
> (fixture = offline fallback only), T10.5 plan 0003 (`block_render.rs` ANSI
> result blocks, `tantra tools search|describe|call` via `tantra-agent-mcp`
> `McpServer`, `tantra mcp config --client ...`, `ResultBlockWidget` ratatui
> widget). Gaps 0003–0006 closed; plan 0003 Implemented; `tantra-ui` open gaps
> 5 → 1 (0007 EE+ charts). Workspace verification green: fmt, clippy `-D
> warnings`, all crate test suites (cli 79, tui 33, app 59, core 44, api-wasm
> 37, charts 33, assets 17, highlight 13, client 7), WASM check, `make prod`.
> Known limitation: `cargo test --workspace` OOMs linking the `tantra-ui`
> facade's test binary under prod LTO (the facade has no tests; run suites
> per-crate or `--workspace --exclude tantra-ui`).
>
> 2026-11-13 — docs batch: former `docs/roadmap/roadmap.md` dismantled into
> `docs/design/` (`architecture.md` rewritten source-verified + `implementation-record.md` +
> per-crate `code-*.md` for all 9 crates + `code-migration-map.md`), `docs/gaps/` gained four
> source-verified open gaps (0003–0006), `docs/plans/README.md` corrected (plan 0005 SHAs
> rewritten out of history; plan 0001 "blocked upstream" claim stale — Ask/Chat/Tui wired).
> No Rust code touched. TUI `run()` event loop is DONE (`terminal.rs`).
>
> 2026-11-13 (second batch): new **plan 0007** + design doc for the **Agent Desk UI Wiring &
> Winning-Trade Flow** — 14-persona roster + lenses (`--persona`/`/agent` + new `--lens`/`/lens`/
> `/desk`/`/trade`) across CLI/TUI/web, and the risk-gated screen → desk → trade-plan → place →
> monitor → journal journey. Docs/design/plan only; depends on `tantra-agent` plan 0012 (persona
> roster) and plan 0003 (result blocks).

## 1. Phase / Slice Status
- **PLAN-0001 STAGE 4 COMPLETE** (`tantra-runtime/docs/plans/0001`, CLI half —
  runtime Stages 1–3 landed in the `tantra-runtime` repo):
  - `tantra-cli/src/jobs_runtime.rs`: `JobsRuntime` — `tantra jobs
    add/list/run/tick/pause/resume/approvals/approve/deny/logs` wired to
    `BoundedWorkerPool` + `FileJobRegistry` + `FileJournalWriter` (path deps on
    `tantra-runtime-jobs` / `-api` / `-api-rest`, **native binaries only**; must
    never leak into WASM modules).
  - Job-run journaling: per-run journal dir `journals/<run_id>/` with
    `manifest.json` (`kind: "job"`), `events.ndjson` (`run.queued`, `run.started`,
    `run.result|run.failed`, `run.finalized`), `trades.json`, `metrics.json`.
    Budget-429 refusals mint no run id and write no journal.
  - Agent tool calls journal: `StrategyBridgeRunner` (`kind: "tool"`,
    `tool.started` + `tool.result|tool.failed`) — best-effort, never breaks tool
    results; `jobs add --approval auto|manual` gates breaches into the approvals
    queue (`record_usage` breach/manual → `PendingApproval`, `deny` → Paused,
    `approve` → re-activates).
  - CLI args: `--approval/--strategy/--symbol/--capital/--params` on
    `tantra jobs add`; boot recovery wired in `main.rs`
    (`jobs_runtime::boot_recover`, prints recovered-run count unless quiet).
  - **Acceptance roundtrip proven** in
    `crates/tantra-cli/tests/jobs_runtime_test.rs` (11 tests): add → tick →
    journal verified; once-jobs, failed-strategy journals, all five refusal
    classes, budget-breach → deny → Paused, manual approval gating, saturated
    pool backpressure, agent-tool journaling, boot recovery, legacy
    `jobs.json` migration, `resolve_job` prefix matching.
- **TIER 5 / LAYER 5 COMPLETE** (earlier work, see git history): nine crates;
  seven-tab TUI workstation; feature state models; string-decimal wire contracts.
- Verification gate:
  - `cargo clippy --workspace --all-targets --profile prod -- -D warnings` ✅
  - `cargo test --workspace --profile prod` ✅ as of Stage 4: 59 targets, 275 passed
    (`tantra-cli` unit tests 33 incl. 7 scan tests; `tantra-ui-core` strategy
    primitives 9 incl. 3 trade-row tests); `cargo test -p tantra-cli --profile prod` ✅
    34 passed after the `tantra serve` batch (added `parse_cli_roundtrip_serve`)
  - `cargo fmt` ✅ on all Stage 4 touched files (pre-existing drift in
    `tantra-tui` / `tantra-ui-core` committed files predates this batch and is
    out of scope here)
  - `make prod` ✅ fresh `target/prod`

## 2. Decisions & Discoveries
- CLI `ask`/`chat`/`tui` wired to LLM backends (GGUF in-process / OpenAI
  endpoints); **`scan` closed** — wired to canonical screener via
  `crates/tantra-cli/src/scan.rs` + `runner.rs`
  (`CommandOutcome::ScanResults`; tests in `crates/tantra-cli/tests/unit_tests.rs`).
- **`trades.json` closed** — job runs now journal per-trade rows (serialized
  `BacktestReport.trades`) instead of `{"total_trades": N}`; `BacktestReport`
  carries `trades: Vec<TradeRow>` emitted by `simulate_strategy`
  (tests in `crates/tantra-ui-core/tests/strategy_primitives_test.rs`).
- Registry ops order per run: claim → pool dispatch → release_lease →
  record_usage (breach/manual wins over lease) → advance cron cursor; cron
  evaluation via the runtime `JobScheduler` (`cron` crate, Vixie dom/dow OR).
- Determinism: all job/manifest timestamps are explicit `DateTime<Utc>` passed
  from the caller; journal events carry only seq + type + engine output.
- Zero `#[cfg(test)]` inside `crates/*/src/`; tests live in `<crate>/tests/`.
- **`tantra serve` added** — `tantra-runtime-api-rest::bootstrap::{ServeConfig,
  run_server_blocking}` was a complete, tested library with no binary ever calling it
  (same gap existed in `honba-ya`); `tantra-cli` already depended on the crate. Added
  `Command::Serve(ServeArgs)` (`args.rs`), `run_serve` (`runner.rs`) mapping CLI flags
  (`data_dir` positional, `--addr`, `--cors-origin`, `--profiles-dir`, `--portfolios-dir`,
  `--journals-dir`, `--tokens-file`) onto `ServeConfig`, and the `ServeSuccess` outcome
  (`main.rs`). Smoke-tested against `tantra-data/data` (empty catalog): binds, serves
  `GET /profile` over real HTTP, shuts down cleanly on signal. `tantra-ui-web`'s
  `settings`/`alerts` stores are being wired to the now-reachable `/profile` API in the
  same batch (see that repo's own HANDOFF/gaps); `watchlists`/`portfolios`/
  `risk-profiles`/`workspaces` don't fit the current `Profile` schema — tracked as
  `tantra-runtime/docs/gaps/0004`.

## 3. Immediate Next Steps
1. **P4 golden parity LANDED (root plan 0007 T14.14)**: `tests/golden/paper_golden.json`
   is now the single canonical vector, byte-identical across the paper engine
   (`tantra-exec-paper`), this WASM embedding, and the Python wheel
   (`tantra-ffi` pins the identity in `python/tests/test_paper_golden_identity.py`);
   `golden_parity_test.rs` reads the canonical schema. Next: the paper-trading
   user guide (T14.15, coordinated with `tantra-docs`).
2. **EE+ Strategy Studio — DONE (plan 0006 Phases 2–4, T12.9 remainder)**: the
   shared `tantra-ui-core::ee` view-model helpers now power the web Studio
   (`StudioEeFeature`/`StudioView` EE sections), the TUI EE+ Positioning
   section (Portfolio tab), and `tantra elephant`. Remaining when the upstream
   REST endpoints land: swap the seeded fixtures for live
   `GET /level-map/{date}` / `GET /signals/{date}` data (drop the `unavailable`
   note) and wire live `StopUpdate`s into the stop lines (plan 0006 §1.2).
3. **TUI live-feed consumer**: `TuiApp::publish` is wired but nothing drains an
   `EventStream` into it yet — a tokio feed task (over `tantra-runtime-api-rest`
   `GET /quotes`/`/portfolio/*`/`/orders` or a market feed) would light up the
   four real panels without touching the render path.
4. **Agent Desk UI** (plan 0007, proposed): depends on `tantra-agent` plan 0012
   (persona roster) + plan 0003 result blocks (now available).

## 4. Operating Invariants
- Zero `#[cfg(test)]` / `#[cfg(test)] mod tests` inside `crates/*/src/`.
- Zero floats: all financial math is `rust_decimal::Decimal`.
- WASM crates (`ui-core`, `ui-assets`, `ui-charts`, `ui-client`, `ui-api-wasm`,
  `ui-app`) must compile to `wasm32-unknown-unknown`; no `tokio` there. The
  `tantra-runtime-*` path deps are CLI-binary-only.
- Tests live in `<crate>/tests/` parallel to `src/`.
- Edition 2024, rust-version 1.85, AGPL-3.0.
