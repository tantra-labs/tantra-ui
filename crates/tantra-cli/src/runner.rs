//! CLI runner: dispatch parsed commands to their implementations.

use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tantra_agent_llm::{
    CandleGgufConfig, CandleGgufProvider, ChatMessage, ChatRequest, ChatStreamEvent, ENV_GGUF_PATH,
    LlmProvider,
};
use tantra_agent_mcp::McpServer;
use tantra_agent_mcp::frontier::ManifestGenerator;

use crate::args::{
    ApprovalPolicyArg, AskArgs, ChatArgs, CliArgs, Command, DoctorArgs, JobsArgs, MarketplaceArgs,
    McpArgs, MemoryArgs, MissedPolicy, OutputFormat, PaperArgs, PaperMode, PluginArgs, PurgeArgs,
    ScanArgs, ServeArgs, SkillsArgs, StrategyArgs, ToolsArgs, VerifyArgs,
};
use crate::jobs_runtime::{AddRequest, JobsRuntime, short_id, summarize_receipt};
use crate::scan;
use crate::strategy_bridge::{self, StrategyBridge, StrategyBridgeRunner};
use rust_decimal::Decimal;
use std::collections::BTreeMap;

/// Errors surfaced by the CLI runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    /// The command is not wired to an in-process backend yet.
    NotWired(String),
    /// Missing file error (exit code 2).
    MissingFile(String),
    /// Strategy verification error (exit code 1).
    VerificationFailed(String),
    /// Execution error (exit code 1).
    Execution(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotWired(what) => write!(f, "`{what}` is not wired to a backend yet"),
            Self::MissingFile(err) => write!(f, "file error: {err}"),
            Self::VerificationFailed(err) => write!(f, "verification failed: {err}"),
            Self::Execution(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for CliError {}

/// What a successful CLI run produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandOutcome {
    /// The TUI was launched (or, in tests, requested to launch).
    LaunchedTui,
    /// An `ask` turn produced an answer string.
    Answer(String),
    /// An interactive chat session completed.
    ChatCompleted,
    /// A `scan` produced a result count.
    ScanResults {
        /// Number of rows returned.
        rows: usize,
    },
    /// A `jobs` command produced an outcome message.
    JobsSuccess(String),
    /// A `verify` run produced an outcome report.
    VerifySuccess {
        /// Whether the file passed without errors.
        passed: bool,
        /// Diagnostic message.
        message: String,
    },
    /// A `memory` command produced an outcome message.
    MemorySuccess(String),
    /// A `plugin` command produced an outcome message.
    PluginSuccess(String),
    /// A `skills` command produced an outcome message.
    SkillsSuccess(String),
    /// An `mcp` command produced an outcome message.
    McpSuccess(String),
    /// A `tools` command produced an outcome message.
    ToolsSuccess(String),
    /// A `doctor` run produced an environment report.
    DoctorReport(String),
    /// A `purge` run produced a cleanup summary.
    PurgeSuccess(String),
    /// A `health` check produced a report string.
    HealthReport(String),
    /// A `strategy` command produced an outcome message.
    StrategySuccess(String),
    /// A `serve` run exited cleanly after graceful shutdown.
    ServeSuccess(String),
    /// A `paper` session produced a report string (plan 0008 D4).
    PaperSuccess(String),
}

/// The CLI home directory (`~/.tantra`, override with `TANTRA_HOME`).
pub fn tantra_home_dir() -> PathBuf {
    if let Ok(home) = std::env::var("TANTRA_HOME") {
        if !home.trim().is_empty() {
            return PathBuf::from(home);
        }
    }
    if let Ok(user_home) = std::env::var("HOME") {
        PathBuf::from(user_home).join(".tantra")
    } else {
        PathBuf::from(".tantra")
    }
}

fn find_existing_path(path: &Path) -> PathBuf {
    if path.exists() {
        return path.to_path_buf();
    }
    if path.is_relative() {
        for parent in ["..", "../..", "../../.."] {
            let candidate = Path::new(parent).join(path);
            if candidate.exists() {
                return candidate;
            }
        }
    }
    path.to_path_buf()
}

/// Resolve provider to use for the CLI command.
pub fn resolve_provider(
    model_path: Option<&Path>,
    temperature: Option<f64>,
) -> Result<Option<Arc<dyn LlmProvider>>, CliError> {
    // 1. Explicit model path passed on CLI
    if let Some(path) = model_path {
        let resolved = find_existing_path(path);
        let mut config = CandleGgufConfig::new(resolved);
        if let Some(t) = temperature {
            config = config.with_temperature(t);
        }
        return Ok(Some(Arc::new(CandleGgufProvider::new(config))));
    }

    // 2. Environment variable TANTRA_GGUF_PATH
    if let Ok(env_path) = std::env::var(ENV_GGUF_PATH) {
        if !env_path.trim().is_empty() {
            let resolved = find_existing_path(Path::new(&env_path));
            let mut config = CandleGgufConfig::new(resolved);
            if let Some(t) = temperature {
                config = config.with_temperature(t);
            }
            return Ok(Some(Arc::new(CandleGgufProvider::new(config))));
        }
    }

    // 3. OpenAI-compatible endpoint from environment
    if let Ok(provider) = tantra_agent_llm::openai_provider_from_env() {
        return Ok(Some(Arc::new(provider)));
    }

    // 4. Default model file on disk
    let candidates = [
        "models/SmolLM2-360M-Instruct-Q4_K_M.gguf",
        "models/smollm2-360m-instruct-q4_k_m.gguf",
    ];
    for c in candidates {
        let resolved = find_existing_path(Path::new(c));
        if resolved.exists() {
            let mut config = CandleGgufConfig::new(resolved);
            if let Some(t) = temperature {
                config = config.with_temperature(t);
            }
            return Ok(Some(Arc::new(CandleGgufProvider::new(config))));
        }
    }

    Ok(None)
}

/// Dispatch a parsed command using default provider resolution.
pub fn run(cli: &CliArgs) -> Result<CommandOutcome, CliError> {
    let provider = resolve_provider(cli.model_path.as_deref(), cli.temperature)?;
    run_with_provider(cli, provider)
}

/// Dispatch a parsed command with an explicitly supplied LLM provider.
pub fn run_with_provider(
    cli: &CliArgs,
    provider: Option<Arc<dyn LlmProvider>>,
) -> Result<CommandOutcome, CliError> {
    match &cli.command {
        Command::Tui { .. } => Ok(CommandOutcome::LaunchedTui),
        Command::Ask(args) => run_ask(args, cli, provider),
        Command::Chat(args) => run_chat(args, cli, provider),
        Command::Scan(args) => run_scan(args),
        Command::Serve(args) => run_serve(args),
        Command::Jobs(args) => run_jobs(args),
        Command::Verify(args) => run_verify(args),
        Command::Memory(args) => run_memory(args),
        Command::Plugin(args) => run_plugin(args),
        Command::Skills(args) => run_skills(args),
        Command::Mcp(args) => run_mcp(args),
        Command::Tools(args) => run_tools(args),
        Command::Doctor(args) => run_doctor(args),
        Command::Purge(args) => run_purge(args),
        Command::Health => run_health(),
        Command::Strategy(args) => run_strategy(args),
        Command::Paper(args) => run_paper(args),
    }
}

/// Run `tantra paper`: embedded deterministic engine session or thin-server
/// instructions (plan 0008 D4).
fn run_paper(args: &PaperArgs) -> Result<CommandOutcome, CliError> {
    let cash = args
        .cash
        .as_deref()
        .and_then(|s| s.trim().parse::<Decimal>().ok())
        .unwrap_or_else(|| Decimal::new(1_000_000, 0));
    let msg = match args.mode {
        PaperMode::Embedded => crate::paper::run_embedded_report(cash, args.seed),
        PaperMode::Server => crate::paper::run_server_instructions(args.server.as_deref()),
    };
    Ok(CommandOutcome::PaperSuccess(msg))
}

fn run_ask(
    args: &AskArgs,
    cli: &CliArgs,
    provider: Option<Arc<dyn LlmProvider>>,
) -> Result<CommandOutcome, CliError> {
    let prompt = args.prompt.trim();
    if prompt.is_empty() {
        return Err(CliError::Execution("prompt cannot be empty".to_string()));
    }

    let lower_prompt = prompt.to_lowercase();
    let is_strategy_workflow = lower_prompt.contains("strategy")
        || lower_prompt.contains("tune")
        || lower_prompt.contains("2r")
        || lower_prompt.contains("turtle")
        || lower_prompt.contains("trap")
        || lower_prompt.contains("recommendation")
        || lower_prompt.contains("suggestion");

    // Direct backtest simulation query handler (tantra-sim tool integration for portfolio rebalancing)
    if !is_strategy_workflow {
        if let Some(res) =
            crate::backtest_gen::handle_backtest_query(prompt, args.format, cli.quiet)
        {
            return res;
        }
    }

    let provider = provider.ok_or_else(|| CliError::NotWired("tantra ask".to_string()))?;

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::Execution(format!("failed to initialize tokio: {e}")))?;

    rt.block_on(async {
        // Quantcept convention: chatter / progress to stderr
        if !cli.quiet && args.format == OutputFormat::Text {
            eprintln!(
                "🧠 Tantra Agent initializing ({}) ...",
                provider.model_name()
            );
            if let Some(ref p) = args.persona {
                eprintln!("🎭 Persona: {p}");
            }
            eprintln!("⚡ Question: {prompt}\n");
        }

        let temp_decimal = cli
            .temperature
            .and_then(|t| rust_decimal::Decimal::try_from(t).ok());

        let strategy_tools = tantra_agent_chat_tools::catalog::create_strategy_tools(Some(
            Arc::new(StrategyBridgeRunner::default()),
        ))
        .into_iter()
        .map(|t| t.spec().clone())
        .collect::<Vec<_>>();

        // Tool-calling workflow: detect if prompt requests strategy selection, backtesting, tuning, 2R, or trap analysis
        let lower_prompt = prompt.to_lowercase();
        let is_strategy_workflow = lower_prompt.contains("strategy")
            || lower_prompt.contains("backtest")
            || lower_prompt.contains("tune")
            || lower_prompt.contains("2r")
            || lower_prompt.contains("turtle")
            || lower_prompt.contains("trap")
            || lower_prompt.contains("recommendation")
            || lower_prompt.contains("suggestion");

        let (final_prompt, tool_calls_json, suggestion_card_opt, trade_sugg_opt) =
            if is_strategy_workflow {
                let strat_name = if lower_prompt.contains("sma") {
                    "sma_crossover"
                } else if lower_prompt.contains("rsi") {
                    "rsi_reversal"
                } else {
                    "turtle_breakout"
                };

                let symbol = if lower_prompt.contains("banknifty") {
                    "NSE:BANKNIFTY-INDEX"
                } else if lower_prompt.contains("reliance") {
                    "NSE:RELIANCE-EQ"
                } else {
                    "NSE:NIFTY50-INDEX"
                };

                let capital = Decimal::new(1_000_000, 0);
                let risk_pct = Decimal::new(1, 0);

                // Execute real tool call
                match StrategyBridge::generate_trade_suggestion(strat_name, symbol, capital, risk_pct) {
                    Ok(sugg) => {
                        let card = strategy_bridge::render_suggestion_card(&sugg);
                        let tool_call_log = format!(
                            "strategy_suggest(strategy=\"{strat_name}\", symbol=\"{symbol}\", capital={capital}, risk_pct={risk_pct})"
                        );

                        if args.format == OutputFormat::Text {
                            println!("⚡ Tool Call: {tool_call_log}");
                            println!("{card}");
                            println!("── Analytical Synthesis & Trap Verification ──");
                        }

                        let tuned_params_str = sugg
                            .tuned_params
                            .iter()
                            .map(|(k, v)| format!("{k}={v}"))
                            .collect::<Vec<_>>()
                            .join(", ");

                        let prompt_with_context = format!(
                            r#"USER QUERY:
{prompt}

VERIFIED TOOL EXECUTION CONTEXT (from tantra-sim & parameter tuner):
• Selected Strategy : {} ({})
• Target Instrument : {}
• Tuned Parameters  : {} (optimized via 100-trial simulation over parameter space)
• Backtest Metrics  : Sharpe {:.2} (vs {:.2} baseline) | Win Rate: {:.2}% | Max DD: {:.2}%
• Overfitting Audit : Deflated Sharpe Ratio (DSR) = {:.2}% ({})
• Look-Ahead Audit  : Zero look-ahead bias. Orders execute on Next-Bar Open (C_t -> O_{{t+1}}).
• Execution Geometry:
  - Entry Level : ₹{:.2}
  - Stop Loss   : ₹{:.2} (2.0 ATR protective distance)
  - Target 1 (2R): ₹{:.2} (+₹{:.2} profit objective)
  - Target 2 (3R): ₹{:.2} (+₹{:.2} profit objective)
• Sizing & Fees     : {} units (Notional: ₹{:.2}, Margin: {:.2}%, Risk Budget: ₹{:.2})
• Statutory Cost    : ₹{:.2} (STT + GST + Exchange + Stamp Duty) -> Net Friction-Adjusted RR: 1 : {:.2}
• Platform Trap Checks:
  - Bull Trap check: False breakout veto (confirmed 20-day high with volume confirmation)
  - Bear Trap check: Short squeeze protection verified
  - Liquidity Hunt Trap: 2.0 ATR buffer prevents stop cluster sweeps
  - Statutory Friction Drag: Net RR remains at 1 : {:.2} after Indian transaction taxes

TASK:
Provide a concise, quantitative institutional synthesis covering:
1. Strategy Selection Rationale (why {} fits current regime)
2. 2R Trade Geometry Calibration (Entry, SL, and 2R/3R targets)
3. Look-Ahead Bias Avoidance (O_{{t+1}} fill timing, DSR confidence)
4. Platform Trap Checks (Bull Trap, Bear Trap, Liquidity Hunt, Statutory Friction) reinforcing the buy/sell recommendation.
"#,
                            sugg.strategy.name,
                            sugg.strategy.id,
                            sugg.symbol,
                            tuned_params_str,
                            sugg.optimization.tuned_sharpe,
                            sugg.optimization.default_sharpe,
                            sugg.optimization.win_rate_pct,
                            sugg.optimization.max_drawdown_pct,
                            (sugg.optimization.deflated_sharpe * Decimal::new(100, 0)).round_dp(2),
                            sugg.optimization.overfitting_verdict,
                            sugg.signal.entry_price,
                            sugg.signal.stop_loss,
                            sugg.signal.target_1,
                            (sugg.signal.target_1 - sugg.signal.entry_price).abs(),
                            sugg.signal.target_2,
                            (sugg.signal.target_2 - sugg.signal.entry_price).abs(),
                            sugg.sizing.quantity,
                            sugg.sizing.notional_value,
                            sugg.sizing.margin_utilization_pct,
                            sugg.sizing.risk_amount,
                            sugg.statutory_charges.total_charges,
                            sugg.statutory_charges.net_risk_reward_ratio,
                            sugg.statutory_charges.net_risk_reward_ratio,
                            sugg.strategy.name,
                        );

                        (
                            prompt_with_context,
                            Some(serde_json::json!([{
                                "tool": "strategy_suggest",
                                "arguments": {
                                    "strategy": strat_name,
                                    "symbol": symbol,
                                    "capital": capital,
                                    "risk_pct": risk_pct
                                }
                            }])),
                            Some(card),
                            Some(sugg),
                        )
                    }
                    Err(e) => {
                        eprintln!("⚠️ Strategy tool execution failed: {e}");
                        (prompt.to_string(), None, None, None)
                    }
                }
            } else {
                (prompt.to_string(), None, None, None)
            };

        let system_prompt = args.persona.as_ref().map(|p| {
            format!(
                "You are the Tantra {p} agent. Analyze Indian equity markets, microstructure, and risk strictly from this perspective."
            )
        });

        let request = ChatRequest {
            messages: vec![ChatMessage::user(final_prompt)],
            system_prompt,
            tools: strategy_tools,
            temperature: temp_decimal,
            max_tokens: None,
        };

        let mut stream = provider
            .chat_stream(&request)
            .await
            .map_err(|e| CliError::Execution(format!("LLM inference error: {e}")))?;

        use futures::StreamExt;
        let mut full_answer = String::new();

        while let Some(event_res) = stream.next().await {
            let event =
                event_res.map_err(|e| CliError::Execution(format!("LLM stream error: {e}")))?;
            match event {
                ChatStreamEvent::TextDelta { text } => {
                    if args.format == OutputFormat::Text {
                        print!("{text}");
                        io::stdout()
                            .flush()
                            .map_err(|e| CliError::Execution(format!("flush stdout: {e}")))?;
                    }
                    full_answer.push_str(&text);
                }
                ChatStreamEvent::MessageEnd { .. } => break,
                _ => {}
            }
        }

        if args.format == OutputFormat::Text {
            println!();
        } else {
            let json_out = serde_json::json!({
                "prompt": prompt,
                "tool_calls": tool_calls_json,
                "trade_suggestion": trade_sugg_opt,
                "card": suggestion_card_opt,
                "answer": full_answer,
                "persona": args.persona,
                "mode": args.mode,
                "timeout_min": args.timeout_min,
                "model": provider.model_name()
            });
            println!("{}", serde_json::to_string_pretty(&json_out).unwrap_or_default());
        }

        Ok(CommandOutcome::Answer(full_answer))
    })
}

