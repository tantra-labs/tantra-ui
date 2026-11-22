//! `tantra study-loop` and upgraded `tantra elephant` command handler (`Track T21.4`).
//!
//! Two execution paths:
//! - **Live (`--server <url>`)**: Calls `POST /api/v2/studies/strategy-loop/{evaluate,sweep,recommend-hedge}`
//!   against a running `tantra serve` (`tantra-runtime-api-rest`) instance.
//! - **Offline fallback (no `--server`)**: Runs [`evaluate_offline_strategy_loop`] over
//!   deterministic `Decimal` multi-asset fixtures (explicitly labeled `"offline_seeded_fixture"`).
//!
//! Both paths produce identical [`StrategyLoopViewModel`]s, support `--sweep`, `--hedge`,
//! `--json`, and `--export-scene <PATH>` (writing serialized 6-layer `OverlayScene` JSON).

use std::fs;

use rust_decimal::Decimal;
use tantra_runtime_api::experimental_analysis::{RollBasisMode, StudyAssetClass};
use tantra_runtime_api::strategy_loop::{
    ExpiryHedgeRequest, ExpiryHedgeResponse, HedgeLegSideDto, HedgeOptionKindDto,
    HedgeRecommendationDto, HintSeverityDto, LoopEntryTriggerMode, LoopExpiryHorizon,
    LoopStopPolicyMode, LoopTimeframe, MarkerKindDto, MarkerSideDto, OverlaySceneDto, StopKindDto,
    StrategyLoopRequest, StrategyLoopResponse, StrategyParameterBoundsDto, StrategySweepRequest,
    StrategySweepResponse,
};
use tantra_runtime_client::{BlockingRuntimeClient, ClientError};
use tantra_ui_charts::levels::{LevelLine, LevelLineStyle};
use tantra_ui_charts::markers::{MarkerKind, MarkerSide, SignalMarker};
use tantra_ui_charts::scene::{
    HedgeLegSide, HedgeLevelOverlay, HedgeOptionKind, HintAnnotation, HintSeverity, OverlayScene,
    SceneLayerMask, TradeConnectorRay,
};
use tantra_ui_charts::series::{ChartPoint, ChartSeries};
use tantra_ui_charts::stops::{StopKind, StopLine};
use tantra_ui_charts::zones::ZoneBand;
use tantra_ui_core::{
    HedgeCardView, HedgeLegView, HintInspectorItem, LIVE_RUNTIME_API_SOURCE, LoopCandle,
    LoopDeltaSummary, StopPolicyTableRow, StrategyLoopConfig, StrategyLoopViewModel,
    SweepLeaderboardRow, build_penetration_histogram, evaluate_offline_strategy_loop,
    format_strategy_loop_cli_report, infer_asset_class, project_overlay_scene,
};

use crate::args::StudyLoopArgs;
use crate::runner::{CliError, CommandOutcome};

fn parse_timeframe(s: &str) -> LoopTimeframe {
    match s.trim().to_ascii_lowercase().as_str() {
        "1m" | "m1" => LoopTimeframe::M1,
        "5m" | "m5" => LoopTimeframe::M5,
        "15m" | "m15" => LoopTimeframe::M15,
        "1h" | "h1" => LoopTimeframe::H1,
        _ => LoopTimeframe::D1,
    }
}

fn parse_entry_mode(s: &str) -> LoopEntryTriggerMode {
    match s.trim().to_ascii_lowercase().as_str() {
        "zone_touch_close" | "zone_touch" => LoopEntryTriggerMode::ZoneTouchClose,
        "rejection_wick" | "wick" => LoopEntryTriggerMode::RejectionWick,
        "orb_breakout" | "orb" => LoopEntryTriggerMode::OrbBreakout,
        _ => LoopEntryTriggerMode::Auto,
    }
}

fn parse_expiry_horizon(s: &str) -> LoopExpiryHorizon {
    match s.trim().to_ascii_lowercase().as_str() {
        "day" | "intraday" | "0dte" => LoopExpiryHorizon::Day,
        "monthly" | "month" => LoopExpiryHorizon::Monthly,
        "commodity_cycle" | "commodity" | "mcx" => LoopExpiryHorizon::CommodityCycle,
        _ => LoopExpiryHorizon::Weekly,
    }
}

