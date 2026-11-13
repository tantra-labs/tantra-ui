# Plans Index — `tantra-ui`

Per workspace rule `.agents/rules/gap-tracking.md`:
- Gaps are individual features (`docs/gaps/XXXX-*.md`).
- Plans are collections of gaps (`docs/plans/XXXX-*.md`).
- When implemented, plan documents are cleaned up from `docs/plans/` and recorded in this index.

## Active Plans

| # | Title | Target Scope | Status |
|---|-------|--------------|--------|
| [0002](0002-quantcept-web-ux-reference.md) | UX reference for the `AgentChat` tab, captured from a live product session | `tantra-ui` | Live product UX reference |
| [0003](0003-mcp-gateway-result-rendering-and-client-configs.md) | MCP Gateway UI/CLI Integration, Result Block Rendering, and Client Configs | `tantra-cli`, `tantra-tui` | Ready for implementation |

## Implemented Plans

| # | Title | Target Scope | Implemented In |
|---|-------|--------------|----------------|
| 0001 | Building `tantra-cli` / `tantra-tui` / `tantra-highlight` from the `artha` design work | `tantra-cli`, `tantra-tui` | Implemented — crate build-out complete (`tantra-highlight` lexer, `tantra-tui` `TuiApp`/`ActiveTab`/`run_terminal`, `tantra-cli` `CliArgs`/`Command` + runner). `Ask`/`Chat`/`Tui` are wired to real LLM providers (`CandleGgufProvider` in-process, OpenAI-compatible endpoints) via `tantra-agent-llm`; CLI `scan` closed via gap 0001. Doc retained because plans `0002`/`0003` list it as a "Depends on reading" reference. |
| 0004 | Full CLI Command Reference & Interactive TUI Slash Command Palette | `tantra-cli`, `tantra-tui`, `tantra-highlight` | Implemented (Definitive reference: `docs/reference/cli-and-tui.md`) |
| 0005 | FinceptTerminal's Screen Inventory vs. tantra-ui's Roadmap | `tantra-ui-core`, `tantra-ui-app`, `tantra-tui` | Implemented — Watchlist, Screener, Backtest, Algos, Portfolio features in `tantra-ui-app` + the 7-tab TUI workstation (commits `89f78ce` features, `a0b7129`+`74fc304`+`dd1e5ef` TUI/CLI phases; the SHAs `2cda245`/`acdc137` previously cited predate the `vyapaar-*` → `tantra-*` history rewrite and no longer exist in this repo's log) |