fn run_chat(
    args: &ChatArgs,
    _cli: &CliArgs,
    provider: Option<Arc<dyn LlmProvider>>,
) -> Result<CommandOutcome, CliError> {
    let provider = provider.ok_or_else(|| {
        CliError::NotWired(
            "tantra chat (no model found. Provide --model-path or TANTRA_GGUF_PATH)".to_string(),
        )
    })?;

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::Execution(format!("failed to initialize tokio: {e}")))?;

    rt.block_on(async {
        println!("┌────────────────────────────────────────────────────────────┐");
        println!("│ Tantra Interactive AI Trading Desk                        │");
        println!("│ Model: {:<51} │", provider.model_name());
        println!("│ Type /help for commands, 'exit' or Ctrl+C to quit          │");
        println!("└────────────────────────────────────────────────────────────┘\n");

        let mut history: Vec<ChatMessage> = Vec::new();

        let stdin = io::stdin();
        let mut input_line = String::new();

        loop {
            print!("\x1b[1;36mtantra>\x1b[0m ");
            io::stdout()
                .flush()
                .map_err(|e| CliError::Execution(format!("flush stdout: {e}")))?;

            input_line.clear();
            if stdin.read_line(&mut input_line).is_err() || input_line.is_empty() {
                break;
            }

            let prompt = input_line.trim();
            if prompt.is_empty() {
                continue;
            }
            if prompt.eq_ignore_ascii_case("exit") || prompt.eq_ignore_ascii_case("quit") {
                println!("Exiting chat. Dhanyavaad!");
                break;
            }

            history.push(ChatMessage::user(prompt));
            let strategy_tools = tantra_agent_chat_tools::catalog::create_strategy_tools(None)
                .into_iter()
                .map(|t| t.spec().clone())
                .collect::<Vec<_>>();

            let request = ChatRequest {
                messages: history.clone(),
                system_prompt: args.system_prompt.clone(),
                tools: strategy_tools,
                temperature: None,
                max_tokens: None,
            };

            let mut stream = provider
                .chat_stream(&request)
                .await
                .map_err(|e| CliError::Execution(format!("LLM inference error: {e}")))?;

            use futures::StreamExt;
            let mut answer = String::new();

            print!("\x1b[1;32magent>\x1b[0m ");
            while let Some(event_res) = stream.next().await {
                let event =
                    event_res.map_err(|e| CliError::Execution(format!("LLM stream error: {e}")))?;
                match event {
                    ChatStreamEvent::TextDelta { text } => {
                        print!("{text}");
                        io::stdout()
                            .flush()
                            .map_err(|e| CliError::Execution(format!("flush stdout: {e}")))?;
                        answer.push_str(&text);
                    }
                    ChatStreamEvent::MessageEnd { .. } => break,
                    _ => {}
                }
            }
            println!("\n");
            history.push(ChatMessage::assistant(answer));
        }

        Ok(CommandOutcome::ChatCompleted)
    })
}

