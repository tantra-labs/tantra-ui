# 0009 — TUI never restores the terminal on panic and blocks input on network awaits

**Status**: open
**Filed**: 2026-11-23 — workspace design review (root plan 0021) · **Kind**: design-flaw · **Severity**: medium
**Plan**: [0011](../plans/0011-design-review-2026-11.md)

## What's stubbed or hardcoded

- `crates/tantra-tui/src/terminal.rs:47-75` sets raw mode and the alternate screen manually; teardown runs only after `run_loop` returns. There is no `set_hook` or `Drop` guard (no `panic` match in `tantra-tui/src`).
- `Cargo.toml:107-111` `[profile.prod] panic = "abort"`: no unwinding occurs at all.
- `crates/tantra-tui/src/terminal.rs:179-197` inside the render loop: `client.set_model` awaited with a 10 s timeout, `client.prompt(prompt).await`, and `connect(...)` with a 10 s timeout (`:151-157`). The UI is frozen while awaiting.
- `event::poll(30ms)` + `event::read()` is blocking crossterm (`:245`) on the tokio thread.
- Only `Event::Key` is handled (`:246`); no `Event::Resize`, mouse or paste, and no SIGTERM/SIGHUP handler.

## Why

Any panic (for example the `.expect` calls in `app.rs`) leaves the user's shell in raw mode and the alternate screen. A slow ACP server freezes keystrokes for up to 10 s.

## How to close it

1. Add a `TerminalGuard` (`Drop`) plus a panic hook that disables raw mode and leaves the alternate screen; with `panic = "abort"` the hook is the only cleanup path because it runs before the abort.
2. Move `connect`/`set_model`/`prompt` into spawned tasks that report through the existing mpsc channel.
3. Replace poll/read with `crossterm::event::EventStream` inside a `tokio::select!` with a tick interval.
4. Handle `Event::Resize` and SIGTERM/SIGHUP.

**Verification**: ratatui `TestBackend` test for the guard; pty smoke test (panic injected, terminal state restored); `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test -p tantra-tui --profile prod`, `make prod`.

## Related

- Root plan 0019 P3.1 (app.rs split) overlaps only structurally.
- Plan 0011 R2 (client-only build) and R6 (slash-command registry) touch the same TUI crate.
