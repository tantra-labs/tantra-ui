# 0010 — Serve state defaults are CWD-relative; two persistence roots; recovery sweep on every invocation

**Status**: open
**Filed**: 2026-11-23 — workspace design review (root plan 0021) · **Kind**: deployment · **Severity**: medium
**Plan**: [0011](../plans/0011-design-review-2026-11.md)

## What's stubbed or hardcoded

- `tantra-runtime/crates/tantra-runtime-api-rest/src/bootstrap.rs:123-129` defaults to `data/journals|profiles|portfolios`; `crates/tantra-cli/src/args.rs:226-232` documents "Default `data/profiles`". Both resolve against the CWD, so starting `tantra serve` from another directory silently creates a fresh, empty profile/portfolio/run store (the untracked `data/` at the workspace root is exactly this).
- `crates/tantra-cli/src/runner.rs:104-116` `tantra_home_dir()` returns `~/.tantra`, or a relative `.tantra` when `HOME` is unset (containers, systemd units).
- `crates/tantra-cli/src/main.rs:14` `boot_recover(Some(&tantra_home_dir()))` runs before every command, including `serve` and `--help` paths after parse, and rewrites job journals unrelated to serve's `journals_dir`.
- Only `std::env::var("HOME")` is consulted: no XDG/`dirs` handling; on Windows (`USERPROFILE`) it falls to `./.tantra`.

## Why

Restarting from another CWD or inside a container loses server state, contrary to the restart-safe intent of `.agents/rules/scalability-and-concurrency.md` §4. An unset `HOME` writes into whatever directory the process started in. Read-only commands mutate on-disk journals.

## How to close it

1. Default the serve state directory to `$TANTRA_HOME/state` (absolute, created on start) and log the chosen paths at startup.
2. Fail with an error, rather than fall back to a relative path, when neither `TANTRA_HOME` nor a platform home directory resolves.
3. Use `dirs`/`etcetera` for platform directories (Linux XDG, macOS, Windows).
4. Run `boot_recover` only for `jobs` and `tools` commands.

**Verification**: tempdir and env-isolated tests using the existing `*_with_home` pattern (unset `HOME` errors; serve from two CWDs resolves to one state dir; `--help` performs no writes); `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test -p tantra-cli --profile prod`, `make prod`.

## Related

- Root plan 0022 (deployment architecture: state/data layout).
- Plan 0011 R1 (serve composition root) and R4 (daemon basics) share the serve startup path; provider side `tantra-runtime/docs/gaps/0026-single-composition-root-serve.md`.
