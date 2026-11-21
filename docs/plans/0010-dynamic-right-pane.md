# 0010 — Dynamic right pane (query-driven panes + agent present_view)

**Status**: Implemented 2026-11-21

## 0. Executive Summary & Objective

The right half of the Agent view shows whatever fits the user's last action instead of a fixed
Analysis page: `/help` shows help, informational slash commands (`/mcp`, `/memory`, ...) show
their output there instead of in the chat, `/backtest` / `/loop-eval` / `/ee-study` / `/hedge`
show their views, an LLM analysis query shows the Analysis page, and when the agent calls the
`present_view` tool the pane switches to the agent-composed view. Several panes are kept as tabs.

## 1. Context & Gaps Covered

- Agent plan `0020` (`present_view` tool, `DisplayBlock::View`, `PRESENT_VIEW_GUIDANCE`).
- Agent plan `0019` Phase 3 (the TUI switching to `AcpClient`) is still pending and is the next
  step; the embedded agent in `terminal.rs` registers `create_presentation_tools()` meanwhile.

## 2. Component Specifications & Phases

- `src/pane.rs` (pure): `PaneBody`, `AgentView`, `Pane`, `PaneStack` (cap 8, dedup by identity,
  eviction of the oldest non-active pane, wrapping next/prev), `view_kind_label`, `view_cards`.
- `ToolCard.turn`; `AnalysisPage::push_blocks(tool, turn, blocks)`; `TuiApp.panes` / `TuiApp.turn`.
- Routing in `execute_slash_command`: `/help [cmd]` opens Help; `INFO_PANE_COMMANDS` output
  (System messages) is drained from chat into an Info pane; strategy/backtest/positions commands
  open their pane; new `/pane [list|next|prev|close]`.
- `ToolFinished` blocks: each `DisplayBlock::View` opens a View pane (not a card); the rest become
  cards tagged with the turn. `submit_prompt` opens the Analysis pane.
- `src/pane_view.rs`: tab strip + body renderers (reusing analysis, strategy lab, backtest,
  portfolio renderers). Keys: Alt+Left/Right switch, Alt+x close, PgUp/PgDn scroll the active pane.
- Welcome screen only when chat and panes are both empty.
