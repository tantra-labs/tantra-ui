# 0003 — CLI `memory`/`plugin`/`skills`/`mcp`/`doctor`/`purge`/`health` commands return canned string reports

**Status**: open

## What's stubbed or hardcoded

`crates/tantra-cli/src/runner.rs::run_with_provider` dispatches the full 15-variant `Command`
enum, but seven variants return hardcoded string reports rather than real functionality:

- `Memory`, `Plugin`, `Skills`, `Mcp`, `Doctor`, `Purge`, `Health` all produce canned output
  (e.g. `run_health` prints `"tantra: healthy (tui ready, ask/chat wired to Candle GGUF, scan
  pending universe)"` — itself stale, since `scan` is wired).
- `Doctor --fix` and `Purge --yes` do perform real filesystem work (create/delete directories),
  but the reports they print are canned.

## Why

These commands were stubbed to complete the CLI surface for plan 0001 without depending on
upstream capabilities that were not yet available (persistent memory, plugin/marketplace
infrastructure, the MCP gateway, the `tantra-agent` skills system). The `Mcp` command's real
target — a stdio MCP daemon — is specified in `tantra-agent/docs/plans/0001` and
`tantra-ui/docs/plans/0003`.

## How to close it

- Wire `Memory` to real memory-store operations (blocked on `tantra-agent` persistent memory,
  `tantra-agent/docs/plans/0009`).
- Wire `Plugin`/`Skills`/`Mcp` to the `tantra-agent` plugin registry / MCP gateway once those
  land (`tantra-agent` plan 0003; the MCP *client config exporter* half is `tantra-ui`
  plan 0003 Phase 3).
- Wire `Doctor` to a real environment check (broker auth, DB connectivity, model files present)
  and `Purge` to real cleanup with a confirmation flow.
- Update the `run_health` canned string's `scan` status once any of these land.