fn parse_asset_class(symbol: &str) -> StudyAssetClass {
    match infer_asset_class(symbol) {
        "index" => StudyAssetClass::Index,
        "commodity" => StudyAssetClass::Commodity,
        _ => StudyAssetClass::FnoStock,
    }
}

fn expiry_horizon_str(h: LoopExpiryHorizon) -> &'static str {
    match h {
        LoopExpiryHorizon::Day => "day",
        LoopExpiryHorizon::Weekly => "weekly",
        LoopExpiryHorizon::Monthly => "monthly",
        LoopExpiryHorizon::CommodityCycle => "commodity_cycle",
    }
}

fn entry_mode_str(m: LoopEntryTriggerMode) -> &'static str {
    match m {
        LoopEntryTriggerMode::Auto => "auto",
        LoopEntryTriggerMode::ZoneTouchClose => "zone_touch_close",
        LoopEntryTriggerMode::RejectionWick => "rejection_wick",
        LoopEntryTriggerMode::OrbBreakout => "orb_breakout",
    }
}

fn asset_class_str(ac: StudyAssetClass) -> &'static str {
    match ac {
        StudyAssetClass::Index => "index",
        StudyAssetClass::Commodity => "commodity",
        StudyAssetClass::FnoStock => "fno_stock",
        StudyAssetClass::Etf => "etf",
        StudyAssetClass::Equity => "equity",
        StudyAssetClass::Forex => "forex",
    }
}

/// Map a client failure to the CLI's error type.
fn client_err(what: &str, e: ClientError) -> CliError {
    CliError::Execution(format!("strategy-loop {what} failed: {e}"))
}

fn dto_to_overlay_scene(dto: &OverlaySceneDto) -> OverlayScene {
    let mut scene = OverlayScene::new(&dto.strategy_id, &dto.symbol, &dto.timeframe);
    scene.layers = SceneLayerMask {
        signals: dto.layers.signals,
        bands: dto.layers.bands,
        stops: dto.layers.stops,
        indicators: dto.layers.indicators,
        hints: dto.layers.hints,
        hedges: dto.layers.hedges,
    };

    for s in &dto.signals {
        let side = match s.side {
            MarkerSideDto::Buy => MarkerSide::Buy,
            MarkerSideDto::Sell => MarkerSide::Sell,
        };
        let kind = match s.kind {
            MarkerKindDto::Breakout => MarkerKind::Breakout,
            MarkerKindDto::Reversal => MarkerKind::Reversal,
            MarkerKindDto::Retest => MarkerKind::Retest,
            MarkerKindDto::StopLine => MarkerKind::StopLine,
            MarkerKindDto::Sweep => MarkerKind::Sweep,
            MarkerKindDto::SweepSaved => MarkerKind::SweepSaved,
            MarkerKindDto::TargetHit => MarkerKind::TargetHit,
            MarkerKindDto::ElephantBar => MarkerKind::ElephantBar,
        };
        scene
            .signals
            .push(SignalMarker::new(s.price, s.index, side, kind));
    }

    for c in &dto.connectors {
        let side = match c.side {
            MarkerSideDto::Buy => MarkerSide::Buy,
            MarkerSideDto::Sell => MarkerSide::Sell,
        };
        if let Ok(mut ray) = TradeConnectorRay::new(
            c.from_index,
            c.from_price,
            c.to_index,
            c.to_price,
            side,
            c.realized_r,
            c.stopped_out,
        ) {
            if let Some(ref lbl) = c.label {
                ray = ray.with_label(lbl.clone());
            }
            scene.connectors.push(ray);
        }
    }

    for b in &dto.bands {
        if let Ok(mut zb) = ZoneBand::new(b.low, b.high, b.x0, b.x1) {
            zb = zb.with_fill_index(b.fill_index);
            if let Some(ref lbl) = b.label {
                zb = zb.with_label(lbl.clone());
            }
            scene.bands.push(zb);
        }
    }

    for l in &dto.levels {
        if let Ok(mut ll) = LevelLine::new(l.price, l.x0, l.x1) {
            ll = ll.with_style(LevelLineStyle {
                dashed: l.style.dashed,
                color_index: l.style.color_index,
                line_width: l.style.line_width,
            });
            if let Some(ref lbl) = l.label {
                ll = ll.with_label(lbl.clone());
            }
            scene.levels.push(ll);
        }
    }

    for st in &dto.stops {
        let kind = match st.kind {
            StopKindDto::Initial => StopKind::Initial,
            StopKindDto::Trailing => StopKind::Trailing,
            StopKindDto::Breakeven => StopKind::Breakeven,
            StopKindDto::Target => StopKind::Target,
        };
        if let Ok(mut line) = StopLine::new(st.price, st.x0, st.x1, kind) {
            if let Some(prev) = st.previous {
                // Reconstruct `previous` cleanly via update(actual) from `prev`.
                if let Ok(mut seeded) = StopLine::new(prev, st.x0, st.x1, kind) {
                    if seeded.update(st.price).is_ok() {
                        line = seeded;
                    }
                }
            }
            scene.stops.push(line);
        }
    }

    for ser in &dto.indicators {
        let pts = ser
            .points
            .iter()
            .map(|p| ChartPoint::new(p.x, p.y))
            .collect();
        scene.indicators.push(ChartSeries::new(&ser.name, pts));
    }

    for h in &dto.hints {
        let sev = match h.severity {
            HintSeverityDto::Info => HintSeverity::Info,
            HintSeverityDto::Positive => HintSeverity::Positive,
            HintSeverityDto::Warning => HintSeverity::Warning,
            HintSeverityDto::Critical => HintSeverity::Critical,
        };
        if let Ok(mut ha) = HintAnnotation::new(h.bar_index, h.price, sev, &h.title, &h.detail) {
            for m in &h.metrics {
                ha = ha.with_metric(&m.key, &m.value);
            }
            scene.hints.push(ha);
        }
    }

    for hd in &dto.hedges {
        let side = match hd.side {
            HedgeLegSideDto::Buy => HedgeLegSide::Buy,
            HedgeLegSideDto::Sell => HedgeLegSide::Sell,
        };
        let otype = match hd.option_kind {
            HedgeOptionKindDto::Spot => HedgeOptionKind::Spot,
            HedgeOptionKindDto::Future => HedgeOptionKind::Future,
            HedgeOptionKindDto::Call => HedgeOptionKind::Call,
            HedgeOptionKindDto::Put => HedgeOptionKind::Put,
        };
        if let Ok(mut hl) = HedgeLevelOverlay::new(&hd.role, side, otype, hd.strike, hd.x0, hd.x1) {
            if let Some(be) = hd.breakeven {
                hl = hl.with_breakeven(be);
            }
            if let Some(ml) = hd.max_loss {
                hl = hl.with_max_loss(ml);
            }
            if let Some(ref s) = hd.summary {
                hl = hl.with_summary(s.clone());
            }
            scene.hedges.push(hl);
        }
    }

    scene
}