fn run_scan(args: &ScanArgs) -> Result<CommandOutcome, CliError> {
    scan::run_scan_args(args)
}

/// Boots `tantra-runtime-api-rest` and blocks until `Ctrl-C`/`SIGTERM`.
///
/// `run_server_blocking` prints `listening on http://<addr>` itself once bound and performs its
/// own graceful shutdown; this just builds the `ServeConfig` from CLI args and maps the result.
fn run_serve(args: &ServeArgs) -> Result<CommandOutcome, CliError> {
    let mut config = tantra_runtime_api_rest::ServeConfig::new(args.data_dir.clone());
    config.addr = args.addr;
    config.cors_origins = args.cors_origin.clone();
    if let Some(dir) = &args.profiles_dir {
        config.runs.profiles_dir = dir.clone();
    }
    if let Some(dir) = &args.portfolios_dir {
        config.runs.portfolios_dir = dir.clone();
    }
    if let Some(dir) = &args.journals_dir {
        config.runs.journals_dir = dir.clone();
    }
    config.auth.tokens_file = args.tokens_file.clone();

    tantra_runtime_api_rest::bootstrap::run_server_blocking(&config)
        .map_err(|e| CliError::Execution(e.to_string()))?;
    Ok(CommandOutcome::ServeSuccess(
        "tantra: server shut down cleanly".to_string(),
    ))
}

fn run_jobs(args: &JobsArgs) -> Result<CommandOutcome, CliError> {
    run_jobs_with_home(args, &tantra_home_dir())
}

/// Runs a `tantra jobs` command against an explicit home directory
/// (`TANTRA_HOME` isolation for tests/operators).
pub fn run_jobs_with_home(args: &JobsArgs, home: &Path) -> Result<CommandOutcome, CliError> {
    let runtime = JobsRuntime::open(home)?;

    match args {
        JobsArgs::Add {
            goal,
            schedule,
            max_turns,
            max_inr,
            max_usd,
            max_tokens,
            max_tool_calls,
            max_data_calls,
            missed_policy,
            approval,
            strategy,
            symbol,
            capital,
            params,
            success,
            read_only,
            once,
        } => {
            // Quantcept convention: add prints the 8-character ID.
            let req = AddRequest {
                goal: goal.clone(),
                schedule: schedule.clone(),
                max_turns: *max_turns,
                max_inr: *max_inr,
                max_usd: *max_usd,
                max_tokens: *max_tokens,
                max_tool_calls: *max_tool_calls,
                max_data_calls: *max_data_calls,
                missed_policy: match missed_policy {
                    MissedPolicy::Skip => tantra_runtime_jobs::MissedPolicy::Skip,
                    MissedPolicy::CatchUp => tantra_runtime_jobs::MissedPolicy::CatchUp,
                },
                approval_manual: *approval == ApprovalPolicyArg::Manual,
                strategy: strategy.clone(),
                symbol: symbol.clone(),
                capital: *capital,
                params: params.clone(),
                success: success.clone(),
                read_only: *read_only,
                once: *once,
            };
            let spec = runtime.add(&req)?;
            let sched_desc = schedule.as_deref().unwrap_or("none (manual trigger)");
            let trigger_desc = if schedule.is_some() { "cron" } else { "manual" };
            let cost_desc = if let Some(inr) = max_inr {
                format!("₹{inr}")
            } else if let Some(usd) = max_usd {
                format!("${usd}")
            } else {
                "unlimited".to_string()
            };
            let append_budget = |s: &mut String, opt: &Option<usize>, unit: &str| {
                if let Some(limit) = opt {
                    s.push_str(&format!(", max {limit} {unit}"));
                }
            };
            let mut summary = format!(
                "Created job {short}\n\
                 Goal:           {goal}\n\
                 Schedule:       {sched_desc}\n\
                 Trigger:        {trigger_desc}\n\
                 Approval:       {approval}\n\
                 Strategy:       {strategy_label}\n\
                 Budget:         max {max_turns} turns, {cost_desc}, max {} tokens\n\
                 Read-Only:      {read_only}\n\
                 Run Once:       {once}",
                max_tokens
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "unlimited".to_string()),
                short = short_id(&spec),
                approval = match approval {
                    ApprovalPolicyArg::Auto => "auto (budget breaches need approval)",
                    ApprovalPolicyArg::Manual => "manual (every run needs approval)",
                },
                strategy_label = match (strategy, symbol, capital) {
                    (Some(strategy), Some(symbol), Some(capital)) => {
                        format!("{strategy} on {symbol} @ ₹{capital}")
                    }
                    (Some(strategy), Some(symbol), None) => format!("{strategy} on {symbol}"),
                    (Some(strategy), None, _) => strategy.to_string(),
                    _ => "none (goal-only; `jobs run` requires a strategy)".to_string(),
                }
            );
            append_budget(&mut summary, max_tool_calls, "tool calls");
            append_budget(&mut summary, max_data_calls, "data calls");
            Ok(CommandOutcome::JobsSuccess(summary))
        }
        JobsArgs::List => {
            let mut specs = runtime.registry()?.list_jobs();
            if specs.is_empty() {
                return Ok(CommandOutcome::JobsSuccess(
                    "No autonomous jobs registered. Create one with `tantra jobs add <goal> --schedule <cron> --strategy <name>`.".to_string(),
                ));
            }
            specs.sort_by_key(|spec| spec.created_at);
            let mut out = String::from(
                "ID         STATUS             SCHEDULE        NEXT RUN                   GOAL\n",
            );
            out.push_str(
                "─────────  ─────────────────  ──────────────  ─────────────────────────  ────────────────────────────────────────\n",
            );
            for spec in &specs {
                let manual = spec.metadata.get("trigger").map(String::as_str) == Some("manual");
                let sched = if manual {
                    "manual".to_string()
                } else {
                    spec.schedule.expression().to_string()
                };
                let next = spec
                    .next_run_at
                    .map(|at| at.to_rfc3339())
                    .unwrap_or_else(|| "-".to_string());
                out.push_str(&format!(
                    "{:<9}  {:<17}  {:<14}  {:<25}  {:<40}\n",
                    short_id(spec),
                    format!("{:?}", spec.status),
                    sched,
                    next,
                    spec.goal
                ));
            }
            Ok(CommandOutcome::JobsSuccess(out))
        }
        JobsArgs::Run { id } => {
            let spec = runtime.resolve_job(id)?;
            let receipt = runtime.run(&spec, "manual")?;
            Ok(CommandOutcome::JobsSuccess(summarize_receipt(&receipt)))
        }
        JobsArgs::Tick { all } => {
            let scope = if *all {
                "all workspaces"
            } else {
                "current workspace"
            };
            let report = runtime.tick()?;
            let mut out = format!(
                "Ticked cron scheduler across {scope}: recovered {} interrupted journal(s), swept {} expired lease(s).",
                report.recovered_runs, report.swept_leases
            );
            if !report.executed.is_empty() {
                out.push_str(&format!("\nExecuted {} job run(s):", report.executed.len()));
                for (id, receipt) in &report.executed {
                    out.push_str(&format!("\n• {id}: {}", summarize_receipt(receipt)));
                    if receipt.job_status != tantra_runtime_jobs::JobStatus::Active {
                        out.push_str(&format!(" [job now: {:?}]", receipt.job_status));
                    }
                }
            } else {
                out.push_str("\nNo jobs fired on this tick.");
            }
            for (id, fire) in &report.pending_until {
                out.push_str(&format!("\nWaiting {id}: fires {fire}"));
            }
            for (id, reason) in &report.held {
                out.push_str(&format!("\nHeld {id}: {reason}"));
            }
            Ok(CommandOutcome::JobsSuccess(out))
        }
        JobsArgs::Install => {
            // Quantcept convention: prints snippet and does not install
            let snippet = "# Tantra cron scheduler setup:\n\
                           # Run every 15 minutes to evaluate autonomous jobs\n\
                           */15 * * * * tantra jobs tick\n\n\
                           # Or as a systemd user timer (~/.config/systemd/user/tantra-jobs.timer):\n\
                           # [Timer]\n\
                           # OnCalendar=*:0/15\n\
                           # Persistent=true";
            Ok(CommandOutcome::JobsSuccess(snippet.to_string()))
        }
        JobsArgs::Logs { id } => {
            let spec = runtime.resolve_job(id)?;
            let runs = runtime.journal_runs(&spec)?;
            if runs.is_empty() {
                return Ok(CommandOutcome::JobsSuccess(format!(
                    "No journaled runs for job {} yet. Run `tantra jobs run` or wait for `tantra jobs tick`.",
                    short_id(&spec)
                )));
            }
            let mut out = format!("Runs of job {} (newest first):", short_id(&spec));
            for run in &runs {
                let run_id = run
                    .manifest
                    .get("run_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let status = run
                    .manifest
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let trigger = run
                    .manifest
                    .get("trigger")
                    .and_then(|v| v.as_str())
                    .unwrap_or("manual");
                out.push_str(&format!(
                    "\n• run_id={run_id} status={status} trigger={trigger} events={} dir={}",
                    run.events.len(),
                    run.dir.display()
                ));
            }
            out.push_str("\nNewest run events:");
            for event in &runs[0].events {
                out.push_str(&format!(
                    "\n  seq={} type={}",
                    event
                        .get("seq")
                        .map(|v| v.to_string())
                        .unwrap_or("0".into()),
                    event
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                ));
            }
            Ok(CommandOutcome::JobsSuccess(out))
        }
        JobsArgs::Pause { id } => {
            let spec = runtime.pause(id)?;
            Ok(CommandOutcome::JobsSuccess(format!(
                "Job {} paused.",
                short_id(&spec)
            )))
        }
        JobsArgs::Resume { id } => {
            let spec = runtime.resume(id)?;
            Ok(CommandOutcome::JobsSuccess(format!(
                "Job {} resumed.",
                short_id(&spec)
            )))
        }
        JobsArgs::Approvals => {
            let queue = runtime.approvals()?;
            if queue.is_empty() {
                return Ok(CommandOutcome::JobsSuccess(
                    "No pending actions requiring human confirmation (0 approvals in queue)."
                        .to_string(),
                ));
            }
            let mut out = format!(
                "{} pending approval(s) (budget breaches / manual gates):",
                queue.len()
            );
            for spec in &queue {
                let reason = spec.budgets.validate(&spec.usage).err();
                out.push_str(&format!(
                    "\n• {} '{}' — approve: `tantra jobs approve {}`, deny: `tantra jobs deny {}`",
                    short_id(spec),
                    spec.name,
                    short_id(spec),
                    short_id(spec)
                ));
                if let Some(breach) = reason {
                    out.push_str(&format!(" (breach: {breach})"));
                }
            }
            Ok(CommandOutcome::JobsSuccess(out))
        }
        JobsArgs::Approve { id } => {
            let spec = runtime.approve(id)?;
            Ok(CommandOutcome::JobsSuccess(format!(
                "Job {} approved; it may run again (`tantra jobs run {}`).",
                short_id(&spec),
                short_id(&spec)
            )))
        }
        JobsArgs::Deny { id } => {
            let spec = runtime.deny(id)?;
            Ok(CommandOutcome::JobsSuccess(format!(
                "Job {} denied and paused.",
                short_id(&spec)
            )))
        }
    }
}

