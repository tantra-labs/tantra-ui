# `tantra-ui-assets` — Embedded Branding, Themes, Fonts, Fixtures & Audio

Portable (`wasm32` + native). Deps: `tantra-core`, `tantra-ui-core`, `serde`,
`rust_decimal`, `chrono`, `thiserror`.

## Modules

| Module | Contents |
| :--- | :--- |
| `branding.rs` | `Branding`/`BrandName`, `BRAND_NAME` and `TAGLINE` constants. |
| `themes.rs` | `Theme`/`ThemeId`/`Color` — exactly three themes: dark, light, terminal (`all_themes()`). |
| `fonts.rs` | `FontFamily`, `FontStack`, `FontWeight`, `sans_stack()`, `mono_stack()`, `FONTS_CSS` (include_str of `assets/fonts/fonts.css`). Embedded woff/woff2 pairs: IBM Plex Mono/Sans, Inter, Noto Sans, Roboto Mono, Silkscreen, Source Sans 3. |
| `fixtures.rs` | Deterministic sample data: `reliance_bars`, `reliance_quote_tick`, `tcs_depth_tick`, `portfolio_snapshot`, `screener_request`, `filter_chips`. |
| `audio.rs` | Descriptor-only audio: `SoundId`, `SoundProfile`, `profile()`. No playback. |
| `lib.rs` | `COMPONENTS_CSS` (include_str of `assets/style/components.css`). |

## Design notes

- All fixtures are timestamp-anchored and deterministic — no wall clock, matching the
  workspace's temporal-determinism invariant.
- The embedded fonts/CSS are consumed by `tantra-tui`/`tantra-cli` (and would be by a future
  web shell) through `include_str!`; nothing is read from the filesystem at runtime.
- The `assets/` directory also carries standalone branding icon SVGs (added in commit `d055366`).

## Tests

`tests/unit_tests.rs`, `tests/fixtures_test.rs` (17 `#[test]` functions).