fn hedge_dto_to_view(dto: &HedgeRecommendationDto) -> HedgeCardView {
    HedgeCardView {
        horizon: expiry_horizon_str(dto.horizon).to_string(),
        structure: format!("{:?}", dto.structure),
        chain_status: dto.chain_status.clone(),
        live_contract_count: dto.live_contract_count,
        legs: dto
            .legs
            .iter()
            .map(|l| HedgeLegView {
                side: l.side.clone(),
                option_type: l.option_type.clone(),
                strike: l.strike,
                expiry: l.expiry.clone(),
                lot_qty: l.lot_qty,
                leg_price: l.leg_price,
                quote_source: l.quote_source.clone(),
            })
            .collect(),
        net_credit_or_debit: dto.net_credit_or_debit,
        max_loss_inr: dto.max_loss_inr,
        max_loss_pct_equity: dto.max_loss_pct_equity,
        p95_stress_loss_inr: dto.p95_stress_loss_inr,
        estimated_friction_inr: dto.estimated_friction_inr,
        breakevens: dto.breakevens.clone(),
        rationale: dto.rationale.clone(),
    }
}

fn server_responses_to_view_model(
    cfg: &StrategyLoopConfig,
    eval: StrategyLoopResponse,
    sweep: Option<StrategySweepResponse>,
    hedge: Option<ExpiryHedgeResponse>,
) -> StrategyLoopViewModel {
    let candles: Vec<LoopCandle> = eval
        .bars
        .iter()
        .map(|b| LoopCandle {
            index: b.index,
            timestamp: b.timestamp.clone(),
            open: b.open,
            high: b.high,
            low: b.low,
            close: b.close,
            volume: b.volume,
        })
        .collect();

    let scene = if let Some(ref sw) = sweep {
        dto_to_overlay_scene(&sw.winner_scene)
    } else {
        dto_to_overlay_scene(&eval.scene)
    };
    let projected = project_overlay_scene(&candles, &scene);

    let stop_comparison: Vec<StopPolicyTableRow> = eval
        .stop_comparison
        .models
        .iter()
        .map(|m| StopPolicyTableRow {
            policy_id: m.policy_id.clone(),
            label: m.label.clone(),
            buffer_atr: m.buffer_atr,
            trades: m.kpis.trades,
            win_rate_pct: m.kpis.win_rate_pct,
            stop_out_rate_pct: m.kpis.stop_out_rate_pct,
            false_sweep_rate_pct: m.kpis.false_sweep_stop_rate_pct,
            avg_realized_r: m.kpis.avg_realized_r,
            net_avg_realized_r: m.kpis.net_avg_realized_r,
            profit_factor: m.kpis.net_profit_factor,
            sharpe_ratio: m.kpis.sharpe_ratio,
            max_drawdown_r: m.kpis.max_drawdown_r,
            is_recommended: m.policy_id == eval.stop_comparison.recommended_policy_id,
        })
        .collect();

    let penetration = build_penetration_histogram(
        eval.penetration.sample_count,
        eval.penetration.pct_recovering_after_outer_touch,
        eval.penetration.p50_atr,
        eval.penetration.p75_atr,
        eval.penetration.p90_atr,
        eval.penetration.p95_atr,
        eval.penetration.max_atr,
        16,
    );

    let (sweep_rows, delta_summary) = if let Some(ref sw) = sweep {
        let base_r = sw.baseline_kpis.net_avg_realized_r;
        let rows = sw
            .candidates
            .iter()
            .enumerate()
            .map(|(idx, c)| SweepLeaderboardRow {
                rank: idx + 1,
                candidate_id: format!("cand-{:02}", c.candidate_index + 1),
                sl_buffer_atr: c.sl_buffer_atr,
                horizon_bars: c.horizon_bars,
                target_r: c.target_r,
                is_sharpe: c.is_kpis.sharpe_ratio,
                oos_sharpe: c.oos_kpis.sharpe_ratio,
                oos_avg_r: c.oos_kpis.net_avg_realized_r,
                wfe_pct: c.walk_forward_efficiency_pct,
                dsr: c.deflated_sharpe_ratio,
                stop_out_rate_pct: c.full_kpis.stop_out_rate_pct,
                delta_vs_baseline_r: (c.full_kpis.net_avg_realized_r - base_r).round_dp(2),
                is_winner: c.candidate_index == sw.winner.candidate_index,
            })
            .collect();
        let dlt = Some(LoopDeltaSummary {
            label: "winner vs baseline".to_string(),
            avg_r_delta: sw.delta_winner_vs_baseline.net_avg_realized_r_delta,
            win_rate_pct_delta: sw.delta_winner_vs_baseline.win_rate_pct_delta,
            stop_out_rate_pct_delta: sw.delta_winner_vs_baseline.stop_out_rate_pct_delta,
            trap_rate_pct_delta: sw.delta_winner_vs_baseline.false_sweep_stop_rate_pct_delta,
            sharpe_delta: sw.delta_winner_vs_baseline.sharpe_ratio_delta,
        });
        (rows, dlt)
    } else {
        (
            Vec::new(),
            eval.delta_vs_previous.map(|dlt| LoopDeltaSummary {
                label: "vs previous run".to_string(),
                avg_r_delta: dlt.net_avg_realized_r_delta,
                win_rate_pct_delta: dlt.win_rate_pct_delta,
                stop_out_rate_pct_delta: dlt.stop_out_rate_pct_delta,
                trap_rate_pct_delta: dlt.false_sweep_stop_rate_pct_delta,
                sharpe_delta: dlt.sharpe_ratio_delta,
            }),
        )
    };

    let (hedge_card, all_horizon_hedges) = if let Some(ref h) = hedge {
        (
            hedge_dto_to_view(&h.recommended),
            h.all_horizons.iter().map(hedge_dto_to_view).collect(),
        )
    } else {
        let card = hedge_dto_to_view(&eval.hedge);
        (card.clone(), vec![card])
    };

    let hints = scene
        .hints
        .iter()
        .enumerate()
        .map(|(idx, h)| HintInspectorItem {
            index: idx,
            bar_index: h.bar_index,
            price: h.price,
            severity: h.severity,
            title: h.title.clone(),
            detail: h.detail.clone(),
            metrics: h.metrics.clone(),
        })
        .collect();

    StrategyLoopViewModel {
        data_source: LIVE_RUNTIME_API_SOURCE.to_string(),
        session_id: eval.session_id,
        run_seq: eval.run_seq,
        config: StrategyLoopConfig {
            strategy_id: eval.pipeline.strategy_id,
            symbol: eval.instrument_id,
            asset_class: asset_class_str(eval.asset_class).to_string(),
            timeframe: eval.timeframe.as_str().to_string(),
            expiry_horizon: cfg.expiry_horizon.clone(),
            entry_trigger_mode: cfg.entry_trigger_mode.clone(),
            sl_buffer_atr: cfg.sl_buffer_atr,
            horizon_bars: cfg.horizon_bars,
            target_r: cfg.target_r,
        },
        spot_price: eval.spot_price,
        atr: eval.atr,
        total_trades: eval.kpis.trades,
        win_rate_pct: eval.kpis.win_rate_pct,
        stop_out_rate_pct: eval.kpis.stop_out_rate_pct,
        false_sweep_rate_pct: eval.kpis.false_sweep_stop_rate_pct,
        net_avg_realized_r: eval.kpis.net_avg_realized_r,
        profit_factor: eval.kpis.net_profit_factor,
        sharpe_ratio: eval.kpis.sharpe_ratio,
        max_drawdown_r: eval.kpis.max_drawdown_r,
        verdict_summary: eval.stop_comparison.verdict_summary,
        candles,
        scene,
        projected,
        stop_comparison,
        recommended_policy_id: eval.stop_comparison.recommended_policy_id,
        penetration,
        sweep_rows,
        delta_summary,
        hedge_card,
        all_horizon_hedges,
        hints,
    }
}

