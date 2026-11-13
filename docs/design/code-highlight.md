# `tantra-highlight` — Zero-I/O Syntax Highlighter

Native / Portable. Zero runtime dependencies (only `std`); zero I/O; WASM-safe.

## Public API

- `Language` enum: `Rust`, `Python`, `Json`, `Markdown`, `Shell`, `Plain` (+ `from_name`
  mapping "rust/rs", "py", "json", "md/mdx", "sh/bash/zsh", unknown → Plain).
- `HighlightClass`: `Keyword`, `String`, `Comment`, `Number`, `Function`, `Type`,
  `Punctuation`, `Plain`.
- `HighlightSpan { range: Range<usize>, class }` — half-open byte offsets, returned sorted by
  start.
- `pub fn highlight_code(language: Language, src: &str) -> Vec<HighlightSpan>` — dispatch per
  language with a `LexSyntax` config; Markdown gets a dedicated lexer (fences, headings, inline
  code, bold, links).

## Design notes

- **No tree-sitter** (per `docs/plans/0001` §1.3): the `artha-highlight` tree-sitter +
  WASM-grammar design was explicitly rejected; this is a hand-rolled char lexer. Keeps the crate
  buildable for `wasm32-unknown-unknown` with zero native grammar blobs.
- Consumed by `tantra-tui` (`ChatTranscript::highlight` via `highlight_code(Language::Markdown,
  …)` and `ui.rs::class_to_style`) and available to any future web shell.
- Styling is a `HighlightClass` → ratatui `Style` mapping done in the consumer, keeping the
  lexer presentation-agnostic.

## Tests

`tests/unit_tests.rs` (13 `#[test]` functions).
