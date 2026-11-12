# 0003 — MCP Gateway UI/CLI Integration, Result Block Rendering, and Client Configs

> **Status**: Ready for implementation.
> **Target Crates**: `tantra-cli`, `tantra-tui`, `tantra-highlight`.
> **Depends on reading**:
> - `tantra-ui/docs/plans/0001-agent-chat-terminal-from-artha.md`
> - `tantra-ui/docs/plans/0002-quantcept-web-ux-reference.md`
> - `tantra-agent/docs/plans/0001-mcp-gateway-discovery-and-quantitative-engines.md`

---

## 0. Executive Summary & Objective

With `tantra-agent` establishing a two-tier discovery gateway, universal `tantra/result/v1` envelope, and 15 quantitative engines, `tantra-ui` must deliver the presentation and operator interface for these tools:
1. **Visual Presentation Block Rendering**: Turning `DisplayBlock`s (`Table`, `MetricGrid`, `Chart`/sparkline, `Alert`) into ANSI-colored terminal output in `tantra-cli` and Ratatui widgets in `tantra-tui`.
2. **Tool Gateway CLI Commands**: Providing power users with direct inspection tools (`tantra tools search`, `tantra tools describe`, `tantra tools call`).
3. **One-Click MCP Client Config Exporters**: Generating ready-to-run configuration snippets for external coding agents (Claude Code, Cursor, Copilot/VS Code, Codex) that connect directly to the local `tantra-cli mcp` stdio daemon.

---

## 1. UI & Terminal Interaction Architecture

```
┌────────────────────────────────────────────────────────────────────────┐
│                          tantra-ui Surface                            │
├───────────────────────────────────┬────────────────────────────────────┤
│         tantra-cli               │            tantra-tui             │
│                                   │                                    │
│  • tantra tools search           │  • AgentChat Tab                   │
│  • tantra tools describe         │    - Streaming token markdown      │
│  • tantra tools call             │    - Collapsible Tool Execution    │
│  • tantra mcp config             │    - Embedded Ratatui Tables       │
│  • tantra ask --persona <p>      │    - Unicode Sparkline Bar Charts  │
└─────────────────┬─────────────────┴──────────────────┬─────────────────┘
                  │                                    │
┌─────────────────▼────────────────────────────────────▼─────────────────┐
│                     Block Presentation Engine                          │
│                                                                        │
│   • TableRenderer: ANSI borders, aligned headers, zero-float decimals  │
│   • MetricGridRenderer: Side-by-side KPI boxes with delta colors       │
│   • SparklineRenderer: Unicode block characters ( ▂▃▄▅▆▇█)             │
│   • AlertRenderer: Styled badges ([INFO], [WARN], [CRITICAL])          │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │ Parses
┌───────────────────────────────────▼────────────────────────────────────┐
│              Universal Result Protocol (tantra/result/v1)             │
│                Emitted by tantra-agent-chat / MCP                     │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Component Specifications

### 2.1 Result Envelope Block Renderers

Tools returning `tantra/result/v1` contain an optional `blocks` array. `tantra-ui` renders each block type cleanly:

#### 1. `DisplayBlock::Table`
- **CLI**: Rendered using UTF-8 box-drawing characters (`┌─┬─┐`, `│ │ │`, `└─┴─┘`), with right-aligned numeric columns.
- **TUI**: Rendered inside `ratatui::widgets::Table` with selectable row highlight, scrollbar, and fixed header.

#### 2. `DisplayBlock::MetricGrid`
- Renders KPI cards (e.g., Sharpe Ratio: `1.84`, Max Drawdown: `-8.20%`, Annualized Return: `28.4%`).
- Value coloring: Green for positive return/Sharpe, Red for drawdown/VaR breach, Cyan for neutral.

#### 3. `DisplayBlock::Chart`
- **CLI**: Renders high-density Unicode sparklines (` ▂▃▅▆▇█`) for quick 1-line trend visualization.
- **TUI**: Renders `ratatui::widgets::Sparkline` or multi-series `ratatui::widgets::Chart` directly inside the AgentChat turn view.

#### 4. `DisplayBlock::Alert`
- Styled warning boxes for risk limit warnings, margin shortfall alerts, or statutory cost notices.

---

### 2.2 CLI Tool Gateway Commands

Add a new top-level `tools` command family to `tantra-cli`:

```bash
# 1. Search across all 15 engines
tantra tools search "GARCH volatility"
# Output:
#   Found 2 matching tools:
#   - tantra_volatility_fit_garch (volatility): Estimate GARCH(1,1)/EGARCH models
#   - tantra_volatility_forecast_var (volatility): Value-at-Risk & ES out-of-sample forecast

# 2. Inspect exact schema and usage
tantra tools describe tantra_volatility_fit_garch
# Output:
#   Tool: tantra_volatility_fit_garch
#   Category: volatility
#   Description: Fits univariate GARCH-family conditional volatility models...
#   Parameters:
#     - series_ref (string, required): Ref ID ($tantra/ref/...) or inline array
#     - model (enum: "garch", "egarch", "gjr"): Default "garch"
#     - p (integer): Default 1
#     - q (integer): Default 1

# 3. Direct invocation
tantra tools call tantra_calc_statutory_costs '{"segment":"equity_delivery","turnover":"100000","side":"sell"}'
```

---

### 2.3 One-Click MCP Client Configuration Exporter

Coding agents (Claude Code, Cursor, Codex, VS Code) require an MCP config file pointing to the server. Add `tantra mcp config`:

```bash
# Generate configuration for Claude Code
tantra mcp config --client claude-code

# Generate configuration for Cursor
tantra mcp config --client cursor

# Generate configuration for VS Code (GitHub Copilot Agent mode)
tantra mcp config --client vscode
```

#### Generated Cursor Configuration (`.cursor/mcp.json`):
```json
{
  "mcpServers": {
    "tantra": {
      "command": "tantra",
      "args": ["mcp", "--stdio"],
      "env": {
        "TANTRA_LOG": "info",
        "TANTRA_WORKSPACE": "/path/to/workspace"
      }
    }
  }
}
```

#### Generated Claude Code Configuration (`~/.claude.json`):
```json
{
  "mcpServers": {
    "tantra": {
      "command": "tantra",
      "args": ["mcp", "--stdio"]
    }
  }
}
```

---

## 3. Implementation Phases

### Phase 1: Block Presentation Engine (`crates/tantra-tui` & `crates/tantra-cli`)
- Create `block_render.rs` in `tantra-cli` implementing ANSI table, metric, and sparkline rendering.
- Add `ResultBlockWidget` in `crates/tantra-tui/src/widgets/` for in-TUI rendering.

### Phase 2: Gateway CLI Commands (`crates/tantra-cli/src/lib.rs`)
- Add `Command::Tools { action: ToolsAction }` with subcommands `search`, `describe`, and `call`.
- Connect calls to `tantra-agent-mcp` gateway services.

### Phase 3: MCP Config Generator (`crates/tantra-cli/src/mcp_config.rs`)
- Implement `Command::Mcp { subcommand: McpCommand::Config { client } }`.
- Validate generated paths and permissions.

### Phase 4: Full End-to-End Turn Test
- Unit test verifying `tantra ask` correctly formats and prints nested result envelope blocks.
- Unit test verifying `tantra tools search` and `tantra tools describe` output.
