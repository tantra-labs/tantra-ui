//! Tests for the pure analysis-page model and LiveData price history.

use std::str::FromStr;

use rust_decimal::Decimal;
use tantra_agent_core::DisplayBlock;
use tantra_core::chrono::Utc;
use tantra_core::{Bar, Exchange, InstrumentId, Price, Quantity};
use tantra_tui::analysis::{AnalysisPage, base_symbol, detect_instruments};
use tantra_tui::live_data::{LiveData, PRICE_HISTORY_CAP};
use tantra_ui_client::ClientEvent;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn alert(msg: &str) -> DisplayBlock {
    DisplayBlock::Alert {
        level: "info".into(),
        message: msg.into(),
    }
}

#[test]
fn base_symbol_cases() {
    assert_eq!(base_symbol("NSE:NIFTY50-INDEX"), "NIFTY50");
    assert_eq!(base_symbol("NSE:reliance-eq"), "RELIANCE");
    assert_eq!(base_symbol("NSE:NIFTY-FUT"), "NIFTY");
    assert_eq!(base_symbol("tcs"), "TCS");
}

#[test]
fn detects_aliases() {
    assert_eq!(
        detect_instruments("What is NIFTY doing vs BANKNIFTY?", &[]),
        s(&["NSE:NIFTY50-INDEX", "NSE:NIFTYBANK-INDEX"])
    );
}

#[test]
fn detects_caps_tokens() {
    assert_eq!(
        detect_instruments("compare RELIANCE and TCS", &[]),
        s(&["NSE:RELIANCE", "NSE:TCS"])
    );
}

#[test]
fn detects_known_symbols_case_insensitive() {
    assert_eq!(
        detect_instruments("how is reliance today", &s(&["NSE:RELIANCE-EQ"])),
        s(&["NSE:RELIANCE-EQ"])
    );
    assert_eq!(
        detect_instruments("RELIANCE", &s(&["NSE:RELIANCE-EQ"])),
        s(&["NSE:RELIANCE-EQ"])
    );
}

#[test]
fn ignores_stopwords_and_dedupes() {
    assert!(detect_instruments("what is the RSI and ATR", &[]).is_empty());
    assert_eq!(
        detect_instruments("TCS and TCS and TCS", &[]),
        s(&["NSE:TCS"])
    );
}

#[test]
fn page_focus_cap_and_order() {
    let mut p = AnalysisPage::default();
    assert!(p.is_empty());
    p.focus_on(s(&["A1", "B1", "C1"]));
    assert_eq!(p.primary(), Some("A1"));
    p.focus_on(s(&["D1", "B1", "E1"]));
    assert_eq!(p.focus, s(&["D1", "B1", "E1", "A1"]));
    assert!(!p.is_empty());
    p.clear();
    assert!(p.is_empty());
}

#[test]
fn page_cards_cap_newest_first_and_scroll() {
    let mut p = AnalysisPage::default();
    for i in 0..15 {
        p.push_blocks("t", 0, vec![alert(&i.to_string())]);
    }
    assert_eq!(p.cards.len(), 12);
    assert_eq!(p.cards[0].block, alert("14"));
    p.scroll_up();
    assert_eq!(p.scroll, 0);
    p.scroll_down();
    assert_eq!(p.scroll, 1);
}

#[test]
fn price_history_grows_and_caps() {
    let mut live = LiveData::new();
    let inst = InstrumentId::new(Exchange::Nse, "NIFTY50");
    for i in 0..(PRICE_HISTORY_CAP + 10) {
        let px = Price::new(Decimal::from_str(&(100 + i).to_string()).unwrap());
        live.apply_event(&ClientEvent::Bar(Bar {
            instrument: inst.clone(),
            open: px,
            high: px,
            low: px,
            close: px,
            volume: Quantity::new(Decimal::from(1)),
            timestamp: Utc::now(),
        }));
    }
    let sym = format!("{inst}");
    let h = live.history(&sym).unwrap();
    assert_eq!(h.len(), PRICE_HISTORY_CAP);
    assert_eq!(*h.front().unwrap(), Decimal::from(110));
    assert!(live.quote(&sym).is_some());
    assert!(live.depth(&sym).is_none());
    assert!(live.position(&sym).is_none());
}

#[test]
fn detect_instruments_handles_multibyte_text() {
    let known = vec!["NSE:RELIANCE-EQ".to_string()];
    let got = tantra_tui::analysis::detect_instruments(
        "analyze NSE:RELIANCE-EQ 1d — target ₹2,950 ✓, हिन्दी straße NIFTY",
        &known,
    );
    assert!(got.contains(&"NSE:RELIANCE-EQ".to_string()), "{got:?}");
    assert!(got.contains(&"NSE:NIFTY50-INDEX".to_string()), "{got:?}");
}

#[test]
fn streamed_rupee_reply_renders_without_panic() {
    use ratatui::{Terminal, backend::TestBackend};
    let mut app = tantra_tui::TuiApp::new();
    app.submit_prompt("analyze NSE:RELIANCE-EQ 1d".into());
    let reply = "* Current Price: ₹2,950.40 **up** `0.8%` — हिन्दी ✓";
    for ch in reply.chars() {
        app.apply_stream_update(tantra_tui::AgentStreamUpdate::Token(ch.to_string()));
        let mut t = Terminal::new(TestBackend::new(140, 40)).expect("terminal");
        t.draw(|f| tantra_tui::draw(f, &app)).expect("draw");
    }
}
