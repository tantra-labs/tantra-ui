//! Pure state for the split-screen "Analysis" page (no I/O, no rendering).

use tantra_agent_core::DisplayBlock;

const STOPWORDS: &[&str] = &[
    "AND", "THE", "FOR", "WHAT", "WHY", "HOW", "NSE", "BSE", "SEBI", "ATR", "RSI", "SMA", "EMA",
    "MACD", "VWAP", "TWAP", "ETF", "IPO", "FII", "DII", "PNL", "OTR", "CE", "PE", "OI", "IV",
    "DTE", "ORB", "USD", "INR", "GDP", "CPI", "RBI", "EPS", "ROE", "ROCE", "YOY", "QOQ", "API",
];

/// Maximum focused tickers.
pub const FOCUS_CAP: usize = 4;
/// Maximum retained tool cards.
pub const CARD_CAP: usize = 12;

/// Normalise an instrument string to its bare uppercase base symbol.
pub fn base_symbol(sym: &str) -> String {
    let up = sym.to_uppercase();
    let s = up.rsplit(':').next().unwrap_or(&up);
    for suffix in ["-INDEX", "-EQ", "-FUT"] {
        if let Some(stripped) = s.strip_suffix(suffix) {
            return stripped.to_string();
        }
    }
    s.to_string()
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '&'
}

/// Detect instruments mentioned in free text. See module docs for rules.
pub fn detect_instruments(text: &str, known: &[String]) -> Vec<String> {
    let upper = text.to_uppercase();
    // (position, symbol) so output follows first-seen order in the text.
    let mut hits: Vec<(usize, String)> = Vec::new();
    let find = |needle: &str| -> Option<usize> {
        let mut start = 0;
        while let Some(pos) = upper[start..].find(needle) {
            let a = start + pos;
            let b = a + needle.len();
            let before_ok = upper[..a]
                .chars()
                .next_back()
                .is_none_or(|c| !is_word_char(c));
            let after_ok = upper[b..].chars().next().is_none_or(|c| !is_word_char(c));
            if before_ok && after_ok {
                return Some(a);
            }
            start = b;
        }
        None
    };

    for k in known {
        let base = base_symbol(k);
        if let Some(p) = find(&base) {
            hits.push((p, k.clone()));
        }
    }
    let aliases: &[(&str, &str)] = &[
        ("NIFTY50", "NSE:NIFTY50-INDEX"),
        ("NIFTY 50", "NSE:NIFTY50-INDEX"),
        ("NIFTY", "NSE:NIFTY50-INDEX"),
        ("BANKNIFTY", "NSE:NIFTYBANK-INDEX"),
        ("NIFTY BANK", "NSE:NIFTYBANK-INDEX"),
        ("FINNIFTY", "NSE:FINNIFTY-INDEX"),
        ("SENSEX", "BSE:SENSEX-INDEX"),
    ];
    let bank_pos = find("NIFTY BANK");
    for (alias, sym) in aliases {
        if let Some(p) = find(alias) {
            if *alias == "NIFTY" && bank_pos == Some(p) {
                continue;
            }
            hits.push((p, (*sym).to_string()));
        }
    }

    // All-caps tokens from the original text.
    let mut offset = 0;
    for raw in text.split(|c: char| !is_word_char(c)) {
        let pos = text[offset..]
            .find(raw)
            .map(|i| i + offset)
            .unwrap_or(offset);
        offset = pos + raw.len();
        let n = raw.chars().count();
        let valid = (3..=15).contains(&n)
            && raw.chars().next().is_some_and(|c| c.is_ascii_uppercase())
            && raw
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '&')
            && !STOPWORDS.contains(&raw)
            && !aliases.iter().any(|(a, _)| *a == raw);
        if !valid {
            continue;
        }
        let sym = known
            .iter()
            .find(|k| base_symbol(k) == raw)
            .cloned()
            .unwrap_or_else(|| format!("NSE:{raw}"));
        hits.push((pos, sym));
    }

    hits.sort_by_key(|(p, _)| *p);
    let mut out: Vec<String> = Vec::new();
    for (_, s) in hits {
        if !out.contains(&s) {
            out.push(s);
        }
    }
    // Drop duplicate-by-base entries where a known/alias symbol exists.
    let mut seen_bases: Vec<String> = Vec::new();
    out.retain(|s| {
        let b = base_symbol(s);
        if seen_bases.contains(&b) {
            false
        } else {
            seen_bases.push(b);
            true
        }
    });
    out
}

/// One tool result block shown on the analysis page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCard {
    /// Producing tool name.
    pub tool: String,
    /// Typed result block.
    pub block: DisplayBlock,
    /// Conversation turn that produced the card.
    pub turn: u64,
}

/// State of the dynamic analysis page.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AnalysisPage {
    /// Focused instruments, newest first (cap 4).
    pub focus: Vec<String>,
    /// Tool result cards, newest first (cap 12).
    pub cards: Vec<ToolCard>,
    /// Whether the backtest report section is shown.
    pub show_backtest: bool,
    /// Scroll offset counted in cards.
    pub scroll: u16,
}

impl AnalysisPage {
    /// Move/insert `symbols` to the front, deduped and capped.
    pub fn focus_on(&mut self, symbols: Vec<String>) {
        for sym in symbols.into_iter().rev() {
            self.focus.retain(|s| *s != sym);
            self.focus.insert(0, sym);
        }
        self.focus.truncate(FOCUS_CAP);
    }

    /// Add each block as a card at the front (capped).
    pub fn push_blocks(&mut self, tool: &str, turn: u64, blocks: Vec<DisplayBlock>) {
        for block in blocks {
            self.cards.insert(
                0,
                ToolCard {
                    tool: tool.to_string(),
                    block,
                    turn,
                },
            );
        }
        self.cards.truncate(CARD_CAP);
    }

    /// The primary (most recently focused) instrument.
    pub fn primary(&self) -> Option<&str> {
        self.focus.first().map(String::as_str)
    }

    /// True when nothing is shown.
    pub fn is_empty(&self) -> bool {
        self.focus.is_empty() && self.cards.is_empty() && !self.show_backtest
    }

    /// Reset to the empty state.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Scroll one card down.
    pub fn scroll_down(&mut self) {
        let max = self.cards.len().saturating_sub(1) as u16;
        self.scroll = self.scroll.saturating_add(1).min(max);
    }

    /// Scroll one card up.
    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }
}
