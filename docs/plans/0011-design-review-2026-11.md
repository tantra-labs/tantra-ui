# 0011 — tantra-ui design review (2026-11): honest data, thin-client CLI, robust TUI & distribution

**Status**: Proposed (2026-11-23)
**Scope**: `tantra-ui` · **Umbrella**: root `docs/plans/0021-workspace-design-review-2026-11.md`

## 0. Executive Summary & Objective

The L5 repo ships the user's daily server (`tantra-ui/target/prod/tantra serve ...`), so it is both a client and an unintended runtime release gate. The review found: synthetic sine-wave bars presented as strategy results (`crates/tantra-ui-core/src/strategy.rs:798`, `Utc::now()` at `:849,904,948,1122,1345`); `tantra serve` exposing about 7 of `ServeConfig`'s knobs; a TUI that never restores the terminal on panic; CWD-relative server state; and a handful of duplication and distribution gaps.
This plan closes four gaps and schedules nine plan-only items (R1-R9) in three phases. The provider-side gap for the serve composition root is `tantra-runtime/docs/gaps/0026-single-composition-root-serve.md`.
Non-goals: module moves in `scene_projector.rs`/`app.rs` splits and the runtimes-only items owned by root plan 0019 (P3.1, P3.5, P5.3); web (`tantra-ui-web`) fixes.

## 1. Context & Gaps Covered

| Gap | Title | Kind | Severity |
|---|---|---|---|
| [0008](../gaps/0008-synthetic-bars-in-run-tune-suggest.md) | Synthetic sine-wave bars drive strategy run/tune/suggest | design-flaw | high |
| [0009](../gaps/0009-tui-panic-guard-and-async-loop.md) | TUI never restores the terminal on panic; input blocks on network awaits | design-flaw | medium |
| [0010](../gaps/0010-state-dir-and-boot-recover.md) | Serve state defaults CWD-relative; recovery sweep on every invocation | deployment | medium |
| [0011](../gaps/0011-ui-core-purity.md) | `tantra-ui-core` not pure: fs, clock, floats | design-flaw | medium |

Plan-only items (no standalone gap doc):

- R1 — `tantra serve` is a partial second composition root — `crates/tantra-cli/src/args.rs:218-239`.
- R2 — L5 binary links the whole backend; no client-only build — `crates/tantra-cli/Cargo.toml:15-37`.
- R3 — Two HTTP client stacks; hand-built wire JSON — `crates/tantra-cli/src/scan.rs:185-214`.
- R4 — `serve` daemon basics missing — `crates/tantra-cli/src/main.rs`, `crates/tantra-cli/src/runner.rs:846-856`.
- R5 — Result-block rendering duplicated across surfaces — `crates/tantra-cli/src/block_render.rs:34-70`.
- R6 — Slash-command registry vs clap subcommands — `crates/tantra-tui/src/app.rs:236`.
- R7 — CLI machine-readable/exit-code contract incomplete — `crates/tantra-cli/src/main.rs:110-121`.
- R8 — No completions, man page or build info — `crates/tantra-cli/src/args.rs:592`.
- R9 — Offline fixture and help-text fallbacks instead of `Unavailable` — `crates/tantra-cli/src/scan.rs:281-311`.

## 2. Component Specifications & Phases

### Phase A — Honesty and terminal safety (quick, low risk)

1. Gap 0008: route run/tune/suggest through `tantra-runtime-client` backtests or an explicit `bars` argument; move the generator to `tests/`; inject `now`.
2. Gap 0009: `TerminalGuard` + panic hook; spawn network awaits; `EventStream` in `tokio::select!`.
3. Gap 0011: `Decimal::from_str` parsing in `backtest_gen.rs`; `StrategySource` port; wasm32 CI check.
4. Gap 0010: absolute `$TANTRA_HOME/state` default; error when no home resolves; `boot_recover` only for `jobs`/`tools`.
5. R9 — `crates/tantra-cli/src/paper.rs:92-110` `--mode server` only prints an instruction string naming a `tantra paper serve` that does not exist in `Command`; `crates/tantra-cli/src/scan.rs:281-311` silently uses a 4-scrip fixture universe when no `--server` or config runtime is present (`study_loop.rs:615-617` has the same fallback). Why: a mistyped or absent server config yields plausible fixture output. Close: fixtures only with explicit `--offline`/`--demo`, otherwise an `Unavailable`-style error (exit 3); tag `--json` results `source: fixture`; cite root gap 0012.

**Gate**: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `make check`, `cargo test -p tantra-ui-core --profile prod`, `cargo test -p tantra-tui --profile prod`, `cargo test -p tantra-cli --profile prod`, `make prod`.

### Phase B — Structural refactors and de-duplication

