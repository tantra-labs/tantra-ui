# व्यापार・界面 tantra-ui

> *Unified Workstations: WASM/Native Shell Strategy Studio, Screener, Ratatui Terminal UI, CLI binary, and syntax highlighting.*

Trader and quantitative workstation interfaces across terminal and browser.

## Subcrates

- `tantra-ui` — Workspace facade
- `tantra-ui-core` — Common UI state, theme definitions, and view models
- `tantra-ui-app` — Desktop & WASM canvas strategy studio
- `tantra-ui-charts` — High-performance Rust financial charting primitives
- `tantra-ui-client` — WebSocket & REST client connection to `tantra-runtime`
- `tantra-ui-assets` — Embedded fonts, icons, and stylesheets
- `tantra-ui-api-wasm` — WASM bridge bindings for web integration
- `tantra-tui` — Ratatui-based terminal dashboard with interactive LLM Agent chat
- `tantra-cli` — Command line interface (`tantra` CLI) with one-shot ask & interactive chat
- `tantra-highlight` — Zero-I/O WASM-safe syntax highlighter (hand-rolled lexer, no tree-sitter)

## LLM provider config (CLI & TUI)

`tantra ask`, `tantra chat` and `tantra tui` read their LLM providers from a `tantra.jsonc`
(project file found by walking up from cwd, merged over `~/.config/tantra/tantra.jsonc`):

```jsonc
{
  "model": "local/qwen3-30b-a3b",
  "provider": {
    "local": {
      "options": { "baseURL": "http://127.0.0.1:8080/v1" },
      "models": { "qwen3-30b-a3b": {} }
    }
  }
}
```

```bash
tantra ask "Explain VWAP vs TWAP"
tantra chat --model local/qwen3-30b-a3b
tantra tui --config ./tantra.jsonc
```

`--model` takes a `provider/model` catalog id; `--config` points at an explicit file. See
`tantra-agent/docs/design/provider-config.md` for the full schema.

## Build & Test

```bash
make prod
make check
make test
```

License: AGPL-3.0.
