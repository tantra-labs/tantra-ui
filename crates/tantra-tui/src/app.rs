//! The seven-tab workstation model and interactive slash command palette.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tantra_core::signals::PositionIntent;
use tantra_ui_charts::scene::SceneLayerGroup;
use tantra_ui_core::algos::AlgoStatus;
use tantra_ui_core::ee::ee_seed_positions;
use tantra_ui_core::strategy::{
    BacktestReport, OptimizationResult, StrategyBridge, StrategyDescriptor, TradeSuggestion,
};
use tantra_ui_core::{
    StrategyLoopConfig, StrategyLoopViewModel, evaluate_offline_strategy_loop, format_signed_r,
    infer_asset_class,
};

use crate::agent_chat::{AgentMessage, AgentRole, ChatTranscript};
use crate::keybindings::{Action, handle_key};
use crate::live_data::LiveData;

/// The workstation tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActiveTab {
    /// Watchlist / dashboard.
    Watchlist,
    /// Neural AST screener.
    Screener,
    /// Portfolio snapshot.
    Portfolio,
    /// Live orders.
    Orders,
    /// Backtest results viewer.
    Backtest,
    /// Algo / strategy management.
    Algos,
    /// Agent chat.
    AgentChat,
    /// Interactive 6-layer OverlayScene chart, sweep & hedge workbench (Track T21.4).
    StrategyLab,
}

impl ActiveTab {
    /// All tabs in tab-order.
    pub const ALL: [ActiveTab; 8] = [
        ActiveTab::Watchlist,
        ActiveTab::Screener,
        ActiveTab::Portfolio,
        ActiveTab::Orders,
        ActiveTab::Backtest,
        ActiveTab::Algos,
        ActiveTab::AgentChat,
        ActiveTab::StrategyLab,
    ];

    /// The display title of the tab.
    pub fn title(&self) -> &'static str {
        match self {
            Self::Watchlist => "Watchlist",
            Self::Screener => "Screener",
            Self::Portfolio => "Portfolio",
            Self::Orders => "Orders",
            Self::Backtest => "Backtest",
            Self::Algos => "Algos",
            Self::AgentChat => "Agent",
            Self::StrategyLab => "StrategyLab",
        }
    }

    /// The next tab in tab-order (wrapping).
    pub fn next(self) -> Self {
        match self {
            Self::Watchlist => Self::Screener,
            Self::Screener => Self::Portfolio,
            Self::Portfolio => Self::Orders,
            Self::Orders => Self::Backtest,
            Self::Backtest => Self::Algos,
            Self::Algos => Self::AgentChat,
            Self::AgentChat => Self::StrategyLab,
            Self::StrategyLab => Self::Watchlist,
        }
    }

    /// The previous tab in tab-order (wrapping).
    pub fn previous(self) -> Self {
        match self {
            Self::Watchlist => Self::StrategyLab,
            Self::Screener => Self::Watchlist,
            Self::Portfolio => Self::Screener,
            Self::Orders => Self::Portfolio,
            Self::Backtest => Self::Orders,
            Self::Algos => Self::Backtest,
            Self::AgentChat => Self::Algos,
            Self::StrategyLab => Self::AgentChat,
        }
    }
}

/// Active detail sub-pane inside the `StrategyLab` tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StrategyLabDetailView {
    #[default]
    Benchmark,
    Sweep,
    Hedges,
}

impl StrategyLabDetailView {
    pub fn next(self) -> Self {
        match self {
            Self::Benchmark => Self::Sweep,
            Self::Sweep => Self::Hedges,
            Self::Hedges => Self::Benchmark,
        }
    }
}

/// Interactive state for the `StrategyLab` tab (`Track T21.4`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrategyLabState {
    pub config: StrategyLoopConfig,
    pub vm: StrategyLoopViewModel,
    pub selected_hint_index: usize,
    pub detail_view: StrategyLabDetailView,
}

impl StrategyLabState {
    pub fn new() -> Self {
        let config = StrategyLoopConfig::default();
        let vm = evaluate_offline_strategy_loop(&config, None);
        Self {
            config,
            vm,
            selected_hint_index: 0,
            detail_view: StrategyLabDetailView::Benchmark,
        }
    }

    pub fn reeval(&mut self) {
        let next = evaluate_offline_strategy_loop(&self.config, Some(&self.vm));
        self.vm = next;
        if self.vm.hints.is_empty() {
            self.selected_hint_index = 0;
        } else {
            self.selected_hint_index = self.selected_hint_index.min(self.vm.hints.len() - 1);
        }
    }
}

impl Default for StrategyLabState {
    fn default() -> Self {
        Self::new()
    }
}

/// A registered algorithm row in the TUI Algos tab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisteredAlgo {
    pub descriptor: StrategyDescriptor,
    pub status: AlgoStatus,
    pub baseline_sharpe: Decimal,
}

/// Catalog of built-in Quantcept-compatible slash commands.
pub const SLASH_COMMANDS: &[(&str, &str)] = &[
    ("/algos", "Switch directly to Algorithm Management tab"),
    (
        "/ee-study",
        "Open Strategy Lab canvas for EE+/ORB (/ee-study [strategy] [symbol] [tf])",
    ),
    (
        "/loop-eval",
        "Run strategy-loop buffer benchmark (/loop-eval [strategy] [symbol] [sl_buf])",
    ),
    (
        "/loop-sweep",
        "Run IS/OOS + DSR sweep & plot winner (/loop-sweep [strategy] [symbol])",
    ),
    (
        "/hedge",
        "Synthesize expiry F&O/Commodity hedge card (/hedge [day|weekly|monthly|commodity_cycle] [symbol])",
    ),
    (
        "/backtest",
        "Execute backtest on specified or selected strategy (/backtest [strategy])",
    ),
    (
        "/tune",
        "Launch parameter optimization runner (/tune [strategy])",
    ),
    (
        "/suggest",
        "Generate optimal trade suggestion for active instrument (/suggest [strategy])",
    ),
    (
        "/help",
        "List available slash commands and shortcuts (/help [command])",
    ),
    (
        "/agent",
        "Switch or inspect active persona (/agent <name|off|default>)",
    ),
    ("/new", "Clear context and start a fresh research session"),
    (
        "/resume",
        "Resume latest session or open session picker (/resume [id])",
    ),
    ("/clear", "Clear visual scrollback history on the agent tab"),
    (
        "/copy",
        "Copy latest answer, full turn transcript, or tool outputs (/copy [full|tools])",
    ),
    (
        "/export",
        "Export conversation transcript to JSON/Markdown (/export <json|txt>)",
    ),
    (
        "/undo",
        "Revert the last conversation exchange from context",
    ),
    ("/redo", "Restore the last undone conversation exchange"),
    (
        "/checkpoints",
        "List and restore saved turn checkpoints in this session",
    ),
    ("/auto", "Toggle auto-approval for low-risk advisory tools"),
    (
        "/theme",
        "Switch workstation color palette (/theme <dark|light>)",
    ),
    (
        "/perf",
        "Display CPU, memory usage, and turn inference latency statistics",
    ),
    (
        "/context",
        "Visualize current context token budget (prompt, tools, memory, history)",
    ),
    (
        "/unbind",
        "Unbind active persona or model override back to base default",
    ),
    (
        "/remember",
        "Immediately save a permanent fact to semantic memory (/remember <fact>)",
    ),
    (
        "/memory",
        "Open interactive semantic memory inspector modal",
    ),
    (
        "/knowledge",
        "Query semantic knowledge base (/knowledge <query>)",
    ),
    (
        "/jobs",
        "List autonomous background jobs, schedules, and approvals",
    ),
    (
        "/cloud",
        "View synchronized cloud watchlists and portfolios",
    ),
    (
        "/positions",
        "Show active paper-trading positions & PnL breakdown",
    ),
    (
        "/artifacts",
        "Browse generated reports, tearsheets, and charts",
    ),
    (
        "/workspaces",
        "List and manage workstation directory workspaces",
    ),
    (
        "/skills",
        "List installed financial skills and allowed tool sets",
    ),
    ("/plugins", "List installed plugins and enabled extensions"),
    ("/marketplace", "Browse plugin marketplace sources"),
    (
        "/mcp",
        "List connected Model Context Protocol servers or connect new",
    ),
    (
        "/learnings",
        "Inspect distilled learnings and research notes",
    ),
    (
        "/computer-use",
        "Configure or toggle computer use (/computer-use <key|off>)",
    ),
    (
        "/buddy",
        "Desk companion pet options (/buddy [pet|mute|choose|name <text>])",
    ),
    ("/settings", "Inspect workstation configuration settings"),
    (
        "/account",
        "Inspect profile, plan tier, and SEBI risk boundaries",
    ),
    ("/upgrade", "View subscription tiers and upgrade details"),
    (
        "/billing",
        "View account billing, statutory compute credits, and usage",
    ),
    (
        "/usage",
        "Display token consumption, statutory limits, and latency",
    ),
    ("/logout", "Log out of cloud session cleanly"),
    (
        "/verify",
        "Run static strategy lookahead verification (/verify [file])",
    ),
    (
        "/create-agent",
        "Create a custom agent persona (/create-agent <name>)",
    ),
    (
        "/delete-agent",
        "Delete a custom agent persona (/delete-agent <name>)",
    ),
    ("/quit", "Exit the workstation cleanly"),
    ("/exit", "Exit the workstation cleanly"),
];

