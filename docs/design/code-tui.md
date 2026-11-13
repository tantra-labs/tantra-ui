# `tantra-tui` — Ratatui Terminal Workstation

Native (`ratatui` + `crossterm`). Deps include `tokio`, `futures`, `ratatui`, `crossterm` and
the `tantra-agent-*` LLM/tool crates. Not WASM-targetable by design.

## Modules

| Module | Contents |
| :--- | :--- |
| `app.rs` | `TuiApp` — full state: active_tab, should_quit, chat transcript, input buffer, palette (is_palette_open, query), undo stack, auto_approve, theme, algos registry, backtest report/equity curve/optimization result/trade suggestion. `ActiveTab` has **7 variants**: Watchlist, Screener, Portfolio, Orders, Backtest, Algos, AgentChat (`ALL`, `next()`, `previous()`, `title()`). `on_key(KeyEvent)` handles Ctrl+P palette, chat input, `/` palette trigger, Algos/Backtest hotkeys, digit 1–7 tab switching, then falls through to `handle_key` → `apply(action)`. Constructor runs a real `StrategyBridge` backtest/tune/suggest for the first discovered algo at startup. |
| `agent_chat.rs` | `ChatTranscript`/`AgentMessage`/`AgentRole` models with a `highlight()` seam calling `tantra_highlight::highlight_code(Language::Markdown, …)`. The live streaming itself lives in `terminal.rs`. |
| `keybindings.rs` | `Action`, `KeyBinding`, `handle_key`, `DEFAULT_KEYBINDINGS`. |
| `layout.rs` | `LayoutModel`. |
| `terminal.rs` | `run_terminal(app, provider)` — **fully implemented** crossterm event loop: raw mode, alternate screen, `Terminal<CrosstermBackend<Stdout>>`, `run_loop` (draw → drain tokio mpsc `AgentStreamUpdate` tokens → spawn a real `provider.chat_stream` task per pending prompt → `event::poll(30ms)` → `app.on_key`), clean teardown on quit. |
| `ui.rs` | Full ratatui rendering: header, 7-tab bar, Watchlist (hardcoded static rows), Screener/Portfolio/Orders placeholder panels, Backtest quad (KPIs + param-delta table + sparkline + suggestion card), Algos table + parameter inspector, AgentChat transcript/input, command-palette overlay. `class_to_style` maps `HighlightClass` → ratatui `Style`. |
| `lib.rs` | `SLASH_COMMANDS` (40-entry static table: `/algos`, `/backtest`, `/tune`, `/suggest`, `/help`, `/agent`, `/jobs`, `/positions`, `/mcp`, `/skills`, `/plugins`, `/settings`, `/quit`, …), `execute_slash_command` (most locally; `/backtest`/`/tune`/`/suggest` call the real `StrategyBridge`). |

## Design notes

- **7 tabs, not 4**: `docs/plans/0001` predicted `ActiveTab{Screener,Portfolio,Orders,AgentChat}`
  (4); the actual enum has 7 (Watchlist, Screener, Portfolio, Orders, Backtest, Algos, AgentChat).
- **Live LLM wiring**: Ask/Chat/TUI are wired to real providers (`CandleGgufProvider` in-process,
  OpenAI-compatible endpoints) via `provider.chat_stream` — the old "blocked upstream on
  `tantra-agent`" note in `docs/plans/0001` is stale.
- Watchlist rows are hardcoded and Screener/Portfolio/Orders are placeholder panels — open gap
  `docs/gaps/0005`.

## Tests

`tests/unit_tests.rs` (21 `#[test]` functions).