6. R1 — Evidence: `crates/tantra-cli/src/runner.rs:655-676` maps only data_dir, addr, cors_origin, three dirs and tokens_file onto `ServeConfig`; JWT auth, risk-limit file, fill model, ingest, runs limits and trading state in `tantra-runtime/crates/tantra-runtime-api-rest/src/bootstrap.rs:163-279` are unreachable. A second root exists at `tantra-runtime/crates/tantra-runtime-api-rest/src/bin/paper_serve.rs:1-60`. Why: JWT-only fail-closed auth (`.agents/rules/security-and-secrets.md` §1) cannot be deployed via `tantra serve`. Close: demote `serve` to an optional cargo feature delegating to a runtime-provided binary with one shared `ServeConfig` loader (flag > env > `tantra.jsonc`), per `tantra-runtime/docs/gaps/0026-single-composition-root-serve.md`; fix the `tantra paper serve` text in `crates/tantra-cli/src/args.rs:61`, `crates/tantra-cli/src/paper.rs:10,92,107`.
7. R2 — Evidence: 17 internal path deps incl. `tantra-agent-mcp` via a raw relative path bypassing `[workspace.dependencies]`; `tantra-tui` pulls agent-chat/llm crates; binary is 38 MB fat-LTO. Why: a laptop client compiles the whole backend and every runtime change rebuilds L5. Close: features `serve`, `local-agent`, `paper-embedded`, default = client; `tantra-agent-mcp` as workspace dep; CI job `--no-default-features`.
8. R3 — Evidence: `crates/tantra-tui/Cargo.toml` direct `reqwest`; `DEFAULT_RUNTIME_BASE` duplicated at `crates/tantra-tui/src/market_feed.rs:12`, `crates/tantra-cli/src/paper.rs:96`, `args.rs:222,725`; `runner.rs:220-236` `resolve_cli_runtime` not used by the TUI (`terminal.rs` `config_runtime`). Why: divergent base-URL/token/timeout resolution and hand-built screener JSON that drifts from OpenAPI (root gap 0022). Close: one `RuntimeEndpoint` resolver; typed `screener_scan`/`strategy_loop_*` in `tantra-runtime-client`; drop direct reqwest from cli/tui; rename `tantra-ui-client` to `tantra-ui-events`.
9. R5 — Evidence: `block_render.rs:34-70`, `crates/tantra-tui/src/result_block.rs:22-77`, `ui.rs:164-174` each match `DisplayBlock` and re-implement padding and numeric detection. Why: a new variant needs three edits; INR/percent formatting diverges. Close: pure `block_layout.rs` in `tantra-ui-core` (`DisplayBlock -> styled cells`, `money_inr`, `pct`, `qty`), exhaustive `match`, shared golden snapshots.
10. R6 — Evidence: `app.rs:236` slash table and `:1028` dispatch vs `args.rs:470-530` `Command` and `runner.rs:254`, plus `docs/reference/cli-and-tui.md`. Why: drift (for example `--json` only on some commands). Close: one `CommandSpec` table in `tantra-ui-core` generating clap, palette and the reference doc; parity test.

**Gate**: Phase A gate plus `cargo check -p tantra-cli --no-default-features` and `cargo check -p tantra-ui-core --target wasm32-unknown-unknown`.

### Phase C — Deployment readiness

11. R4 — Evidence: `println!("listening on ...")` in `bootstrap.rs:~600`; no tracing subscriber in `main.rs`; `--log-level` falls back to legacy `QUANTCEPT_LOG_LEVEL` (`args.rs:558-575`); `stop_signal` handles SIGINT/SIGTERM only; `jobs install` prints a snippet but no serve unit exists. Why: no readiness probe, structured logs or unit file under systemd/k8s. Close: JSON `tracing` to stderr, sd_notify/`--ready-fd`, `tantra serve --print-unit systemd|launchd`, Dockerfile with HEALTHCHECK, drop the `QUANTCEPT_` fallback; provider `tantra-runtime/docs/gaps/0022-observability-health-readiness.md`.
12. R7 — Evidence: `--json` per command only (`args.rs:97,376,788`); `main.rs:110-121` prints `error: {other}` from `CliError::Execution(String)`; `runner.rs:227` reads `runtime.token` from `tantra.jsonc` with no 0600 check or zeroize. Why: scripts cannot tell usage, auth, network and domain failures apart. Close: global `--output`, `{code,kind,message}` errors, exit-code table (2 usage, 3 network, 4 auth, 5 not-found, 1 domain), typed `CliError`, `secrecy::SecretString`.
13. R8 — Evidence: `args.rs:592` bare `version`; no `clap_complete`/`clap_mangen`; no `build.rs` build info; `.github/workflows` has no release job. Why: packaging needs completions, man page and commit/schema version. Close: `tantra completions <shell>`, `tantra man`, build-info env, release workflow with static musl, macOS and Windows artefacts and checksums (root plan 0023).

**Gate**: Phase A gate plus spawn-binary integration test (READY line, SIGTERM exit 0), assert_cmd exit-code matrix, `--version` snapshot.

## 3. Relationship to Other Plans

- Root plan 0019: P3.1 (app.rs split), P3.5 (runtime construction) and P5.3 (module moves) stay there and are not duplicated.
- Root gaps 0012 (synthetic data), 0022 (OpenAPI clients); root plans 0022 (deployment) and 0023 (build/CI/release) receive R4, gap 0010 and R8.
- Provider repo: `tantra-runtime/docs/gaps/0026-single-composition-root-serve.md` (R1) and `tantra-runtime/docs/gaps/0022-observability-health-readiness.md` (R4).
- Existing plans 0006/0007 (Studio, Agent Desk) are unaffected.

## 4. Exit Criteria

1. No non-test code path in `tantra-ui-core` generates bars or calls `Utc::now()`.
2. A panic inside the TUI leaves the terminal restored (pty test).
3. `tantra serve` (if kept) delegates to the shared loader; JWT/risk/fill options are configurable.
4. Server state resolves to one absolute directory regardless of CWD.
5. `cargo check --no-default-features` builds a client-only `tantra`; wasm32 check is green.
6. Missing server config yields an error, not fixture data, unless `--offline` is given.
7. All gates in section 2 pass and `make prod` is fresh.