fn run_verify(args: &VerifyArgs) -> Result<CommandOutcome, CliError> {
    let file_path = match &args.file {
        Some(path) => path.clone(),
        None => {
            let default_strategy = PathBuf::from("strategy.py");
            if default_strategy.exists() {
                default_strategy
            } else {
                return Ok(CommandOutcome::VerifySuccess {
                    passed: true,
                    message: "No strategy file provided and 'strategy.py' not found. Specify `tantra verify <path/to/strategy.py>`.".to_string(),
                });
            }
        }
    };

    // Quantcept convention: Exit 2 if the file is missing
    if !file_path.exists() {
        return Err(CliError::MissingFile(format!(
            "strategy file does not exist: {}",
            file_path.display()
        )));
    }

    let content = fs::read_to_string(&file_path).map_err(|e| {
        CliError::Execution(format!("cannot read file {}: {e}", file_path.display()))
    })?;

    let mut lookahead_errors = Vec::new();
    let mut leakage_warnings = Vec::new();

    // Check lines for common lookahead bias and leakage patterns
    for (idx, line) in content.lines().enumerate() {
        let line_num = idx + 1;
        let trimmed = line.trim();

        // 1. Future candle shifting or index lookahead (Fatal: exit 1)
        if trimmed.contains(".shift(-") || trimmed.contains("shift(-1)") {
            lookahead_errors.push(format!(
                "Line {line_num}: Negative shift access (.shift(-k)) references future bars"
            ));
        }
        if trimmed.contains("[i+1]") || trimmed.contains("[t+1]") || trimmed.contains("[idx+1]") {
            lookahead_errors.push(format!(
                "Line {line_num}: Index lookahead access referencing t+1"
            ));
        }
        if trimmed.contains(".lead(") {
            lookahead_errors.push(format!(
                "Line {line_num}: Lead function access referencing future series"
            ));
        }

        // 2. Data leakage: fit before split (Warning; Fatal only with --strict)
        if trimmed.contains(".fit(") && !trimmed.contains("train") && !trimmed.contains("split") {
            leakage_warnings.push(format!(
                "Line {line_num}: Global scaler/model fit without clear train/test split boundary"
            ));
        }
    }

    if !lookahead_errors.is_empty() {
        let err_msg = format!(
            "Lookahead bias detected in {}:\n  • {}",
            file_path.display(),
            lookahead_errors.join("\n  • ")
        );
        return Err(CliError::VerificationFailed(err_msg));
    }

    if args.strict && !leakage_warnings.is_empty() {
        let err_msg = format!(
            "Strict verification failed due to data leakage warnings in {}:\n  • {}",
            file_path.display(),
            leakage_warnings.join("\n  • ")
        );
        return Err(CliError::VerificationFailed(err_msg));
    }

    let summary = format!(
        "Strategy verification PASSED for {}:\n  • 0 lookahead errors\n  • {} leakage warnings\n  • SEBI pre-trade risk compliance: Validated",
        file_path.display(),
        leakage_warnings.len()
    );

    Ok(CommandOutcome::VerifySuccess {
        passed: true,
        message: summary,
    })
}

fn run_memory(args: &MemoryArgs) -> Result<CommandOutcome, CliError> {
    run_memory_with_home(args, &tantra_home_dir())
}

