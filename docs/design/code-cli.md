# `tantra-cli` — The `tantra` Binary & Command Dispatcher

Native (`clap`, `tokio`). Deps include `clap`, `toml`, `tokio`, `futures`, `async-trait`, the
`tantra-agent-*` LLM/tool crates, and — for `Serve`/`Jobs` — `tantra-runtime-jobs`,
`tantra-runtime-api`, and `tantra-runtime-api-rest` (native binaries only; never leak into WASM).

## Modules

| Module | Contents |
| :--- | :--- |
| `args.rs` | `CliArgs`, `CliCommand` (clap), and the **15-variant `Command` enum**: `Tui`, `Ask`, `Chat`, `Scan`, `Serve`, `Jobs`, `Verify`, `Memory`, `Plugin`, `Skills`, `Mcp`, `Doctor`, `Purge`, `Health`, `Strategy`. Plus `JobsArgs` (11 subcommands: Add/List/Run/Tick/Install/Logs/Pause/Resume/Approvals/Approve/Deny), `StrategyArgs` (List/Run/Tune/Suggest), `LogLevel`, `OutputFormat`. |
| `main.rs` | `boot_recover` sweep at startup; dispatches `CommandOutcome`; `LaunchedTui` builds a current-thread tokio runtime and `rt.block_on(tantra_tui::run_terminal(app, provider))`. |
| `runner.rs` | `run`/`run_with_provider`, `CommandOutcome` (LaunchedTui, Answer, ChatCompleted, ScanResults, JobsSuccess, VerifySuccess, MemorySuccess, PluginSuccess, SkillsSuccess, McpSuccess, DoctorReport, PurgeSuccess, HealthReport, StrategySuccess, ServeSuccess), `resolve_provider`, `tantra_home_dir`, `CliError`. |
| `scan.rs` | `parse_query`, `run_scan`, `scan_universe` — parses `"metric op value"` / `"metric between low high"` / `"a and b"` into `tantra_core::screener::ScreenerScanRequest` (`FilterOp::{Gt,Gte,Lt,Lte,Eq,Ne,Between}`, `match_all=true`), evaluates via `tantra_ui_api_wasm::screener::apply_scan` against an in-memory fixture universe (RELIANCE, TCS, INFY, HDFCBANK) — `Decimal`-only. Header comment: "gap 0001 closed; gap doc removed". The fixture universe (vs a live `GET /screener/scan`) is open gap `docs/gaps/0006`. |
| `jobs_runtime.rs` | `JobsRuntime::open/registry/resolve_job/add/run/tick/pause/resume/approvals/approve/deny/journal_runs`, `AddRequest`, `RunReceipt`, `RunDisposition`, `TickReport`, `JobRunRecord`, `mint_run_id`, `short_id`, `boot_recover`. `run()` implements ADR-0017 fencing: claim lease → `BoundedWorkerPool.dispatch` (429 backpressure) → `FileJournalWriter` per-run journal dir (`manifest.json` `kind:"job"`, `events.ndjson` run.queued/started/result\|failed/finalized, `trades.json`, `metrics.json`) → release lease → record budget usage → advance cron cursor. `trades.json` serializes the `report.trades` row list (gap 0002 closed). |
| `backtest_gen.rs` | Natural-language backtest-query interception inside `run_ask` (only when the prompt is not a strategy workflow): parses "backtest 10L on Alpha30 stocks; with 15 day rebalancing" → generates a Python script (`generate_python_script`), executes with `python3` (spawns a subprocess), scrapes `__TANTRA_METRICS_JSON__=` from stdout, renders a tearsheet box; falls back to canned metrics if the script fails. |
| `strategy_bridge.rs` | Re-exports `StrategyBridge`/`MarketBar`/`TradeSuggestion`; `StrategyBridgeRunner` implements `tantra_agent_chat_tools::ports::strategy::StrategyRunner` (`run_backtest`, `tune_strategy`, `generate_trade_suggestion`) journaling every tool call as a `kind:"tool"` run; `render_suggestion_card` renders the 2R trade-suggestion card. |

## Wiring status (verified)

**Real / wired**: `Tui` (launches `run_terminal`), `Ask` (real LLM streaming + strategy tool
calls), `Chat` (interactive LLM REPL), `Scan` (parses → `apply_scan` over fixture universe),
`Jobs` (real `JobsRuntime`), `Verify` (static lookahead/leakage line scanner, exit codes 0/1/2),
`Strategy` (discover/run_backtest/tune/generate_suggestion), `Serve` (boots
`tantra_runtime_api_rest` and blocks).

**Stubbed / canned**: `Memory`, `Plugin`, `Skills`, `Mcp`, `Doctor`, `Purge`, `Health` return
hardcoded string reports (e.g. `run_health` prints a canned healthy line). `Doctor --fix` and
`Purge --yes` do real filesystem work; the *reports* are canned. Open gap `docs/gaps/0003`.

## Tests

`tests/unit_tests.rs`, `tests/strategy_cli_test.rs`, `tests/jobs_runtime_test.rs`
(57 `#[test]` functions).