fn evaluate_live_server(
    server_base: &str,
    args: &StudyLoopArgs,
    cfg: &StrategyLoopConfig,
) -> Result<StrategyLoopViewModel, CliError> {
    let base = server_base.trim_end_matches('/');
    // R3b adds config-sourced tokens; none is available here yet.
    let client = BlockingRuntimeClient::new(base, None)
        .map_err(|e| CliError::Execution(format!("failed to build HTTP client: {e}")))?;

    let asset_class = parse_asset_class(&cfg.symbol);
    let tf = parse_timeframe(&cfg.timeframe);
    let entry_mode = parse_entry_mode(&cfg.entry_trigger_mode);
    let expiry = parse_expiry_horizon(&cfg.expiry_horizon);

    let start_date = args
        .start_date
        .clone()
        .unwrap_or_else(|| "2025-06-16".to_string());
    let end_date = args
        .end_date
        .clone()
        .unwrap_or_else(|| "2025-12-31".to_string());

    let eval_req = StrategyLoopRequest {
        strategy_id: cfg.strategy_id.clone(),
        instrument_id: cfg.symbol.clone(),
        asset_class,
        start_date: start_date.clone(),
        end_date: end_date.clone(),
        timeframe: tf,
        session_preset: None,
        entry_trigger_mode: entry_mode,
        stop_policy_mode: LoopStopPolicyMode::OuterAtrBuffer,
        sl_buffer_atr: Some(cfg.sl_buffer_atr),
        horizon_bars: Some(cfg.horizon_bars),
        target_r: Some(cfg.target_r),
        atr_period: Some(14),
        partial_r: None,
        runner_r: None,
        time_stop_bars: None,
        expiry_horizon: expiry,
        roll_basis_mode: RollBasisMode::NetAfterSpreadAndCosts,
        spread_slippage_bps: None,
        futures_roll_basis_bps: None,
        include_statutory_costs: true,
        lot_size: None,
        tick_size: None,
        capital: None,
        session_id: None,
        max_chart_bars: Some(120),
    };

    let eval_resp: StrategyLoopResponse = client
        .strategy_loop_evaluate(&eval_req)
        .map_err(|e| client_err("evaluate", e))?;

    let sweep_resp: Option<StrategySweepResponse> = if args.sweep {
        let sweep_req = StrategySweepRequest {
            base: eval_req.clone(),
            bounds: StrategyParameterBoundsDto {
                sl_buffer_atr_grid: vec![Decimal::ZERO, Decimal::new(25, 2), Decimal::new(50, 2)],
                horizon_bars_grid: vec![3, cfg.horizon_bars],
                target_r_grid: vec![Decimal::from(2), cfg.target_r],
                max_candidates: Some(12),
            },
        };
        Some(
            client
                .strategy_loop_sweep(&sweep_req)
                .map_err(|e| client_err("sweep", e))?,
        )
    } else {
        None
    };

    let hedge_resp: Option<ExpiryHedgeResponse> = if args.hedge {
        let hedge_req = ExpiryHedgeRequest {
            instrument_id: cfg.symbol.clone(),
            asset_class,
            strategy_id: Some(cfg.strategy_id.clone()),
            start_date: Some(start_date),
            end_date: Some(end_date),
            timeframe: tf,
            direction: None,
            expiry_horizon: expiry,
            sl_buffer_atr: Some(cfg.sl_buffer_atr),
            horizon_bars: Some(cfg.horizon_bars),
            atr_period: Some(14),
            lot_size: None,
            tick_size: None,
            capital: None,
            spread_slippage_bps: None,
            include_statutory_costs: true,
            iv_gt_rv: false,
        };
        Some(
            client
                .recommend_hedge(&hedge_req)
                .map_err(|e| client_err("recommend-hedge", e))?,
        )
    } else {
        None
    };

    Ok(server_responses_to_view_model(
        cfg, eval_resp, sweep_resp, hedge_resp,
    ))
}