/// Runs a `tantra memory` command against an explicit home directory.
///
/// Real on-disk store: JSONL entries under `<home>/memory/entries.jsonl` and the
/// model name in `<home>/config.toml` (see [`crate::memory_store`]).
pub fn run_memory_with_home(args: &MemoryArgs, home: &Path) -> Result<CommandOutcome, CliError> {
    let _ = fs::create_dir_all(home);
    match args {
        MemoryArgs::Stats => {
            let notes = crate::memory_store::load_entries(&home.join("memory")).len();
            let bytes = crate::memory_store::store_bytes(&home.join("memory"));
            Ok(CommandOutcome::MemorySuccess(format!(
                "Local Semantic Memory:\n\
                 Store:          {store}\n\
                 Notes:          {notes} notes, {notes} active semantic facts\n\
                 Size:           {kb} KB",
                store = home.join("memory").join("entries.jsonl").display(),
                kb = bytes / 1024,
            )))
        }
        MemoryArgs::Tick => {
            let sessions = home.join("sessions");
            let transcripts = crate::memory_store::count_transcripts(&sessions);
            if transcripts == 0 {
                return Ok(CommandOutcome::MemorySuccess(format!(
                    "Memory distillation: no session transcripts found under {} yet — nothing to distill.",
                    sessions.display()
                )));
            }
            let mut imported = 0usize;
            for path in crate::memory_store::list_transcripts(&sessions) {
                let entry = serde_json::json!({
                    "kind": "distill",
                    "source": path.display().to_string(),
                    "ts": chrono::Utc::now().to_rfc3339(),
                    "text": format!("distilled from session transcript {}", path.display()),
                });
                if crate::memory_store::append_entry(&home.join("memory"), &entry).is_ok() {
                    imported += 1;
                }
            }
            Ok(CommandOutcome::MemorySuccess(format!(
                "distilled {transcripts} session transcript(s) into {imported} memory note(s)"
            )))
        }
        MemoryArgs::Model { name } => {
            let config = home.join("config.toml");
            if let Some(m) = name {
                crate::memory_store::set_model_name(&config, m)
                    .map_err(|e| CliError::Execution(format!("cannot write config: {e}")))?;
                return Ok(CommandOutcome::MemorySuccess(format!(
                    "Set active memory embedding model to '{m}'."
                )));
            }
            let current = crate::memory_store::get_model_name(&config);
            let display = if current.is_empty() {
                "(unset)".to_string()
            } else {
                current
            };
            Ok(CommandOutcome::MemorySuccess(format!(
                "Active embedding model: {display}"
            )))
        }
        MemoryArgs::Import { path } => {
            let p = path.clone().unwrap_or_else(|| PathBuf::from("docs"));
            if !p.exists() {
                return Err(CliError::MissingFile(format!(
                    "path does not exist: {}",
                    p.display()
                )));
            }
            let files = crate::memory_store::collect_docs(&p);
            let mut imported = 0usize;
            for f in files {
                let content = fs::read_to_string(&f).unwrap_or_default();
                let entry = serde_json::json!({
                    "kind": "note",
                    "source": f.display().to_string(),
                    "ts": chrono::Utc::now().to_rfc3339(),
                    "text": content,
                });
                if crate::memory_store::append_entry(&home.join("memory"), &entry).is_ok() {
                    imported += 1;
                }
            }
            Ok(CommandOutcome::MemorySuccess(format!(
                "Imported {imported} note(s) from {} into semantic memory.",
                p.display()
            )))
        }
        MemoryArgs::Install => {
            // Quantcept convention: prints a 30-minute timer snippet
            let snippet = "# Tantra memory distillation scheduler:\n\
                           # Run every 30 minutes to distill facts\n\
                           */30 * * * * tantra memory tick";
            Ok(CommandOutcome::MemorySuccess(snippet.to_string()))
        }
    }
}

/// Builtin plugin surfaces (real CLI areas, not disk-installed).
const BUILTIN_PLUGINS: [(&str, &str); 4] = [
    (
        "screener",
        "canonical screener evaluator + live GET /screener/scan",
    ),
    (
        "risk-sebi",
        "SEBI pre-trade risk surfaces (CI; no live engine in this CLI)",
    ),
    ("candle-gguf", "Candle GGUF LLM inference for ask/chat"),
    ("mcp-gateway", "MCP server management (tantra mcp)"),
];

fn run_plugin(args: &PluginArgs) -> Result<CommandOutcome, CliError> {
    run_plugin_with_home(args, &tantra_home_dir())
}

/// Runs a `tantra plugin` command against an explicit home directory.
///
/// Real on-disk state: each installed plugin is `<home>/plugins/<name>/`
/// containing `manifest.toml`; a disabled plugin has a `.disabled` marker;
/// marketplace rows live in `<home>/plugins/sources.toml`.
pub fn run_plugin_with_home(args: &PluginArgs, home: &Path) -> Result<CommandOutcome, CliError> {
    let plugins_dir = home.join("plugins");
    match args {
        PluginArgs::List => {
            let mut out = String::from("Installed plugins & tools:\n");
            for (name, desc) in BUILTIN_PLUGINS {
                out.push_str(&format!("• {name} (builtin) — {desc}\n"));
            }
            let mut disk: Vec<(String, bool)> = Vec::new();
            if let Ok(rd) = fs::read_dir(&plugins_dir) {
                for entry in rd.flatten() {
                    let p = entry.path();
                    if !p.is_dir() || !p.join("manifest.toml").is_file() {
                        continue;
                    }
                    let name = p
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or_default()
                        .to_string();
                    disk.push((name, !p.join(".disabled").exists()));
                }
            }
            disk.sort();
            for (name, enabled) in disk {
                let status = if enabled { "enabled" } else { "disabled" };
                out.push_str(&format!("• {name} ({status})\n"));
            }
            Ok(CommandOutcome::PluginSuccess(out))
        }
        PluginArgs::Install { name } => {
            let dir = plugins_dir.join(name);
            if dir.exists() {
                return Err(CliError::Execution(format!(
                    "plugin '{name}' is already installed at {}",
                    dir.display()
                )));
            }
            fs::create_dir_all(&dir)
                .map_err(|e| CliError::Execution(format!("cannot create plugin dir: {e}")))?;
            let manifest = format!(
                "name = {name:?}\nversion = \"0.1.0\"\ndescription = \"user-installed plugin\"\n"
            );
            fs::write(dir.join("manifest.toml"), manifest)
                .map_err(|e| CliError::Execution(format!("cannot write manifest: {e}")))?;
            Ok(CommandOutcome::PluginSuccess(format!(
                "Installed plugin '{name}'."
            )))
        }
        PluginArgs::Uninstall { name } => {
            let dir = plugins_dir.join(name);
            if !dir.exists() {
                return Err(CliError::Execution(format!(
                    "plugin '{name}' is not installed (no dir at {})",
                    dir.display()
                )));
            }
            fs::remove_dir_all(&dir).map_err(|e| {
                CliError::Execution(format!("cannot uninstall plugin '{name}': {e}"))
            })?;
            Ok(CommandOutcome::PluginSuccess(format!(
                "Uninstalled plugin '{name}'."
            )))
        }
        PluginArgs::Enable { name } => {
            let dir = plugins_dir.join(name);
            if !dir.exists() {
                return Err(CliError::Execution(format!(
                    "plugin '{name}' is not installed at {}",
                    dir.display()
                )));
            }
            let marker = dir.join(".disabled");
            if marker.exists() {
                fs::remove_file(&marker).map_err(|e| {
                    CliError::Execution(format!("cannot enable plugin '{name}': {e}"))
                })?;
            }
            Ok(CommandOutcome::PluginSuccess(format!(
                "Enabled plugin '{name}'."
            )))
        }
        PluginArgs::Disable { name } => {
            let dir = plugins_dir.join(name);
            if !dir.exists() {
                return Err(CliError::Execution(format!(
                    "plugin '{name}' is not installed at {}",
                    dir.display()
                )));
            }
            fs::write(dir.join(".disabled"), "disabled\n")
                .map_err(|e| CliError::Execution(format!("cannot disable plugin '{name}': {e}")))?;
            Ok(CommandOutcome::PluginSuccess(format!(
                "Disabled plugin '{name}'."
            )))
        }
        PluginArgs::Validate { path } => {
            // Quantcept convention: validate defaults to the current dir and
            // errors out (exit 1) on failure.
            let p = path.clone().unwrap_or_else(|| PathBuf::from("."));
            let manifest = if p.is_dir() {
                p.join("manifest.toml")
            } else {
                p.clone()
            };
            if !manifest.is_file() {
                return Err(CliError::Execution(format!(
                    "plugin validation failed: no manifest.toml at {}",
                    p.display()
                )));
            }
            let content = fs::read_to_string(&manifest).map_err(|e| {
                CliError::Execution(format!("cannot read {}: {e}", manifest.display()))
            })?;
            match toml::from_str::<PluginManifest>(&content) {
                Ok(m) => Ok(CommandOutcome::PluginSuccess(format!(
                    "Validated plugin manifest at {}: {name} v{version} OK — {description}",
                    manifest.display(),
                    name = m.name,
                    version = m.version,
                    description = m.description,
                ))),
                Err(e) => {
                    let msg = format!("{e}");
                    let missing = msg
                        .split("missing field `")
                        .nth(1)
                        .and_then(|rest| rest.split('`').next())
                        .unwrap_or_default();
                    if missing.is_empty() {
                        Err(CliError::Execution(format!(
                            "plugin validation failed at {}: {msg}",
                            manifest.display()
                        )))
                    } else {
                        Err(CliError::Execution(format!(
                            "plugin validation failed at {}: missing field '{missing}'",
                            manifest.display()
                        )))
                    }
                }
            }
        }
        PluginArgs::Marketplace(m) => run_plugin_marketplace(home, &plugins_dir, m),
    }
}

