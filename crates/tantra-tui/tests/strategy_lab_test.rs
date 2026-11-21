//! Deterministic `TestBackend` rendering, interaction & slash-command tests
//! for `StrategyLab` (`Track T21.4`).

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use rust_decimal::Decimal;
use tantra_tui::app::{ActiveTab, SLASH_COMMANDS, StrategyLabDetailView, TuiApp};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
    let mut out = String::new();
    let buf = terminal.backend().buffer();
    let area = buf.area;
    for y in 0..area.height {
        for x in 0..area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

#[test]
fn strategy_lab_renders_canvas_layers_hints_penetration_and_tables() {
    let backend = TestBackend::new(140, 44);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = TuiApp::new();

    // Leave input mode, then '8' switches directly to StrategyLab
    app.is_input_mode = false;
    app.on_key(key(KeyCode::Char('8')));
    assert_eq!(app.active_tab, ActiveTab::StrategyLab);

    // Benchmark view
    terminal
        .draw(|f| tantra_tui::draw(f, &app))
        .expect("draw StrategyLab benchmark");
    let txt = buffer_text(&terminal);
    assert!(txt.contains("elephant_edge_v3"), "canvas HUD:\n{txt}");
    assert!(txt.contains("NSE:NIFTY50-INDEX"), "symbol:\n{txt}");
    assert!(txt.contains("[1:Signals"), "layer 1 pill:\n{txt}");
    assert!(txt.contains("[2:Bands"), "layer 2 pill:\n{txt}");
    assert!(txt.contains("[3:Stops"), "layer 3 pill:\n{txt}");
    assert!(txt.contains("[4:Ind"), "layer 4 pill:\n{txt}");
    assert!(txt.contains("[5:Hints"), "layer 5 pill:\n{txt}");
    assert!(txt.contains("[6:Hedges"), "layer 6 pill:\n{txt}");
    assert!(txt.contains("Hint Inspector"), "hint inspector:\n{txt}");
    assert!(txt.contains("Excursion >Outer"), "penetration:\n{txt}");
    assert!(
        txt.contains("4-Policy Stop Benchmark"),
        "benchmark table:\n{txt}"
    );
    assert!(txt.contains("RSI(14)"), "sub-pane RSI sparkline:\n{txt}");

    // Cycle detail view to Sweep (IS/OOS + DSR)
    app.on_key(key(KeyCode::Char('m')));
    assert_eq!(app.strategy_lab.detail_view, StrategyLabDetailView::Sweep);
    terminal
        .draw(|f| tantra_tui::draw(f, &app))
        .expect("draw StrategyLab sweep");
    let sweep_txt = buffer_text(&terminal);
    assert!(
        sweep_txt.contains("Parameter Sweep Leaderboard"),
        "sweep table:\n{sweep_txt}"
    );
    assert!(
        sweep_txt.contains("cand-"),
        "sweep candidates:\n{sweep_txt}"
    );

    // Cycle detail view to Hedges (F&O Expiry Hedge Card)
    app.on_key(key(KeyCode::Char('m')));
    assert_eq!(app.strategy_lab.detail_view, StrategyLabDetailView::Hedges);
    terminal
        .draw(|f| tantra_tui::draw(f, &app))
        .expect("draw StrategyLab hedges");
    let hedge_txt = buffer_text(&terminal);
    assert!(
        hedge_txt.contains("Expiry Hedge Card"),
        "hedge card:\n{hedge_txt}"
    );
    assert!(
        hedge_txt.contains("BullPutCreditSpread"),
        "hedge structure:\n{hedge_txt}"
    );
}

#[test]
fn strategy_lab_layer_toggles_hint_cursor_and_param_tweaks_update_state() {
    let mut app = TuiApp::new();
    app.active_tab = ActiveTab::StrategyLab;
    app.is_input_mode = false;

    // Layer toggles [1..6]
    assert!(app.strategy_lab.vm.scene.layers.signals);
    app.on_key(key(KeyCode::Char('1')));
    assert!(!app.strategy_lab.vm.scene.layers.signals);
    assert!(app.strategy_lab.vm.projected.signals.is_empty());

    app.on_key(key(KeyCode::Char('4')));
    assert!(!app.strategy_lab.vm.scene.layers.indicators);
    app.on_key(key(KeyCode::Char('4')));
    assert!(app.strategy_lab.vm.scene.layers.indicators);

    // Hint cursor stepping (Right / Left / n / p)
    assert_eq!(app.strategy_lab.selected_hint_index, 0);
    app.on_key(key(KeyCode::Right));
    assert_eq!(app.strategy_lab.selected_hint_index, 1);
    app.on_key(key(KeyCode::Char('p')));
    assert_eq!(app.strategy_lab.selected_hint_index, 0);

    // Parameter tweak (+/- adjusts sl_buffer_atr and populates delta_summary)
    let init_buf = app.strategy_lab.config.sl_buffer_atr;
    app.on_key(key(KeyCode::Char('-')));
    assert_eq!(
        app.strategy_lab.config.sl_buffer_atr,
        init_buf - Decimal::new(5, 2)
    );
    assert!(app.strategy_lab.vm.delta_summary.is_some());

    // Strategy, asset & expiry horizon cycling across Index -> F&O Equity -> Commodity
    app.on_key(key(KeyCode::Char('s')));
    assert_eq!(app.strategy_lab.config.strategy_id, "orb_15");
    app.on_key(key(KeyCode::Char('a')));
    assert_eq!(app.strategy_lab.config.symbol, "NSE:RELIANCE-EQ");
    app.on_key(key(KeyCode::Char('a')));
    assert_eq!(app.strategy_lab.config.symbol, "MCX:GOLD");
    app.on_key(key(KeyCode::Char('x')));
    assert_eq!(app.strategy_lab.config.expiry_horizon, "monthly");
}

#[test]
fn phase4_slash_commands_wire_agent_chat_and_strategy_lab() {
    let cmds: Vec<&str> = SLASH_COMMANDS.iter().map(|(c, _)| *c).collect();
    assert!(cmds.contains(&"/ee-study"));
    assert!(cmds.contains(&"/loop-eval"));
    assert!(cmds.contains(&"/loop-sweep"));
    assert!(cmds.contains(&"/hedge"));

    let mut app = TuiApp::new();

    app.execute_slash_command("/ee-study orb_15 NSE:RELIANCE-EQ 5m");
    assert_eq!(app.active_tab, ActiveTab::StrategyLab);
    assert_eq!(app.strategy_lab.config.strategy_id, "orb_15");
    assert_eq!(app.strategy_lab.config.symbol, "NSE:RELIANCE-EQ");
    assert_eq!(app.strategy_lab.config.timeframe, "5m");
    assert!(app.chat.last().unwrap().text.contains("Strategy Lab ready"));

    app.execute_slash_command("/loop-eval elephant_edge_v3 MCX:GOLD 0.50");
    assert_eq!(app.active_tab, ActiveTab::StrategyLab);
    assert_eq!(app.strategy_lab.config.symbol, "MCX:GOLD");
    assert_eq!(app.strategy_lab.config.sl_buffer_atr, Decimal::new(50, 2));
    assert!(app.chat.last().unwrap().text.contains("Loop Eval"));

    app.execute_slash_command("/loop-sweep elephant_edge_v3 NSE:NIFTY50-INDEX");
    assert_eq!(app.strategy_lab.detail_view, StrategyLabDetailView::Sweep);
    assert!(app.chat.last().unwrap().text.contains("Loop Sweep Winner"));

    app.execute_slash_command("/hedge commodity_cycle MCX:GOLD");
    assert_eq!(app.strategy_lab.detail_view, StrategyLabDetailView::Hedges);
    assert_eq!(app.strategy_lab.vm.hedge_card.horizon, "commodity_cycle");
    assert!(app.chat.last().unwrap().text.contains("Hedge Synthesized"));
}