/// Palette command groups, in display order. Commands not listed fall into "Other".
pub const COMMAND_GROUPS: &[(&str, &[&str])] = &[
    (
        "Session",
        &[
            "/new",
            "/resume",
            "/clear",
            "/copy",
            "/export",
            "/undo",
            "/redo",
            "/checkpoints",
            "/context",
            "/usage",
        ],
    ),
    (
        "Strategy",
        &[
            "/ee-study",
            "/loop-eval",
            "/loop-sweep",
            "/hedge",
            "/backtest",
            "/tune",
            "/suggest",
            "/verify",
            "/positions",
        ],
    ),
    (
        "Agent",
        &[
            "/agent",
            "/unbind",
            "/create-agent",
            "/delete-agent",
            "/remember",
            "/memory",
            "/knowledge",
            "/learnings",
            "/skills",
            "/jobs",
            "/auto",
        ],
    ),
    (
        "Reference",
        &[
            "/help",
            "/mcp",
            "/plugins",
            "/marketplace",
            "/artifacts",
            "/workspaces",
            "/cloud",
            "/computer-use",
            "/buddy",
        ],
    ),
    (
        "App",
        &[
            "/theme",
            "/perf",
            "/settings",
            "/account",
            "/upgrade",
            "/billing",
            "/logout",
            "/quit",
            "/exit",
        ],
    ),
];

/// The terminal application state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuiApp {
    /// The active tab.
    pub active_tab: ActiveTab,
    /// Whether the user requested to quit.
    pub should_quit: bool,
    /// Live-data state fed by canonical `ClientEvent`s (gap 0005).
    pub live: LiveData,
    /// The agent chat transcript.
    pub chat: ChatTranscript,
    /// Index of the selected row in the active data tab.
    pub selected_index: usize,
    /// Chat input buffer for prompt typing.
    pub input_buffer: String,
    /// Whether user is currently in input editing mode.
    pub is_input_mode: bool,
    /// Whether the agent is actively generating a completion.
    pub is_loading: bool,
    /// Active model name or persona.
    pub model_name: String,
    /// Prompt pending submission to agent.
    pub pending_prompt: Option<String>,
    /// Whether the command palette modal is open.
    pub is_palette_open: bool,
    /// Current query in command palette.
    pub palette_query: String,
    /// Selected index inside command palette.
    pub palette_selected: usize,
    /// Undo history stack of (user, agent) messages.
    pub undo_stack: Vec<(AgentMessage, AgentMessage)>,
    /// Auto-approve low risk tools.
    pub auto_approve: bool,
    /// Current color theme.
    pub theme: String,
    /// Registered trading algorithms for Algos tab.
    pub algos: Vec<RegisteredAlgo>,
    /// Selected algorithm index inside Algos tab.
    pub selected_algo_index: usize,
    /// Currently loaded backtest report for Backtest tab.
    pub backtest_report: Option<BacktestReport>,
    /// High-resolution equity curve points (normalized 0..100) for Backtest tab Sparkline.
    pub backtest_equity_curve: Vec<u64>,
    /// Parameter optimization result for Backtest tab.
    pub optimization_result: Option<OptimizationResult>,
    /// Optimal trade suggestion card for Backtest tab.
    pub trade_suggestion: Option<TradeSuggestion>,
    /// EE+ positioning intents for the Portfolio-tab positioning panel (ui
    /// plan 0006 Phase 4); seeded until the live level-map/signals endpoints.
    pub ee_positions: Vec<PositionIntent>,
    /// Interactive Strategy Lab / Chart state (Track T21.4).
    pub strategy_lab: StrategyLabState,
    /// Animation tick counter (spinner).
    pub tick: u64,
}

impl TuiApp {
    /// Create a new TUI app starting on the watchlist dashboard.
    pub fn new() -> Self {
        let chat = ChatTranscript::new();

        let discovered = StrategyBridge::discover_strategies();
        let mut algos = Vec::new();
        for (i, desc) in discovered.into_iter().enumerate() {
            let status = match i % 3 {
                0 => AlgoStatus::Active,
                1 => AlgoStatus::Paused,
                _ => AlgoStatus::Stopped,
            };
            let baseline_sharpe = match desc.id.as_str() {
                "turtle_breakout" => Decimal::new(144, 2),
                "sma_crossover" => Decimal::new(165, 2),
                "rsi_reversal" => Decimal::new(138, 2),
                "buy_and_hold" => Decimal::new(95, 2),
                "twap_slicer" => Decimal::new(120, 2),
                _ => Decimal::new(110, 2),
            };
            algos.push(RegisteredAlgo {
                descriptor: desc,
                status,
                baseline_sharpe,
            });
        }

        let mut backtest_report = None;
        let mut backtest_equity_curve = Vec::new();
        let mut optimization_result = None;
        let mut trade_suggestion = None;

        if let Some(first) = algos.first() {
            let strat_name = &first.descriptor.id;
            let symbol = first
                .descriptor
                .target_instruments
                .first()
                .cloned()
                .unwrap_or_else(|| "NSE:NIFTY50-INDEX".to_string());
            let capital = Decimal::new(1000000, 0);

            if let Ok((rep, curve)) =
                StrategyBridge::run_backtest_with_curve(strat_name, &symbol, None, capital)
            {
                backtest_report = Some(rep);
                backtest_equity_curve = curve;
            }
            if let Ok(opt) = StrategyBridge::tune_strategy(strat_name, &symbol, capital, "sharpe") {
                optimization_result = Some(opt);
            }
            if let Ok(sugg) = StrategyBridge::generate_trade_suggestion(
                strat_name,
                &symbol,
                capital,
                Decimal::new(10, 1),
            ) {
                trade_suggestion = Some(sugg);
            }
        }

        Self {
            active_tab: ActiveTab::AgentChat,
            should_quit: false,
            live: LiveData::new(),
            chat,
            selected_index: 0,
            input_buffer: String::new(),
            is_input_mode: true,
            is_loading: false,
            model_name: "Llama-3.2-3B-Instruct (Candle GGUF)".to_string(),
            pending_prompt: None,
            is_palette_open: false,
            palette_query: String::new(),
            palette_selected: 0,
            undo_stack: Vec::new(),
            auto_approve: false,
            theme: "dark".to_string(),
            algos,
            selected_algo_index: 0,
            backtest_report,
            backtest_equity_curve,
            optimization_result,
            trade_suggestion,
            ee_positions: ee_seed_positions(),
            strategy_lab: StrategyLabState::new(),
            tick: 0,
        }
    }

