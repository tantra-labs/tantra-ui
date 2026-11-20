//! Tests for `tantra_ui_core::scene_projector` (`Track T21.4`):
//! - 6-layer `OverlayScene` projection & interactive layer-mask filtering
//! - Pure `Decimal` sparkline normalization & ASCII penetration histogram
//! - Multi-strategy (`elephant_edge_v3`, `orb_15`) & multi-asset
//!   (`NSE:NIFTY50-INDEX`, `NSE:RELIANCE-EQ`, `MCX:GOLD`) deterministic evaluators
//! - Interactive parameter tweaks producing non-zero KPI deltas

use rust_decimal::Decimal;
use tantra_ui_charts::scene::SceneLayerGroup;
use tantra_ui_core::{
    OFFLINE_SEEDED_FIXTURE_SOURCE, StrategyLoopConfig, evaluate_offline_strategy_loop,
    format_strategy_loop_cli_report, render_penetration_histogram_ascii,
};

#[test]
fn projects_all_six_layers_and_respects_layer_toggles() {
    let mut vm = evaluate_offline_strategy_loop(&StrategyLoopConfig::default(), None);
    assert_eq!(vm.data_source, OFFLINE_SEEDED_FIXTURE_SOURCE);
    assert_eq!(vm.candles.len(), 32);

    // Initially all 6 layers are active & populated
    assert!(!vm.projected.signals.is_empty());
    assert!(!vm.projected.bands.is_empty());
    assert!(!vm.projected.stops.is_empty());
    assert!(!vm.projected.overlay_traces.is_empty());
    assert!(!vm.projected.subpane_traces.is_empty());
    assert!(!vm.projected.hints.is_empty());
    assert!(!vm.projected.hedges.is_empty());
    assert_eq!(vm.projected.layer_pills.len(), 6);
    assert!(vm.projected.layer_pills.iter().all(|p| p.active));

    // Sub-pane sparkline bins stay cleanly inside 0..=100
    for tr in &vm.projected.subpane_traces {
        assert!(!tr.sparkline_bins.is_empty());
        assert!(tr.sparkline_bins.iter().all(|&b| b <= 100));
    }

    // Toggling off Signals hides signals & connectors while preserving counts metadata
    let now_visible = vm.toggle_layer(SceneLayerGroup::Signals);
    assert!(!now_visible);
    assert!(vm.projected.signals.is_empty());
    assert!(vm.projected.connectors.is_empty());
    assert!(vm.projected.layer_counts.signals > 0);
    assert!(!vm.projected.layer_pills[0].active);

    // Toggling off Stops & Hedges hides those layers cleanly
    assert!(!vm.toggle_layer(SceneLayerGroup::Stops));
    assert!(vm.projected.stops.is_empty());
    assert!(!vm.toggle_layer(SceneLayerGroup::Hedges));
    assert!(vm.projected.hedges.is_empty());
}

#[test]
fn supports_multi_strategy_and_multi_asset_presets_and_parameter_deltas() {
    let base = StrategyLoopConfig::default();
    let run1 = evaluate_offline_strategy_loop(&base, None);
    assert_eq!(run1.run_seq, 1);
    assert!(run1.delta_summary.is_none());
    assert_eq!(run1.stop_comparison.len(), 4);
    assert!(!run1.sweep_rows.is_empty());
    assert_eq!(run1.all_horizon_hedges.len(), 4);

    // Tweaking sl_buffer_atr down to 0.00 exposes tight stop-outs and computes delta
    let tight_cfg = StrategyLoopConfig {
        sl_buffer_atr: Decimal::ZERO,
        ..base.clone()
    };
    let run2 = evaluate_offline_strategy_loop(&tight_cfg, Some(&run1));
    assert_eq!(run2.run_seq, 2);
    let delta = run2.delta_summary.as_ref().expect("delta vs run1");
    assert!(
        delta.avg_r_delta != Decimal::ZERO || delta.stop_out_rate_pct_delta != Decimal::ZERO,
        "tweaking SL buffer must change KPI outcome: {delta:?}"
    );

    // Second distinct strategy `orb_15` on Equity (`NSE:RELIANCE-EQ`) and Commodity (`MCX:GOLD`)
    let orb_cfg = StrategyLoopConfig {
        strategy_id: "orb_15".to_string(),
        symbol: "NSE:RELIANCE-EQ".to_string(),
        asset_class: "fno_stock".to_string(),
        timeframe: "5m".to_string(),
        expiry_horizon: "monthly".to_string(),
        entry_trigger_mode: "auto".to_string(),
        sl_buffer_atr: Decimal::new(25, 2),
        horizon_bars: 5,
        target_r: Decimal::from(3),
    };
    let orb_vm = evaluate_offline_strategy_loop(&orb_cfg, None);
    assert_eq!(orb_vm.config.entry_trigger_mode, "orb_breakout");
    assert!(
        orb_vm
            .scene
            .bands
            .iter()
            .any(|b| b.label.as_deref() == Some("OR_LOW"))
    );
    assert!(orb_vm.hedge_card.structure.contains("ProtectiveCollar"));

    let gold_cfg = StrategyLoopConfig {
        strategy_id: "elephant_edge_v3".to_string(),
        symbol: "MCX:GOLD".to_string(),
        asset_class: "commodity".to_string(),
        timeframe: "1d".to_string(),
        expiry_horizon: "commodity_cycle".to_string(),
        entry_trigger_mode: "rejection_wick".to_string(),
        sl_buffer_atr: Decimal::new(50, 2),
        horizon_bars: 5,
        target_r: Decimal::from(3),
    };
    let gold_vm = evaluate_offline_strategy_loop(&gold_cfg, None);
    assert!(gold_vm.spot_price > Decimal::from(50_000));
    assert!(gold_vm.hedge_card.structure.contains("CalendarRoll"));
}

#[test]
fn ascii_formatters_render_penetration_histogram_and_report() {
    let vm = evaluate_offline_strategy_loop(&StrategyLoopConfig::default(), None);
    let hist_txt = render_penetration_histogram_ascii(&vm.penetration);
    assert!(hist_txt.contains("P50"));
    assert!(hist_txt.contains("P95"));
    assert!(hist_txt.contains("█"));

    let report = format_strategy_loop_cli_report(&vm, true, true);
    assert!(report.contains("elephant_edge_v3"));
    assert!(report.contains("NSE:NIFTY50-INDEX"));
    assert!(report.contains(OFFLINE_SEEDED_FIXTURE_SOURCE));
    assert!(report.contains("Stop-Hunt Buffer Benchmark"));
    assert!(report.contains("Parameter Sweep Leaderboard"));
    assert!(report.contains("Hedge Recommendation"));
}