/// Build [`StrategyLoopConfig`] from CLI arguments.
pub fn config_from_args(args: &StudyLoopArgs) -> StrategyLoopConfig {
    let symbol = args
        .symbol
        .clone()
        .unwrap_or_else(|| "NSE:NIFTY50-INDEX".to_string());
    let tf = args.tf.clone().unwrap_or_else(|| "1d".to_string());
    let horizon_bars = args
        .horizon
        .unwrap_or_else(|| if tf.eq_ignore_ascii_case("1d") { 5 } else { 12 });
    let expiry_horizon = args
        .expiry_horizon
        .as_deref()
        .map(|s| expiry_horizon_str(parse_expiry_horizon(s)).to_string())
        .unwrap_or_else(|| {
            if infer_asset_class(&symbol) == "commodity" {
                "commodity_cycle".to_string()
            } else {
                "weekly".to_string()
            }
        });
    let entry_trigger_mode = args
        .entry_mode
        .as_deref()
        .map(|s| entry_mode_str(parse_entry_mode(s)).to_string())
        .unwrap_or_else(|| "auto".to_string());

    StrategyLoopConfig {
        strategy_id: args
            .strategy
            .clone()
            .unwrap_or_else(|| "elephant_edge_v3".to_string()),
        asset_class: infer_asset_class(&symbol).to_string(),
        symbol,
        timeframe: tf,
        expiry_horizon,
        entry_trigger_mode,
        sl_buffer_atr: args.sl_buffer.unwrap_or_else(|| Decimal::new(25, 2)),
        horizon_bars,
        target_r: Decimal::from(3),
    }
}