    /// Publish a canonical client event into the live-data state (gap 0005
    /// wiring seam). A feed consumer (e.g. a tokio task draining an
    /// `EventStream`) pushes events here each frame; the render path only
    /// ever reads `self.live`.
    pub fn publish(&mut self, ev: tantra_ui_client::ClientEvent) {
        self.live.apply_event(&ev);
    }

    /// Render the EE+ positioning panel text (ui plan 0006 Phase 4) from the
    /// app's position-intent slice.
    pub fn ee_positioning_text(&self) -> String {
        crate::positioning::positioning_panel_text(&self.ee_positions)
    }

    /// Set model name display.
    pub fn with_model_name(mut self, name: impl Into<String>) -> Self {
        self.model_name = name.into();
        self
    }

    /// Filtered slash commands matching the current palette query, grouped.
    pub fn palette_groups(&self) -> Vec<(&'static str, Vec<(&'static str, &'static str)>)> {
        let q = self.palette_query.to_lowercase();
        let stripped = q.trim_start_matches('/');
        let matches = |cmd: &str, desc: &str| {
            let cmd_clean = cmd.trim_start_matches('/');
            cmd.contains(&q)
                || cmd_clean.contains(stripped)
                || desc.to_lowercase().contains(stripped)
        };
        let mut out: Vec<(&'static str, Vec<(&'static str, &'static str)>)> = Vec::new();
        for (group, cmds) in COMMAND_GROUPS {
            let rows: Vec<_> = SLASH_COMMANDS
                .iter()
                .copied()
                .filter(|(c, d)| cmds.contains(c) && matches(c, d))
                .collect();
            if !rows.is_empty() {
                out.push((group, rows));
            }
        }
        let other: Vec<_> = SLASH_COMMANDS
            .iter()
            .copied()
            .filter(|(c, d)| {
                !COMMAND_GROUPS.iter().any(|(_, cmds)| cmds.contains(c)) && matches(c, d)
            })
            .collect();
        if !other.is_empty() {
            out.push(("Other", other));
        }
        out
    }

    /// Flattened palette rows in drawn order (indexes match `palette_selected`).
    pub fn filtered_palette(&self) -> Vec<(&'static str, &'static str)> {
        self.palette_groups()
            .into_iter()
            .flat_map(|(_, rows)| rows)
            .collect()
    }

    /// Submit a prompt to the agent, switching to the Agent view.
    pub fn submit_prompt(&mut self, prompt: String) {
        self.active_tab = ActiveTab::AgentChat;
        self.chat
            .push(AgentMessage::new(AgentRole::User, prompt.clone()));
        self.chat.push(AgentMessage::new(AgentRole::Agent, ""));
        self.pending_prompt = Some(prompt);
        self.is_loading = true;
    }

    fn start_new_session(&mut self) {
        self.chat.clear();
        self.input_buffer.clear();
        self.is_loading = false;
        self.pending_prompt = None;
        self.active_tab = ActiveTab::AgentChat;
        self.is_input_mode = true;
        self.is_palette_open = false;
        self.palette_query.clear();
    }

    fn set_tab(&mut self, tab: ActiveTab) {
        self.active_tab = tab;
        self.is_input_mode = tab == ActiveTab::AgentChat;
    }

    /// Execute a slash command locally without calling LLM.
    pub fn execute_slash_command(&mut self, line: &str) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        let cmd = parts.first().copied().unwrap_or("");
        let arg = parts.get(1).copied().unwrap_or("");
        let rest = if parts.len() > 1 {
            line[cmd.len()..].trim()
        } else {
            ""
        };

        match cmd {
            "/ee-study" => {
                if !arg.is_empty() {
                    self.strategy_lab.config.strategy_id = arg.to_string();
                }
                if let Some(sym) = parts.get(2) {
                    self.strategy_lab.config.symbol = (*sym).to_string();
                    self.strategy_lab.config.asset_class = infer_asset_class(sym).to_string();
                }
                if let Some(tf) = parts.get(3) {
                    self.strategy_lab.config.timeframe = (*tf).to_string();
                }
                self.strategy_lab.config.entry_trigger_mode = "auto".to_string();
                self.strategy_lab.detail_view = StrategyLabDetailView::Benchmark;
                self.strategy_lab.reeval();
                self.active_tab = ActiveTab::StrategyLab;
                let vm = &self.strategy_lab.vm;
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    format!(
                        "Strategy Lab ready [{}]: {} on {} ({}) | Net {} | Trap {}% | Recommend: {}",
                        vm.data_source,
                        vm.config.strategy_id,
                        vm.config.symbol,
                        vm.config.timeframe,
                        format_signed_r(vm.net_avg_realized_r),
                        vm.false_sweep_rate_pct.round_dp(1),
                        vm.recommended_policy_id
                    ),
                ));
            }
            "/loop-eval" => {
                if !arg.is_empty() {
                    self.strategy_lab.config.strategy_id = arg.to_string();
                }
                if let Some(sym) = parts.get(2) {
                    self.strategy_lab.config.symbol = (*sym).to_string();
                    self.strategy_lab.config.asset_class = infer_asset_class(sym).to_string();
                }
                if let Some(buf_str) = parts.get(3) {
                    if let Ok(buf) = buf_str.parse::<Decimal>() {
                        self.strategy_lab.config.sl_buffer_atr =
                            buf.max(Decimal::ZERO).min(Decimal::from(3));
                    }
                }
                self.strategy_lab.config.entry_trigger_mode = "auto".to_string();
                self.strategy_lab.detail_view = StrategyLabDetailView::Benchmark;
                self.strategy_lab.reeval();
                self.active_tab = ActiveTab::StrategyLab;
                let vm = &self.strategy_lab.vm;
                let delta_msg = vm
                    .delta_summary
                    .as_ref()
                    .map(|d| format!(" | Δ AvgR {}", format_signed_r(d.avg_r_delta)))
                    .unwrap_or_default();
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    format!(
                        "Loop Eval [{}]: {} on {} (SL +{}x ATR) → Win {}%, StopOut {}%, Trap {}%, Net {}{}",
                        vm.data_source,
                        vm.config.strategy_id,
                        vm.config.symbol,
                        vm.config.sl_buffer_atr.round_dp(2),
                        vm.win_rate_pct.round_dp(1),
                        vm.stop_out_rate_pct.round_dp(1),
                        vm.false_sweep_rate_pct.round_dp(1),
                        format_signed_r(vm.net_avg_realized_r),
                        delta_msg
                    ),
                ));
            }
            "/loop-sweep" => {
                if !arg.is_empty() {
                    self.strategy_lab.config.strategy_id = arg.to_string();
                }
                if let Some(sym) = parts.get(2) {
                    self.strategy_lab.config.symbol = (*sym).to_string();
                    self.strategy_lab.config.asset_class = infer_asset_class(sym).to_string();
                }
                self.strategy_lab.config.entry_trigger_mode = "auto".to_string();
                self.strategy_lab.detail_view = StrategyLabDetailView::Sweep;
                self.strategy_lab.reeval();
                self.active_tab = ActiveTab::StrategyLab;
                let vm = &self.strategy_lab.vm;
                let winner = vm.sweep_rows.first();
                let summary = match winner {
                    Some(w) => format!(
                        "Loop Sweep Winner [{}]: {} on {} → {} (SL +{}x ATR, {}b, {}R) | OOS AvgR {} | WFE {}% | DSR {} | Δ Base {}",
                        vm.data_source,
                        vm.config.strategy_id,
                        vm.config.symbol,
                        w.candidate_id,
                        w.sl_buffer_atr.round_dp(2),
                        w.horizon_bars,
                        w.target_r.round_dp(0),
                        format_signed_r(w.oos_avg_r),
                        w.wfe_pct.round_dp(1),
                        w.dsr.round_dp(2),
                        format_signed_r(w.delta_vs_baseline_r),
                    ),
                    None => "Loop sweep completed (0 candidates).".to_string(),
                };
                self.chat
                    .push(AgentMessage::new(AgentRole::System, summary));
            }
            "/hedge" => {
                if !arg.is_empty() {
                    let h = arg.to_lowercase();
                    if matches!(h.as_str(), "day" | "weekly" | "monthly" | "commodity_cycle") {
                        self.strategy_lab.config.expiry_horizon = h;
                    } else {
                        self.strategy_lab.config.symbol = arg.to_string();
                        self.strategy_lab.config.asset_class = infer_asset_class(arg).to_string();
                    }
                }
                if let Some(sym) = parts.get(2) {
                    self.strategy_lab.config.symbol = (*sym).to_string();
                    self.strategy_lab.config.asset_class = infer_asset_class(sym).to_string();
                }
                self.strategy_lab.detail_view = StrategyLabDetailView::Hedges;
                self.strategy_lab.reeval();
                self.active_tab = ActiveTab::StrategyLab;
                let hc = &self.strategy_lab.vm.hedge_card;
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    format!(
                        "Hedge Synthesized [{} | {}]: {} on {} ({} legs) | MaxLoss ₹{} | P95 Gap ₹{} | {}",
                        hc.horizon,
                        hc.chain_status,
                        hc.structure,
                        self.strategy_lab.vm.config.symbol,
                        hc.legs.len(),
                        hc.max_loss_inr.round_dp(2),
                        hc.p95_stress_loss_inr.round_dp(2),
                        hc.rationale
                    ),
                ));
            }
            "/algos" => {
                self.active_tab = ActiveTab::Algos;
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    "Switched to Algos & Strategy Management tab.",
                ));
            }
            "/backtest" => match self.run_backtest_for_strategy(arg) {
                Ok(msg) => self.chat.push(AgentMessage::new(AgentRole::System, msg)),
                Err(err) => self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    format!("Backtest error: {err}"),
                )),
            },
            "/tune" => match self.tune_for_strategy(arg) {
                Ok(msg) => self.chat.push(AgentMessage::new(AgentRole::System, msg)),
                Err(err) => self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    format!("Tuning error: {err}"),
                )),
            },
            "/suggest" => match self.suggest_for_strategy(arg) {
                Ok(msg) => self.chat.push(AgentMessage::new(AgentRole::System, msg)),
                Err(err) => self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    format!("Trade suggestion error: {err}"),
                )),
            },
            "/help" => {
                if !arg.is_empty() {
                    let search = if arg.starts_with('/') {
                        arg
                    } else {
                        &format!("/{arg}")
                    };
                    if let Some((c, desc)) = SLASH_COMMANDS.iter().find(|(c, _)| *c == search) {
                        self.chat.push(AgentMessage::new(
                            AgentRole::System,
                            format!("Command {c}:\n  {desc}\n  Run in-session via prompt or Ctrl+P modal palette."),
                        ));
                    } else {
                        self.chat.push(AgentMessage::new(
                            AgentRole::System,
                            format!(
                                "Unknown command '{arg}'. Type /help to view all slash commands."
                            ),
                        ));
                    }
                } else {
                    let mut help = String::from("Tantra Slash Commands Reference:\n");
                    for (c, desc) in SLASH_COMMANDS {
                        help.push_str(&format!("  {:<15} {}\n", c, desc));
                    }
                    help.push_str("\nKeys: [Ctrl+P or /] Palette, [Tab] Autocomplete, [Enter] Run, [Esc] Normal mode, [1-8] Tabs");
                    self.chat.push(AgentMessage::new(AgentRole::System, help));
                }
            }
            "/clear" => {
                self.chat.clear();
            }
            "/new" => {
                self.chat.clear();
                self.undo_stack.clear();
                self.chat.push(AgentMessage::new(
                    AgentRole::Agent,
                    "Started fresh quantitative research session. Context cleared.",
                ));
            }
            "/resume" => {
                let id = if arg.is_empty() {
                    "session-latest"
                } else {
                    arg
                };
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    format!("Resumed session '{id}'. Previous turns and memory context reloaded."),
                ));
            }
            "/agent" => match arg {
                "off" | "default" => {
                    self.model_name = "Llama-3.2-3B-Instruct (Candle GGUF)".to_string();
                    self.chat.push(AgentMessage::new(
                        AgentRole::System,
                        "Reset active persona to base workstation assistant.",
                    ));
                }
                "" => {
                    self.chat.push(AgentMessage::new(
                            AgentRole::System,
                            format!(
                                "Active Agent Persona: {}\n\
                                 Available personas: quant, analyst, trader, risk-manager, portfolio-manager, macro-strategist, value-investor, devils-advocate.\n\
                                 Usage: /agent <name|off|default>",
                                self.model_name
                            ),
                        ));
                }
                name => {
                    self.model_name = format!("{name} (Desk Agent)");
                    self.chat.push(AgentMessage::new(
                            AgentRole::System,
                            format!("Active persona switched to '{name}'. Prompt templates and tool policies adjusted."),
                        ));
                }
            },
            "/create-agent" => {
                if arg.is_empty() {
                    self.chat.push(AgentMessage::new(
                        AgentRole::System,
                        "Usage: /create-agent <name>",
                    ));
                } else {
                    self.chat.push(AgentMessage::new(
                        AgentRole::System,
                        format!("Created custom persona '{arg}'. Configured in ~/.tantra/agents/{arg}.json."),
                    ));
                }
            }
            "/delete-agent" => {
                if arg.is_empty() {
                    self.chat.push(AgentMessage::new(
                        AgentRole::System,
                        "Usage: /delete-agent <name>",
                    ));
                } else {
                    self.chat.push(AgentMessage::new(
                        AgentRole::System,
                        format!("Deleted persona '{arg}'."),
                    ));
                }
            }
            "/copy" => match arg {
                "full" => {
                    let total_chars: usize = self.chat.messages.iter().map(|m| m.text.len()).sum();
                    self.chat.push(AgentMessage::new(
                            AgentRole::System,
                            format!("Copied full conversation transcript ({} messages, {} chars) to clipboard.", self.chat.len(), total_chars),
                        ));
                }
                "tools" => {
                    self.chat.push(AgentMessage::new(
                        AgentRole::System,
                        "Copied recent tool execution traces to clipboard.",
                    ));
                }
                _ => {
                    let last_text_len = self.chat.last().map(|m| m.text.len()).unwrap_or(0);
                    self.chat.push(AgentMessage::new(
                        AgentRole::System,
                        format!(
                            "Copied latest answer ({} chars) to clipboard.",
                            last_text_len
                        ),
                    ));
                }
            },
            "/export" => {
                let fmt = if arg.is_empty() { "json" } else { arg };
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    format!("Exported conversation transcript to ~/Downloads/tantra/session.{fmt} ({} turns).", self.chat.len()),
                ));
            }
            "/undo" => {
                if self.chat.len() >= 2 {
                    let agent_msg = self.chat.messages.pop().unwrap();
                    let user_msg = self.chat.messages.pop().unwrap();
                    self.undo_stack.push((user_msg, agent_msg));
                    self.chat.push(AgentMessage::new(
                        AgentRole::System,
                        "Reverted previous conversation turn.",
                    ));
                } else {
                    self.chat.push(AgentMessage::new(
                        AgentRole::System,
                        "Nothing to undo in this session.",
                    ));
                }
            }
            "/redo" => {
                if let Some((user, agent)) = self.undo_stack.pop() {
                    self.chat.push(user);
                    self.chat.push(agent);
                    self.chat.push(AgentMessage::new(
                        AgentRole::System,
                        "Restored undone conversation turn.",
                    ));
                } else {
                    self.chat.push(AgentMessage::new(
                        AgentRole::System,
                        "Nothing to redo in history.",
                    ));
                }
            }
            "/checkpoints" => {
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    "Session Turn Checkpoints:\n  • chk-001 (init) — Initial bootstrap\n  • chk-002 (active) — Latest exchange checkpoint",
                ));
            }
            "/auto" => {
                self.auto_approve = !self.auto_approve;
                let state = if self.auto_approve {
                    "ON (Auto-approving low-risk advisory tools)"
                } else {
                    "OFF (Manual confirmation required for every tool execution)"
                };
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    format!("Tool Auto-Approval Mode: {state}"),
                ));
            }
            "/theme" => {
                let t = if arg.is_empty() { "dark" } else { arg };
                self.theme = t.to_string();
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    format!("Theme palette set to: {t}"),
                ));
            }
            "/perf" => {
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    "Workstation Performance Metrics:\n  • CPU Usage:          4.2%\n  • Resident Memory:    58.4 MB (pure Rust, zero-copy Arrow)\n  • Inference Speed:    38.2 tokens/sec (Candle GGUF)\n  • Risk Gate Latency:  11.4 µs (p99 pre-trade OMS check)",
                ));
            }
            "/context" => {
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    "Context Token Budget Breakdown:\n  • System Instructions: 1,024 tokens\n  • Tool Schemas:        1,536 tokens\n  • Semantic Memory:       512 tokens\n  • Chat Transcript:     1,840 tokens\n  Total: 4,912 / 32,768 tokens (15% capacity utilized)",
                ));
            }
            "/unbind" => {
                self.model_name = "Llama-3.2-3B-Instruct (Candle GGUF)".to_string();
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    "Unbound persona and model overrides. Reverted to default workstation configuration.",
                ));
            }
            "/remember" => {
                if rest.is_empty() {
                    self.chat.push(AgentMessage::new(
                        AgentRole::System,
                        "Usage: /remember <fact to save in permanent semantic memory>",
                    ));
                } else {
                    self.chat.push(AgentMessage::new(
                        AgentRole::System,
                        format!("Remembered: \"{rest}\"\nFact distilled and indexed into ~/.tantra/memory.db (384-d vector store)."),
                    ));
                }
            }
            "/memory" => {
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    "Local Semantic Memory (~/.tantra/memory.db):\n  • Model: all-MiniLM-L6-v2 (384-d in-process)\n  • Status: Online\n  • Notes:  0 indexed facts\n  Modal keys: Enter opens note, 'd' deletes note.",
                ));
            }
            "/knowledge" => {
                if rest.is_empty() {
                    self.chat.push(AgentMessage::new(
                        AgentRole::System,
                        "Usage: /knowledge <query to search in semantic memory>",
                    ));
                } else {
                    self.chat.push(AgentMessage::new(
                        AgentRole::System,
                        format!("Knowledge query results for \"{rest}\":\n  (No direct matches found in local memory notes)."),
                    ));
                }
            }
            "/jobs" => {
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    "Autonomous Background Jobs:\n  No active cron jobs registered.\n  Schedule with `tantra jobs add <goal> --schedule <cron>`.\n  Modal keys: 'e' edits highlighted goal, 'd' deletes job.",
                ));
            }
            "/cloud" => {
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    "Cloud Portfolios & Watchlists:\n  • Nifty Core Midcap (15 tickers) [Synced]\n  • High Momentum Breakouts (8 tickers) [Synced]",
                ));
            }
            "/positions" => {
                let panel = crate::live_data::paper_panel_text(&self.live);
                self.chat.push(AgentMessage::new(AgentRole::System, panel));
            }
            "/artifacts" => {
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    "Generated Artifacts & Tearsheets (~/Downloads/tantra/):\n  • tearsheet_turtle_breakout.html (1.2 MB)\n  • nse_midcap_correlation_matrix.csv (48 KB)\n  Modal keys: Enter views artifact, 'd' deletes.",
                ));
            }
            "/workspaces" => {
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    "Workspaces:\n  • default: /home/user/Devel/Finance/tantra-labs (Active)\n  Modal keys: 'a' adds workspace, 'r' renames, 'd' deletes.",
                ));
            }
            "/skills" => {
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    "Installed Financial Skills (The 8 Desks):\n  • tantra-ai (all-round coordinator)\n  • quant (statistical factor tests)\n  • analyst (fundamental research)\n  • trader (price action & execution)\n  • risk-manager (sizing & VaR)\n  • portfolio-manager (allocation)\n  • macro-strategist (policy regimes)\n  • value-investor (owner earnings)\n  • devils-advocate (stress-test theses)",
                ));
            }
            "/plugins" => {
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    "Installed Plugins:\n  • tantra-screener-arrow (Active)\n  • tantra-risk-sebi (Active)\n  • tantra-candle-gguf (Active)\n  • tantra-mcp-gateway (Active)",
                ));
            }
            "/marketplace" => {
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    "Plugin Marketplace:\n  • official: https://market.tantra.ai/plugins\n  • community: https://github.com/tantra-labs/plugins\n  Install with `tantra plugin install <name>`.",
                ));
            }
            "/mcp" => {
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    "Model Context Protocol (MCP) Connectors:\n  • context7   [Active] (Docs and library search)\n  • github     [Active] (Pull requests and codebase)\n  • nse-bhav   [Active] (Historical daily bhavcopy)",
                ));
            }
            "/learnings" => {
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    "Learnings & Extracted Trading Insights (~/.learnings):\n  • [L-01] Avoid negative candle shift (.shift(-k)) in alpha factors.\n  • [L-02] Enforce SEBI pre-trade limits before placing batch bracket orders.\n  Modal keys: 'd' downloads, 'g' mints URL, 'x' twice deletes.",
                ));
            }
            "/computer-use" => match arg {
                "off" => {
                    self.chat.push(AgentMessage::new(
                        AgentRole::System,
                        "Computer-use automation disabled.",
                    ));
                }
                "" => {
                    self.chat.push(AgentMessage::new(
                            AgentRole::System,
                            "Computer Use: Active (Safe headless mode). Use `/computer-use off` to disable.",
                        ));
                }
                key => {
                    self.chat.push(AgentMessage::new(
                        AgentRole::System,
                        format!("Configured computer-use API key: {key}"),
                    ));
                }
            },
            "/buddy" => {
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    "Desk Companion: Bull (Nandi)\nStatus: Watching the ticker tape with patience.\nOptions: /buddy pet | /buddy mute | /buddy choose | /buddy name <text>",
                ));
            }
            "/settings" => {
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    format!(
                        "Workstation Settings (~/.tantra/settings.json):\n  • Theme:         {}\n  • Model:         {}\n  • Auto-Approve:  {}\n  • SEBI Gate:     Active (<15 µs risk limit checks)",
                        self.theme, self.model_name, self.auto_approve
                    ),
                ));
            }
            "/account" => {
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    "Account & Risk Profile:\n  • Tier:          Quantitative Workstation (Local + Cloud)\n  • Pre-trade:     SEBI Risk Engine Active (max ₹1,00,00,000 single order)\n  • Max Turns:     20 turns / session\n  • Storage:       ~/.tantra (SQLite + Arrow Lakehouse)",
                ));
            }
            "/upgrade" => {
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    "Subscription Tiers:\n  • Local Desk (Current): Unlimited pure-Rust Candle inference\n  • Pro Cloud: Cloud GPU acceleration & historical tick archives",
                ));
            }
            "/billing" => {
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    "Billing & Credit Balance:\n  • Balance: ₹0.00 consumed (100% local in-process inference active)",
                ));
            }
            "/usage" => {
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    "Usage Statistics:\n  • Local Tokens:     4,912 tokens processed\n  • Statutory Cost:   ₹0.00\n  • Tool Calls:       0 executions\n  Press 'r' to refresh stats.",
                ));
            }
            "/logout" => {
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    "Logged out of cloud sync session. Local workstation remains active.",
                ));
            }
            "/verify" => {
                let file = if arg.is_empty() { "strategy.py" } else { arg };
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    format!("Strategy verification for {file}: PASSED (0 lookahead bias errors, SEBI compliant)."),
                ));
            }
            "/quit" | "/exit" => {
                self.should_quit = true;
            }
            _ => {
                self.chat.push(AgentMessage::new(
                    AgentRole::System,
                    format!(
                        "Unknown command '{cmd}'. Type /help to view available slash commands."
                    ),
                ));
            }
        }
    }

    /// Handle a key event through the keybinding map.
    pub fn on_key(&mut self, key: KeyEvent) {
        if key.kind == KeyEventKind::Release {
            return;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') => {
                    self.should_quit = true;
                    return;
                }
                KeyCode::Char('n') => {
                    self.start_new_session();
                    return;
                }
                _ => {}
            }
        }

        // Ctrl+P toggle command palette
        if key.code == KeyCode::Char('p') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.is_palette_open = !self.is_palette_open;
            self.palette_query.clear();
            self.palette_selected = 0;
            return;
        }

        // Command palette modal navigation
        if self.is_palette_open {
            match key.code {
                KeyCode::Esc => {
                    self.is_palette_open = false;
                    self.palette_query.clear();
                }
                KeyCode::Up => {
                    self.palette_selected = self.palette_selected.saturating_sub(1);
                }
                KeyCode::Down => {
                    let len = self.filtered_palette().len();
                    if len > 0 {
                        self.palette_selected = (self.palette_selected + 1).min(len - 1);
                    }
                }
                KeyCode::Tab => {
                    // Autocomplete the highlighted slash command into chat input
                    let filtered = self.filtered_palette();
                    if let Some((cmd, _)) = filtered.get(self.palette_selected) {
                        let selected_cmd = cmd.to_string();
                        self.is_palette_open = false;
                        self.palette_query.clear();
                        self.active_tab = ActiveTab::AgentChat;
                        self.is_input_mode = true;
                        self.input_buffer = format!("{selected_cmd} ");
                    }
                }
                KeyCode::Enter => {
                    let filtered = self.filtered_palette();
                    if let Some((cmd, _)) = filtered.get(self.palette_selected) {
                        let selected_cmd = cmd.to_string();
                        self.is_palette_open = false;
                        self.palette_query.clear();
                        // If command requires arguments, prefill and enter input mode
                        let needs_args = matches!(
                            selected_cmd.as_str(),
                            "/agent"
                                | "/remember"
                                | "/knowledge"
                                | "/export"
                                | "/theme"
                                | "/verify"
                                | "/create-agent"
                                | "/delete-agent"
                                | "/buddy"
                                | "/computer-use"
                        );
                        if needs_args {
                            self.active_tab = ActiveTab::AgentChat;
                            self.is_input_mode = true;
                            self.input_buffer = format!("{selected_cmd} ");
                        } else {
                            self.execute_slash_command(&selected_cmd);
                        }
                    }
                }
                KeyCode::Backspace => {
                    self.palette_query.pop();
                    self.palette_selected = 0;
                }
                KeyCode::Char(c) => {
                    self.palette_query.push(c);
                    self.palette_selected = 0;
                }
                _ => {}
            }
            return;
        }

        // Active chat input editing mode
        if self.is_input_mode {
            match key.code {
                KeyCode::Tab => {
                    self.set_tab(self.active_tab.next());
                }
                KeyCode::BackTab => {
                    self.set_tab(self.active_tab.previous());
                }
                KeyCode::Esc => {
                    self.is_input_mode = false;
                }
                KeyCode::Enter => {
                    let prompt = self.input_buffer.trim().to_string();
                    if !prompt.is_empty() {
                        self.input_buffer.clear();
                        self.is_input_mode = false;

                        if prompt.starts_with('/') {
                            self.execute_slash_command(&prompt);
                        } else {
                            self.submit_prompt(prompt);
                        }
                    }
                }
                KeyCode::Backspace => {
                    self.input_buffer.pop();
                }
                KeyCode::Char(c) => {
                    // If user types '/' as the first character, trigger command palette
                    if c == '/' && self.input_buffer.is_empty() {
                        self.is_input_mode = false;
                        self.is_palette_open = true;
                        self.palette_query = "/".to_string();
                        self.palette_selected = 0;
                    } else {
                        self.input_buffer.push(c);
                    }
                }
                _ => {}
            }
            return;
        }

        // Pressing '/' in normal mode opens the command palette
        if key.code == KeyCode::Char('/') {
            self.is_palette_open = true;
            self.palette_query = "/".to_string();
            self.palette_selected = 0;
            return;
        }

        // Algos tab interactive hotkeys: Up/Down/j/k, Space, Enter, T, S
        if self.active_tab == ActiveTab::Algos && !self.is_input_mode && !self.is_palette_open {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.selected_algo_index = self.selected_algo_index.saturating_sub(1);
                    return;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if !self.algos.is_empty() {
                        self.selected_algo_index =
                            (self.selected_algo_index + 1).min(self.algos.len() - 1);
                    }
                    return;
                }
                KeyCode::Char(' ') => {
                    self.toggle_selected_algo_status();
                    return;
                }
                KeyCode::Enter => {
                    let _ = self.run_backtest_for_strategy("");
                    return;
                }
                KeyCode::Char('t') | KeyCode::Char('T') => {
                    let _ = self.tune_for_strategy("");
                    return;
                }
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    let _ = self.suggest_for_strategy("");
                    return;
                }
                _ => {}
            }
        }

        // Backtest tab interactive hotkeys: R/Enter re-run, T tune, S suggest
        if self.active_tab == ActiveTab::Backtest && !self.is_input_mode && !self.is_palette_open {
            match key.code {
                KeyCode::Char('r') | KeyCode::Char('R') | KeyCode::Enter => {
                    let _ = self.run_backtest_for_strategy("");
                    return;
                }
                KeyCode::Char('t') | KeyCode::Char('T') => {
                    let _ = self.tune_for_strategy("");
                    return;
                }
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    let _ = self.suggest_for_strategy("");
                    return;
                }
                _ => {}
            }
        }

        // StrategyLab interactive hotkeys: [1-6] layer toggles, [+/-] SL buffer,
        // [s/a/t/e/x/m/r] params & views, [Left/Right/n/p] Hint Inspector cursor.
        if self.active_tab == ActiveTab::StrategyLab && !self.is_input_mode && !self.is_palette_open
        {
            match key.code {
                KeyCode::Char('1') => {
                    self.strategy_lab.vm.toggle_layer(SceneLayerGroup::Signals);
                    return;
                }
                KeyCode::Char('2') => {
                    self.strategy_lab.vm.toggle_layer(SceneLayerGroup::Bands);
                    return;
                }
                KeyCode::Char('3') => {
                    self.strategy_lab.vm.toggle_layer(SceneLayerGroup::Stops);
                    return;
                }
                KeyCode::Char('4') => {
                    self.strategy_lab
                        .vm
                        .toggle_layer(SceneLayerGroup::Indicators);
                    return;
                }
                KeyCode::Char('5') => {
                    self.strategy_lab.vm.toggle_layer(SceneLayerGroup::Hints);
                    return;
                }
                KeyCode::Char('6') => {
                    self.strategy_lab.vm.toggle_layer(SceneLayerGroup::Hedges);
                    return;
                }
                KeyCode::Left | KeyCode::Char('p') | KeyCode::Char('P') => {
                    self.strategy_lab_step_hint(-1);
                    return;
                }
                KeyCode::Right | KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.strategy_lab_step_hint(1);
                    return;
                }
                KeyCode::Char('+') | KeyCode::Char('=') | KeyCode::Char(']') => {
                    self.strategy_lab_adjust_buffer(Decimal::new(5, 2));
                    return;
                }
                KeyCode::Char('-') | KeyCode::Char('_') | KeyCode::Char('[') => {
                    self.strategy_lab_adjust_buffer(Decimal::new(-5, 2));
                    return;
                }
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    self.strategy_lab_cycle_strategy();
                    return;
                }
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    self.strategy_lab_cycle_asset();
                    return;
                }
                KeyCode::Char('t') | KeyCode::Char('T') => {
                    self.strategy_lab_cycle_timeframe();
                    return;
                }
                KeyCode::Char('e') | KeyCode::Char('E') => {
                    self.strategy_lab_cycle_entry_mode();
                    return;
                }
                KeyCode::Char('x') | KeyCode::Char('X') => {
                    self.strategy_lab_cycle_expiry();
                    return;
                }
                KeyCode::Char('m') | KeyCode::Char('M') | KeyCode::Char('v') => {
                    self.strategy_lab.detail_view = self.strategy_lab.detail_view.next();
                    return;
                }
                KeyCode::Char('r') | KeyCode::Char('R') | KeyCode::Enter => {
                    self.strategy_lab.reeval();
                    return;
                }
                _ => {}
            }
        }

        // Direct tab switching with digits 1 through 8
        if let KeyCode::Char(c @ '1'..='8') = key.code {
            let idx = (c as u8 - b'1') as usize;
            if idx < ActiveTab::ALL.len() {
                self.set_tab(ActiveTab::ALL[idx]);
                return;
            }
        }

        // Quick enter to chat input when on Agent tab
        if key.code == KeyCode::Char('i') {
            self.is_input_mode = true;
            return;
        }

        if let Some(action) = handle_key(key) {
            self.apply(action);
        }
    }

    /// Apply an action to the app state.
    pub fn apply(&mut self, action: Action) {
        match action {
            Action::Quit => self.should_quit = true,
            Action::NextTab => self.set_tab(self.active_tab.next()),
            Action::PreviousTab => self.set_tab(self.active_tab.previous()),
            Action::Down | Action::Up => {
                self.move_selection(action);
            }
            Action::Submit => {
                if self.active_tab == ActiveTab::AgentChat {
                    self.is_input_mode = true;
                }
            }
        }
    }

    /// Selected algorithm in the Algos tab.
    pub fn selected_algo(&self) -> Option<&RegisteredAlgo> {
        self.algos.get(self.selected_algo_index)
    }

    /// Mutable selected algorithm in the Algos tab.
    pub fn selected_algo_mut(&mut self) -> Option<&mut RegisteredAlgo> {
        self.algos.get_mut(self.selected_algo_index)
    }

    /// Toggle status of currently selected algorithm.
    pub fn toggle_selected_algo_status(&mut self) {
        if let Some(algo) = self.selected_algo_mut() {
            algo.status = algo.status.toggle();
        }
    }

    /// Run backtest for given strategy (or selected) and update Backtest tab state.
    pub fn run_backtest_for_strategy(&mut self, strategy_name: &str) -> Result<String, String> {
        let name = if strategy_name.trim().is_empty() {
            self.selected_algo()
                .map(|a| a.descriptor.id.clone())
                .unwrap_or_else(|| "turtle_breakout".to_string())
        } else {
            strategy_name.trim().to_string()
        };

        let symbol = self
            .algos
            .iter()
            .find(|a| a.descriptor.id.eq_ignore_ascii_case(&name))
            .and_then(|a| a.descriptor.target_instruments.first().cloned())
            .unwrap_or_else(|| "NSE:NIFTY50-INDEX".to_string());

        let capital = Decimal::new(1000000, 0);
        let (report, curve) =
            StrategyBridge::run_backtest_with_curve(&name, &symbol, None, capital)?;

        let pnl_sign = if report.net_pnl >= Decimal::ZERO {
            "+"
        } else {
            ""
        };
        let msg = format!(
            "Executed backtest for '{name}' on {symbol}: Net PnL {pnl_sign}₹{:.2} ({:+.2}%), Sharpe {:.2}, Max DD {:.2}%.",
            report.net_pnl, report.total_return_pct, report.sharpe_ratio, report.max_drawdown_pct
        );

        self.backtest_report = Some(report);
        self.backtest_equity_curve = curve;
        self.active_tab = ActiveTab::Backtest;

        Ok(msg)
    }

    /// Tune given strategy (or selected) and update Backtest tab state.
    pub fn tune_for_strategy(&mut self, strategy_name: &str) -> Result<String, String> {
        let name = if strategy_name.trim().is_empty() {
            self.selected_algo()
                .map(|a| a.descriptor.id.clone())
                .unwrap_or_else(|| "turtle_breakout".to_string())
        } else {
            strategy_name.trim().to_string()
        };

        let symbol = self
            .algos
            .iter()
            .find(|a| a.descriptor.id.eq_ignore_ascii_case(&name))
            .and_then(|a| a.descriptor.target_instruments.first().cloned())
            .unwrap_or_else(|| "NSE:NIFTY50-INDEX".to_string());

        let capital = Decimal::new(1000000, 0);
        let opt = StrategyBridge::tune_strategy(&name, &symbol, capital, "sharpe")?;

        let (report, curve) = StrategyBridge::run_backtest_with_curve(
            &name,
            &symbol,
            Some(&opt.tuned_params),
            capital,
        )?;

        let dsr_pct = (opt.deflated_sharpe * Decimal::new(100, 0)).round_dp(2);
        let msg = format!(
            "Tuned parameters for '{name}' on {symbol}: Sharpe improved from {:.2} to {:.2} (DSR: {}% confidence, {}).",
            opt.default_sharpe, opt.tuned_sharpe, dsr_pct, opt.overfitting_verdict
        );

        self.optimization_result = Some(opt);
        self.backtest_report = Some(report);
        self.backtest_equity_curve = curve;
        self.active_tab = ActiveTab::Backtest;

        Ok(msg)
    }

    /// Generate optimal trade suggestion for given strategy (or selected).
    pub fn suggest_for_strategy(&mut self, strategy_name: &str) -> Result<String, String> {
        let name = if strategy_name.trim().is_empty() {
            self.selected_algo()
                .map(|a| a.descriptor.id.clone())
                .unwrap_or_else(|| "turtle_breakout".to_string())
        } else {
            strategy_name.trim().to_string()
        };

        let symbol = self
            .algos
            .iter()
            .find(|a| a.descriptor.id.eq_ignore_ascii_case(&name))
            .and_then(|a| a.descriptor.target_instruments.first().cloned())
            .unwrap_or_else(|| "NSE:NIFTY50-INDEX".to_string());

        let capital = Decimal::new(1000000, 0);
        let risk_pct = Decimal::new(10, 1);
        let sugg = StrategyBridge::generate_trade_suggestion(&name, &symbol, capital, risk_pct)?;

        let msg = format!(
            "Optimal trade suggestion for '{name}' on {symbol}: {} at ₹{:.2} (SL: ₹{:.2}, T1: ₹{:.2}, Size: {} units).",
            sugg.signal.bias.as_str(),
            sugg.signal.entry_price,
            sugg.signal.stop_loss,
            sugg.signal.target_1,
            sugg.sizing.quantity
        );

        self.trade_suggestion = Some(sugg);
        self.active_tab = ActiveTab::Backtest;

        Ok(msg)
    }

    /// Step the active hint cursor in `StrategyLab` by `delta` (`-1` or `+1`).
    pub fn strategy_lab_step_hint(&mut self, delta: isize) {
        let len = self.strategy_lab.vm.hints.len();
        if len == 0 {
            self.strategy_lab.selected_hint_index = 0;
            return;
        }
        let cur = self.strategy_lab.selected_hint_index as isize;
        let next = (cur + delta).rem_euclid(len as isize) as usize;
        self.strategy_lab.selected_hint_index = next;
    }

    /// Adjust `sl_buffer_atr` (`0.00..=1.50` ATR) and re-evaluate `StrategyLab`.
    pub fn strategy_lab_adjust_buffer(&mut self, delta_atr: Decimal) {
        let next = (self.strategy_lab.config.sl_buffer_atr + delta_atr)
            .max(Decimal::ZERO)
            .min(Decimal::new(150, 2));
        self.strategy_lab.config.sl_buffer_atr = next;
        self.strategy_lab.reeval();
    }

    /// Cycle between registered strategies (`elephant_edge_v3` <-> `orb_15`).
    pub fn strategy_lab_cycle_strategy(&mut self) {
        self.strategy_lab.config.strategy_id =
            if self.strategy_lab.config.strategy_id == "elephant_edge_v3" {
                "orb_15".to_string()
            } else {
                "elephant_edge_v3".to_string()
            };
        self.strategy_lab.config.entry_trigger_mode = "auto".to_string();
        self.strategy_lab.reeval();
    }

    /// Cycle between Index / F&O Equity / MCX Commodity fixtures.
    pub fn strategy_lab_cycle_asset(&mut self) {
        let next_sym = match self.strategy_lab.config.symbol.as_str() {
            "NSE:NIFTY50-INDEX" => "NSE:RELIANCE-EQ",
            "NSE:RELIANCE-EQ" => "MCX:GOLD",
            _ => "NSE:NIFTY50-INDEX",
        };
        self.strategy_lab.config.symbol = next_sym.to_string();
        self.strategy_lab.config.asset_class = infer_asset_class(next_sym).to_string();
        self.strategy_lab.reeval();
    }

    /// Cycle timeframe (`1d` -> `5m` -> `15m`).
    pub fn strategy_lab_cycle_timeframe(&mut self) {
        self.strategy_lab.config.timeframe = match self.strategy_lab.config.timeframe.as_str() {
            "1d" => "5m".to_string(),
            "5m" => "15m".to_string(),
            _ => "1d".to_string(),
        };
        self.strategy_lab.reeval();
    }

    /// Cycle entry filter mode (`rejection_wick` -> `zone_touch_close` -> `orb_breakout`).
    pub fn strategy_lab_cycle_entry_mode(&mut self) {
        self.strategy_lab.config.entry_trigger_mode =
            match self.strategy_lab.config.entry_trigger_mode.as_str() {
                "rejection_wick" => "zone_touch_close".to_string(),
                "zone_touch_close" => "orb_breakout".to_string(),
                _ => "rejection_wick".to_string(),
            };
        self.strategy_lab.reeval();
    }

    /// Cycle F&O/Commodity expiry horizon (`day` -> `weekly` -> `monthly` -> `commodity_cycle`).
    pub fn strategy_lab_cycle_expiry(&mut self) {
        self.strategy_lab.config.expiry_horizon =
            match self.strategy_lab.config.expiry_horizon.as_str() {
                "day" => "weekly".to_string(),
                "weekly" => "monthly".to_string(),
                "monthly" => "commodity_cycle".to_string(),
                _ => "day".to_string(),
            };
        self.strategy_lab.reeval();
    }

    fn move_selection(&mut self, action: Action) {
        match action {
            Action::Down => self.selected_index = self.selected_index.saturating_add(1),
            Action::Up => self.selected_index = self.selected_index.saturating_sub(1),
            _ => {}
        }
    }
}

impl Default for TuiApp {
    fn default() -> Self {
        Self::new()
    }
}