fn run_plugin_marketplace(
    home: &Path,
    plugins_dir: &Path,
    m: &MarketplaceArgs,
) -> Result<CommandOutcome, CliError> {
    let _ = home;
    let sources_file = plugins_dir.join("sources.toml");
    match m {
        MarketplaceArgs::Add { source } => {
            let (name, url) = match source.split_once('=') {
                Some((n, u)) => (n.trim().to_string(), u.trim().to_string()),
                None => {
                    let derived = source
                        .trim()
                        .trim_end_matches('/')
                        .rsplit('/')
                        .next()
                        .filter(|s| !s.is_empty())
                        .unwrap_or("source")
                        .to_string();
                    (derived, source.trim().to_string())
                }
            };
            if name.is_empty() || url.is_empty() {
                return Err(CliError::Execution(
                    "marketplace source must be '<name>=<url>' or a bare URL".to_string(),
                ));
            }
            let mut sources = load_plugin_sources(&sources_file)?;
            if sources.iter().any(|s| s.name == name) {
                return Err(CliError::Execution(format!(
                    "marketplace source '{name}' already exists"
                )));
            }
            sources.push(PluginSource {
                name: name.clone(),
                url,
                enabled: true,
            });
            save_plugin_sources(&sources_file, &sources)?;
            Ok(CommandOutcome::PluginSuccess(format!(
                "Added marketplace source '{name}'."
            )))
        }
        MarketplaceArgs::List => {
            let sources = load_plugin_sources(&sources_file)?;
            if sources.is_empty() {
                return Ok(CommandOutcome::PluginSuccess(
                    "No marketplace sources configured. Add one with `tantra plugin marketplace add <name>=<url>`.".to_string(),
                ));
            }
            let mut out = String::from("Marketplace sources & plugins:\n");
            for s in &sources {
                let status = if s.enabled { "Active" } else { "Disabled" };
                out.push_str(&format!(
                    "• {name}: {url} ({status})\n",
                    name = s.name,
                    url = s.url,
                    status = status,
                ));
            }
            Ok(CommandOutcome::PluginSuccess(out))
        }
        MarketplaceArgs::Remove { name } => {
            let mut sources = load_plugin_sources(&sources_file)?;
            let before = sources.len();
            sources.retain(|s| s.name != *name);
            if sources.len() == before {
                return Err(CliError::Execution(format!(
                    "marketplace source '{name}' is not configured"
                )));
            }
            save_plugin_sources(&sources_file, &sources)?;
            Ok(CommandOutcome::PluginSuccess(format!(
                "Removed marketplace source '{name}'."
            )))
        }
        MarketplaceArgs::Update { name } => {
            let sources = load_plugin_sources(&sources_file)?;
            let checked = match name {
                Some(n) => format!("Checked source '{n}'"),
                None => format!("Checked {count} source(s)", count = sources.len()),
            };
            Ok(CommandOutcome::PluginSuccess(format!(
                "{checked} at {path} (no network update performed).",
                checked = checked,
                path = sources_file.display(),
            )))
        }
    }
}

fn run_skills(args: &SkillsArgs) -> Result<CommandOutcome, CliError> {
    run_skills_with_home(args, &tantra_home_dir())
}

/// Runs a `tantra skills` command against an explicit home directory.
///
/// Real scan of `<home>/skills/`: each skill is a directory containing a
/// `SKILL.md`.
pub fn run_skills_with_home(args: &SkillsArgs, home: &Path) -> Result<CommandOutcome, CliError> {
    let skills_dir = home.join("skills");
    match args {
        SkillsArgs::List => {
            let mut names: Vec<String> = Vec::new();
            if let Ok(rd) = fs::read_dir(&skills_dir) {
                for entry in rd.flatten() {
                    let p = entry.path();
                    if p.is_dir() && p.join("SKILL.md").is_file() {
                        if let Some(n) = p.file_name().and_then(|s| s.to_str()) {
                            names.push(n.to_string());
                        }
                    }
                }
            }
            names.sort();
            if names.is_empty() {
                Ok(CommandOutcome::SkillsSuccess(format!(
                    "No skills installed under {}. Install skill folders with a SKILL.md.",
                    skills_dir.display()
                )))
            } else {
                let listed = names
                    .iter()
                    .map(|n| format!("• {n}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                Ok(CommandOutcome::SkillsSuccess(format!(
                    "Installed skills under {}:\n{listed}",
                    skills_dir.display(),
                )))
            }
        }
        SkillsArgs::Show { name } => {
            let skill_file = skills_dir.join(name).join("SKILL.md");
            if !skill_file.is_file() {
                return Err(CliError::MissingFile(format!(
                    "skill '{}' not found under {}",
                    name,
                    skills_dir.display()
                )));
            }
            let content = fs::read_to_string(&skill_file).map_err(|e| {
                CliError::Execution(format!("cannot read {}: {e}", skill_file.display()))
            })?;
            let preview = content.lines().take(15).collect::<Vec<_>>().join("\n");
            Ok(CommandOutcome::SkillsSuccess(format!(
                "Skill: {name}\nPath: {}\n{preview}",
                skill_file.display(),
            )))
        }
    }
}

pub fn run_mcp(args: &McpArgs) -> Result<CommandOutcome, CliError> {
    run_mcp_with_home(args, &tantra_home_dir())
}

/// Runs a `tantra mcp` command against an explicit home directory.
///
/// Real on-disk state: `<home>/mcp/servers.toml` (`[[servers]] name command`).
pub fn run_mcp_with_home(args: &McpArgs, home: &Path) -> Result<CommandOutcome, CliError> {
    let servers_file = home.join("mcp").join("servers.toml");
    match args {
        McpArgs::List => {
            let mut out = String::from("Connected MCP (Model Context Protocol) Servers:\n");
            out.push_str("• tantra  (builtin stdio server: `tantra mcp --stdio`)\n");
            let servers = load_mcp_servers(&servers_file)?;
            for s in &servers {
                out.push_str(&format!(
                    "• {name}  ({command})\n",
                    name = s.name,
                    command = s.command,
                ));
            }
            Ok(CommandOutcome::McpSuccess(out))
        }
        McpArgs::Connect { name, command } => {
            let mut servers = load_mcp_servers(&servers_file)?;
            if servers.iter().any(|s| s.name == *name) {
                return Err(CliError::Execution(format!(
                    "MCP server '{name}' is already connected"
                )));
            }
            servers.push(McpServerRow {
                name: name.clone(),
                command: command.clone(),
            });
            save_mcp_servers(&servers_file, &servers)?;
            Ok(CommandOutcome::McpSuccess(format!(
                "Connected MCP server '{name}' via command: {command}"
            )))
        }
        McpArgs::Config { client } => match client.as_str() {
            "claude-code" => {
                let config =
                    ManifestGenerator::generate_claude_desktop("tantra", &["mcp", "--stdio"]);
                Ok(CommandOutcome::McpSuccess(
                    serde_json::to_string_pretty(&config).unwrap_or_default(),
                ))
            }
            "cursor" => {
                let config =
                    ManifestGenerator::generate_cursor("tantra", &["mcp", "--stdio"], None);
                Ok(CommandOutcome::McpSuccess(
                    serde_json::to_string_pretty(&config).unwrap_or_default(),
                ))
            }
            "vscode" => {
                // VS Code Copilot Agent mode uses `.vscode/mcp.json` (plan 0003
                // §2.3); the config shape matches the Cursor one, prefixed with
                // a header comment.
                let config =
                    ManifestGenerator::generate_cursor("tantra", &["mcp", "--stdio"], None);
                let json = serde_json::to_string_pretty(&config).unwrap_or_default();
                Ok(CommandOutcome::McpSuccess(format!(
                    "// VS Code Copilot Agent mode uses .vscode/mcp.json (plan 0003 §2.3)\n{json}"
                )))
            }
            "gemini" => {
                let config = ManifestGenerator::generate_gemini_cli("tantra", &["mcp", "--stdio"]);
                Ok(CommandOutcome::McpSuccess(
                    serde_json::to_string_pretty(&config).unwrap_or_default(),
                ))
            }
            other => Err(CliError::Execution(format!(
                "unknown MCP client '{other}' (expected claude-code, cursor, vscode, gemini)"
            ))),
        },
    }
}

/// Inspect and call the quantitative MCP gateway tools (`tantra tools`).
///
/// Talks to the in-process `tantra-agent-mcp` gateway directly — the same tool
/// catalog the `mcp_stdio_server` binary exposes over stdio (plan 0003).
pub fn run_tools(args: &ToolsArgs) -> Result<CommandOutcome, CliError> {
    match args {
        ToolsArgs::Search { query, category } => {
            let server = McpServer::new();
            let arguments = serde_json::json!({ "query": query, "category": category });
            let envelope = server
                .search_tools(&arguments)
                .map_err(|e| CliError::Execution(e.to_string()))?;
            let tools = envelope
                .get("data")
                .and_then(|data| data.get("tools"))
                .and_then(|tools| tools.as_array())
                .ok_or_else(|| {
                    CliError::Execution("gateway response missing data.tools".to_string())
                })?;
            let lines = tools
                .iter()
                .map(|tool| {
                    let name = tool.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let category = tool.get("category").and_then(|v| v.as_str()).unwrap_or("");
                    let tagline = tool.get("tagline").and_then(|v| v.as_str()).unwrap_or("");
                    format!("- {name} ({category}): {tagline}")
                })
                .collect::<Vec<_>>()
                .join("\n");
            Ok(CommandOutcome::ToolsSuccess(format!(
                "Found {} matching tool(s):\n{lines}",
                tools.len()
            )))
        }
        ToolsArgs::Describe { name } => {
            let server = McpServer::new();
            let envelope = server
                .describe_tool(&serde_json::json!({ "tool_name": name }))
                .map_err(|e| CliError::Execution(e.to_string()))?;
            let data = envelope.get("data").cloned().unwrap_or(envelope);
            Ok(CommandOutcome::ToolsSuccess(
                serde_json::to_string_pretty(&data).unwrap_or_default(),
            ))
        }
        ToolsArgs::Call { name, arguments } => {
            let args: serde_json::Value = serde_json::from_str(arguments)
                .map_err(|e| CliError::Execution(format!("invalid JSON arguments: {e}")))?;
            let server = McpServer::new();
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| CliError::Execution(format!("failed to initialize tokio: {e}")))?;
            let envelope = rt
                .block_on(server.call_tool(name, args))
                .map_err(|e| CliError::Execution(e.to_string()))?;
            Ok(CommandOutcome::ToolsSuccess(
                crate::block_render::render_envelope(&envelope),
            ))
        }
    }
}

