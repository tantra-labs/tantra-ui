//! Regression: the highlighter scans bytes, and slicing at a byte inside a multi-byte
//! character (e.g. `₹`, which is 3 bytes) panicked the whole TUI mid-stream
//! ("start byte index 139 is not a char boundary; it is inside '₹'").

use tantra_highlight::{Language, highlight_code};

const LANGS: [Language; 6] = [
    Language::Rust,
    Language::Python,
    Language::Json,
    Language::Markdown,
    Language::Shell,
    Language::Plain,
];

#[test]
fn rupee_in_streamed_markdown_does_not_panic() {
    let reply = "Here are the key statistics for Reliance Industries (RELIANCE-EQ) on the 1-day chart:\n\n* Current Price: ₹2,950.40\n* Day range: ₹2,921–₹2,968 **up** `0.8%` [NSE](https://nseindia.com)\n```\n₹ fenced\n```\n# हिन्दी heading ✓";
    // every streamed prefix, cut at each char boundary, as the TUI re-highlights per token
    for (cut, _) in reply.char_indices().chain([(reply.len(), ' ')]) {
        let src = &reply[..cut];
        for lang in LANGS {
            for span in highlight_code(lang, src) {
                assert!(
                    src.get(span.range.clone()).is_some(),
                    "{lang:?} span {:?} not on char boundaries in {src:?}",
                    span.range
                );
            }
        }
    }
}

#[test]
fn multibyte_next_to_every_marker_is_safe() {
    let markers = [
        "```", "~~~", "**", "__", "*", "_", "`", "[", "](", ")", "#", "\"", "'", "//", "/*", "--",
        "0.5", "fn", "{", ":",
    ];
    let wide = ["₹", "é", "✓", "हि", "😀"];
    for m in markers {
        for w in wide {
            for src in [
                format!("{w}{m}{w}"),
                format!("{m}{w}"),
                format!("{w}{m}"),
                format!("a{w}{m}{w}b\n{w}"),
            ] {
                for lang in LANGS {
                    for span in highlight_code(lang, &src) {
                        assert!(
                            src.get(span.range.clone()).is_some(),
                            "{lang:?} {src:?} {:?}",
                            span.range
                        );
                    }
                }
            }
        }
    }
}
