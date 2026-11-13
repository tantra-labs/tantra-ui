# `tantra-ui-client` — In-Memory WASM-Safe Event Broadcast

Portable / WASM. Deps: `tantra-core`, `tantra-ui-core`, `serde`, `thiserror`. Not an I/O
client — it is a pure in-memory push bus (the old `architecture.md` claim of a "Typed REST &
WebSocket streaming client state machine" was wrong for this crate).

## Modules

| Module | Contents |
| :--- | :--- |
| `events.rs` | `ClientEvent` (7 variants: `Tick(Tick)`, `Bar(Bar)`, `Order(OrderEvent)`, `Fill(FillReport)`, `Portfolio(PortfolioSnapshot)`, `Screener(ScreenerScanResponse)`, `System(SystemStatus)`), `EventKind` classifier, `SystemStatus`. |
| `stream.rs` | `EventStream` (`subscribe`, `publish`, `published`, `subscriber_count`; bounded log of 4096 events) and `Subscription` (drop-unsubscribes, per-subscriber drain cursor, `drain()`, `pending()`). |

## Design notes

- WASM-safe: no transport, no sockets, no I/O — only `Vec`-backed subscriber state.
- The bounded 4096-event log prevents unbounded growth in long-lived terminal sessions.
- Consumed by `tantra-ui-app`'s `WatchlistFeature` (folds `ClientEvent::Tick/Bar` into a
  `WatchlistViewModel` with a 64-point sparkline).

## Tests

`tests/stream_test.rs` (7 `#[test]` functions).