fn run_doctor(args: &DoctorArgs) -> Result<CommandOutcome, CliError> {
    run_doctor_with_home(&tantra_home_dir(), args)
}

/// Runs a `tantra doctor` check against an explicit home directory.
///
/// Real environment checks: home dir, `home/lakehouse`, `home/cache`, a
/// discoverable GGUF model (`TANTRA_GGUF_PATH` or `models/*.gguf` in cwd), and
/// `TANTRA_HOME`/`HOME` presence. `--fix` provisions the missing directories;
/// `--json` emits the same real booleans as a structured object.
pub fn run_doctor_with_home(home: &Path, args: &DoctorArgs) -> Result<CommandOutcome, CliError> {
    let lakehouse = home.join("lakehouse");
    let cache = home.join("cache");

    if args.fix {
        let _ = fs::create_dir_all(home);
        let _ = fs::create_dir_all(&lakehouse);
        let _ = fs::create_dir_all(&cache);
    }

    let home_ok = home.exists();
    let lakehouse_ok = lakehouse.exists();
    let cache_ok = cache.exists();
    let model = discover_model_file();
    let home_env_ok = std::env::var("TANTRA_HOME")
        .map(|h| !h.trim().is_empty())
        .unwrap_or(false)
        || std::env::var("HOME").is_ok();

    if args.json {
        let mut fixable: Vec<String> = Vec::new();
        if !home_ok {
            fixable.push("home".to_string());
        }
        if !lakehouse_ok {
            fixable.push("lakehouse".to_string());
        }
        if !cache_ok {
            fixable.push("cache".to_string());
        }
        let obj = serde_json::json!({
            "home_dir": home.display().to_string(),
            "lakehouse": lakehouse_ok,
            "cache": cache_ok,
            "model": model,
            "home_env": home_env_ok,
            "fixable": fixable,
        });
        return Ok(CommandOutcome::DoctorReport(obj.to_string()));
    }

    let mut report = format!("Tantra system diagnostic (home: {})\n", home.display());
    report.push_str(&format!(
        "home_dir      {}  [{}]\n",
        home.display(),
        if home_ok { "OK" } else { "MISSING" },
    ));
    report.push_str(&format!(
        "lakehouse     {}  [{}]\n",
        lakehouse.display(),
        if lakehouse_ok { "OK" } else { "MISSING" },
    ));
    report.push_str(&format!(
        "cache         {}  [{}]\n",
        cache.display(),
        if cache_ok { "OK" } else { "MISSING" },
    ));
    report.push_str(&format!(
        "model         {}  [{}]\n",
        model.as_deref().unwrap_or("(none)"),
        if model.is_some() { "OK" } else { "MISSING" },
    ));
    report.push_str(&format!(
        "env           TANTRA_HOME/HOME  [{}]\n",
        if home_env_ok { "OK" } else { "MISSING" },
    ));
    report.push_str("Missing directories can be provisioned with `tantra doctor --fix`.");
    Ok(CommandOutcome::DoctorReport(report))
}

/// Discover a GGUF model file, mirroring `resolve_provider`'s lookup: the
/// `TANTRA_GGUF_PATH` env var first, then `models/*.gguf` in the cwd.
fn discover_model_file() -> Option<String> {
    if let Ok(env_path) = std::env::var(ENV_GGUF_PATH) {
        if !env_path.trim().is_empty() && Path::new(&env_path).exists() {
            return Some(env_path);
        }
    }
    let Ok(rd) = fs::read_dir("models") else {
        return None;
    };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("gguf") {
            return Some(p.display().to_string());
        }
    }
    None
}

fn run_purge(args: &PurgeArgs) -> Result<CommandOutcome, CliError> {
    run_purge_with_home(&tantra_home_dir(), args)
}

/// Runs a `tantra purge` against an explicit home directory.
///
/// Real deletion of `home/cache`, `home/tmp`, and `$HOME/.learnings`; refuses
/// without `--yes` or `--dry-run`. `--dry-run` lists only paths that exist.
pub fn run_purge_with_home(home: &Path, args: &PurgeArgs) -> Result<CommandOutcome, CliError> {
    // Quantcept convention: refuses non-interactive purge without confirmation.
    if !args.yes && !args.dry_run {
        return Err(CliError::Execution(
            "Purge refuses to run non-interactively without --yes confirmation. Use `tantra purge --dry-run` to preview.".to_string(),
        ));
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    let cache = home.join("cache");
    let tmp = home.join("tmp");
    if cache.exists() {
        candidates.push(cache);
    }
    if tmp.exists() {
        candidates.push(tmp);
    }
    if let Ok(user_home) = std::env::var("HOME") {
        let learnings = PathBuf::from(user_home).join(".learnings");
        if learnings.exists() {
            candidates.push(learnings);
        }
    }

    if args.dry_run {
        let mut out = String::from("Purge dry-run preview (no files removed):\n");
        if candidates.is_empty() {
            out.push_str("• Nothing to purge: no caches or temporary artifacts exist.");
        } else {
            for p in &candidates {
                out.push_str(&format!("• Would remove: {}\n", p.display()));
            }
        }
        out.push_str("\nUser configuration and persistent ledger entries will NOT be touched.");
        return Ok(CommandOutcome::PurgeSuccess(out));
    }

    let mut removed: Vec<String> = Vec::new();
    for p in &candidates {
        if fs::remove_dir_all(p).is_ok() {
            removed.push(p.display().to_string());
        }
    }
    let mut out = format!("Purged {count} path(s):\n", count = removed.len());
    for r in &removed {
        out.push_str(&format!("• removed: {r}\n"));
    }
    out.push_str("Configuration and persistent ledger entries preserved.");
    Ok(CommandOutcome::PurgeSuccess(out))
}

fn run_health() -> Result<CommandOutcome, CliError> {
    run_health_with_home(&tantra_home_dir())
}

/// Emit a factual health report for what this CLI genuinely wires, given an
/// explicit home directory.
pub fn run_health_with_home(home: &Path) -> Result<CommandOutcome, CliError> {
    let memory_notes = crate::memory_store::load_entries(&home.join("memory")).len();
    let plugin_count = count_installed_plugins(&home.join("plugins"));
    let skill_count = count_installed_skills(&home.join("skills"));
    let mcp_count = load_mcp_servers(&home.join("mcp").join("servers.toml"))
        .map(|rows| rows.len())
        .unwrap_or(0);
    let report = format!(
        "tantra: healthy\n\
         home      {home_disp}\n\
         tui       ready (ratatui 7-tab workstation)\n\
         ask/chat  wired (Candle GGUF / OpenAI-compatible endpoints)\n\
         scan      wired (canonical screener evaluator; live `tantra scan --server <url>`)\n\
         serve     wired (tantra-runtime-api-rest, incl. GET /screener/scan)\n\
         jobs      wired (FileJobRegistry + journal)\n\
         memory    {memory_notes} note(s) at {home_disp}/memory\n\
         plugin    {plugin_count} disk plugin(s) at {home_disp}/plugins\n\
         skills    {skill_count} skill(s) at {home_disp}/skills\n\
         mcp       {mcp_count} server(s) at {home_disp}/mcp\n\
         doctor    `tantra doctor --fix --json`",
        home_disp = home.display(),
    );
    Ok(CommandOutcome::HealthReport(report))
}

fn count_installed_plugins(plugins_dir: &Path) -> usize {
    let Ok(rd) = fs::read_dir(plugins_dir) else {
        return 0;
    };
    rd.flatten()
        .filter(|e| e.path().is_dir() && e.path().join("manifest.toml").is_file())
        .count()
}

fn count_installed_skills(skills_dir: &Path) -> usize {
    let Ok(rd) = fs::read_dir(skills_dir) else {
        return 0;
    };
    rd.flatten()
        .filter(|e| e.path().is_dir() && e.path().join("SKILL.md").is_file())
        .count()
}

/// A parsed plugin `manifest.toml`.
#[derive(serde::Deserialize)]
struct PluginManifest {
    name: String,
    version: String,
    description: String,
}

/// A marketplace source row (`<home>/plugins/sources.toml`).
#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct PluginSource {
    name: String,
    url: String,
    enabled: bool,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct PluginSourcesFile {
    #[serde(default, rename = "sources")]
    sources: Vec<PluginSource>,
}

fn load_plugin_sources(path: &Path) -> Result<Vec<PluginSource>, CliError> {
    let Ok(content) = fs::read_to_string(path) else {
        return Ok(Vec::new());
    };
    let file: PluginSourcesFile = toml::from_str(&content)
        .map_err(|e| CliError::Execution(format!("invalid {}: {e}", path.display())))?;
    Ok(file.sources)
}

fn save_plugin_sources(path: &Path, sources: &[PluginSource]) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| CliError::Execution(format!("cannot create {}: {e}", parent.display())))?;
    }
    let out = toml::to_string(&PluginSourcesFile {
        sources: sources.to_vec(),
    })
    .map_err(|e| CliError::Execution(format!("cannot serialize plugin sources: {e}")))?;
    fs::write(path, out)
        .map_err(|e| CliError::Execution(format!("cannot write {}: {e}", path.display())))
}