/// Execute `tantra study-loop` (or flagged `tantra elephant`).
pub fn run_study_loop(args: &StudyLoopArgs) -> Result<CommandOutcome, CliError> {
    let cfg = config_from_args(args);
    let vm = if let Some(ref server) = args.server {
        evaluate_live_server(server, args, &cfg)?
    } else {
        evaluate_offline_strategy_loop(&cfg, None)
    };

    if let Some(ref path) = args.export_scene {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|e| {
                    CliError::Execution(format!(
                        "failed creating scene export dir {}: {e}",
                        parent.display()
                    ))
                })?;
            }
        }
        let scene_json = serde_json::to_string_pretty(&vm.scene)
            .map_err(|e| CliError::Execution(format!("failed serializing OverlayScene: {e}")))?;
        fs::write(path, scene_json).map_err(|e| {
            CliError::Execution(format!("failed writing scene to {}: {e}", path.display()))
        })?;
    }

    let rendered = if args.json {
        serde_json::to_string_pretty(&vm)
            .map_err(|e| CliError::Execution(format!("failed serializing study-loop JSON: {e}")))?
    } else {
        let mut txt = format_strategy_loop_cli_report(&vm, args.sweep, args.hedge);
        if let Some(ref path) = args.export_scene {
            txt.push_str(&format!(
                "\nExported OverlayScene JSON -> {}\n",
                path.display()
            ));
        }
        txt
    };

    Ok(CommandOutcome::StudyLoopReport(rendered))
}