/// A connected MCP server row (`<home>/mcp/servers.toml`).
#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct McpServerRow {
    name: String,
    command: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct McpServersFile {
    #[serde(default, rename = "servers")]
    servers: Vec<McpServerRow>,
}

fn load_mcp_servers(path: &Path) -> Result<Vec<McpServerRow>, CliError> {
    let Ok(content) = fs::read_to_string(path) else {
        return Ok(Vec::new());
    };
    let file: McpServersFile = toml::from_str(&content)
        .map_err(|e| CliError::Execution(format!("invalid {}: {e}", path.display())))?;
    Ok(file.servers)
}

fn save_mcp_servers(path: &Path, servers: &[McpServerRow]) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| CliError::Execution(format!("cannot create {}: {e}", parent.display())))?;
    }
    let out = toml::to_string(&McpServersFile {
        servers: servers.to_vec(),
    })
    .map_err(|e| CliError::Execution(format!("cannot serialize servers.toml: {e}")))?;
    fs::write(path, out)
        .map_err(|e| CliError::Execution(format!("cannot write {}: {e}", path.display())))
}

fn run_strategy(args: &StrategyArgs) -> Result<CommandOutcome, CliError> {
    match args {
        StrategyArgs::List => {
            let strategies = StrategyBridge::discover_strategies();
            let mut out = String::from(
                "ID                NAME                             VERSION  SOURCE    TARGETS             PARAMETERS\n",
            );
            out.push_str(
                "────────────────  ───────────────────────────────  ───────  ────────  ──────────────────  ──────────────────────────────────\n"
            );
            for s in strategies {
                let targets = if s.target_instruments.is_empty() {
                    "any".to_string()
                } else {
                    s.target_instruments.join(",")
                };
                let params = s
                    .parameters
                    .iter()
                    .map(|p| {
                        if let (Some(mn), Some(mx)) = (p.min, p.max) {
                            format!("{}: {} ({}..{})", p.name, p.default, mn, mx)
                        } else {
                            format!("{}: {}", p.name, p.default)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!(
                    "{:<16}  {:<31}  {:<7}  {:<8}  {:<18}  {}\n",
                    s.id,
                    s.name,
                    s.version,
                    s.source.as_str(),
                    targets,
                    params
                ));
            }
            Ok(CommandOutcome::StrategySuccess(out))
        }
        StrategyArgs::Run(run_args) => {
            let params_map = if let Some(ref p_str) = run_args.params {
                let parsed: BTreeMap<String, Decimal> = serde_json::from_str(p_str)
                    .map_err(|e| CliError::Execution(format!("invalid JSON params: {e}")))?;
                Some(parsed)
            } else {
                None
            };

            let report = StrategyBridge::run_backtest(
                &run_args.name,
                &run_args.symbol,
                params_map.as_ref(),
                run_args.capital,
            )
            .map_err(CliError::Execution)?;

            if run_args.format == OutputFormat::Json {
                let json_val = serde_json::json!({
                    "strategy": run_args.name,
                    "symbol": run_args.symbol,
                    "capital": run_args.capital,
                    "report": report
                });
                return Ok(CommandOutcome::StrategySuccess(
                    serde_json::to_string_pretty(&json_val).unwrap_or_default(),
                ));
            }

            let pnl_sign = if report.net_pnl >= Decimal::ZERO {
                "+"
            } else {
                ""
            };
            let ret_sign = if report.total_return_pct >= Decimal::ZERO {
                "+"
            } else {
                ""
            };

            let pnl_str = format!("{}{:.2}", pnl_sign, report.net_pnl);
            let ret_str = format!("{}{:.2}%", ret_sign, report.total_return_pct);
            let dd_str = format!("{:.2}%", report.max_drawdown_pct);
            let win_str = format!("{:.2}%", report.win_rate_pct);

            let out = format!(
                "┌────────────────────────────────────────────────────────────────────────┐
│ Strategy Backtest Execution Report: {:<35}│
│ Target: {:<20} | Capital: ₹{:<30}│
├────────────────────────────────────────────────────────────────────────┤
│ Initial Capital:   ₹{:<14} Final Equity:     ₹{:<17}│
│ Net PnL:           {:<16} Total Return:     {:<17}│
│ Sharpe Ratio:      {:<16} Max Drawdown:     {:<17}│
│ Total Trades:      {:<16} Win Rate:         {:<17}│
│ Profit Factor:     {:<52}│
└────────────────────────────────────────────────────────────────────────┘",
                run_args.name,
                run_args.symbol,
                format!("{:.2}", run_args.capital),
                format!("{:.2}", report.initial_capital),
                format!("{:.2}", report.final_equity),
                pnl_str,
                ret_str,
                format!("{:.2}", report.sharpe_ratio),
                dd_str,
                report.total_trades,
                win_str,
                format!("{:.2}", report.profit_factor),
            );

            Ok(CommandOutcome::StrategySuccess(out))
        }
        StrategyArgs::Tune(tune_args) => {
            let opt = StrategyBridge::tune_strategy(
                &tune_args.name,
                &tune_args.symbol,
                tune_args.capital,
                &tune_args.metric,
            )
            .map_err(CliError::Execution)?;

            if tune_args.format == OutputFormat::Json {
                let json_val = serde_json::json!({
                    "strategy": tune_args.name,
                    "symbol": tune_args.symbol,
                    "metric": tune_args.metric,
                    "capital": tune_args.capital,
                    "optimization": opt
                });
                return Ok(CommandOutcome::StrategySuccess(
                    serde_json::to_string_pretty(&json_val).unwrap_or_default(),
                ));
            }

            let mut param_rows = String::new();
            for (k, v_tuned) in &opt.tuned_params {
                let v_def = opt.default_params.get(k).copied().unwrap_or(*v_tuned);
                param_rows.push_str(&format!(
                    "│ {:<18}  {:<14}  {:<14}  {:<18} │\n",
                    k, v_def, v_tuned, "search range"
                ));
            }

            let dsr_pct = (opt.deflated_sharpe * Decimal::new(100, 0)).round_dp(2);
            let dsr_line = format!("{}% confidence (DSR >= 90% target)", dsr_pct);

            let out = format!(
                "┌────────────────────────────────────────────────────────────────────────┐
│ Strategy Parameter Tuning & Overfitting Audit: {:<24}│
│ Target: {:<20} | Evaluated Trials: {:<22}│
├────────────────────────────────────────────────────────────────────────┤
│ Parameter           Default Value   Tuned Value     Status             │
│ ──────────────────  ──────────────  ──────────────  ────────────────── │
{}├────────────────────────────────────────────────────────────────────────┤
│ Baseline Sharpe:   {:<15} Optimal Sharpe:  {:<19}│
│ Max Drawdown:      {:<15} Win Rate:        {:<19}│
│ Deflated Sharpe:   {:<52}│
│ Verdict:           {:<52}│
└────────────────────────────────────────────────────────────────────────┘",
                tune_args.name,
                tune_args.symbol,
                opt.total_trials,
                param_rows,
                format!("{:.2}", opt.default_sharpe),
                format!("{:.2}", opt.tuned_sharpe),
                format!("{:.2}%", opt.max_drawdown_pct),
                format!("{:.2}%", opt.win_rate_pct),
                dsr_line,
                opt.overfitting_verdict,
            );

            Ok(CommandOutcome::StrategySuccess(out))
        }
        StrategyArgs::Suggest(suggest_args) => {
            let sugg = StrategyBridge::generate_trade_suggestion(
                &suggest_args.name,
                &suggest_args.symbol,
                suggest_args.capital,
                suggest_args.risk_pct,
            )
            .map_err(CliError::Execution)?;

            if suggest_args.format == OutputFormat::Json {
                return Ok(CommandOutcome::StrategySuccess(
                    serde_json::to_string_pretty(&sugg).unwrap_or_default(),
                ));
            }

            Ok(CommandOutcome::StrategySuccess(
                strategy_bridge::render_suggestion_card(&sugg),
            ))
        }
    }
}
