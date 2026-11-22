//! CLI argument parsing (clap derive) implementing the Quantcept command hierarchy.

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::net::SocketAddr;
use std::path::PathBuf;

/// Log levels for CLI output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
    Silent,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
            Self::Silent => "silent",
        }
    }
}

/// Output format for ask and other machine-readable commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
}

/// Missed schedule policy for jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum MissedPolicy {
    #[default]
    Skip,
    CatchUp,
}

/// Approval gating for job runs (`jobs add --approval auto|manual`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum ApprovalPolicyArg {
    /// Runs within budget automatically; only budget breaches need approval.
    #[default]
    Auto,
    /// Hold every run behind a human approval gate.
    Manual,
}

/// Session mode for `tantra paper` (plan 0008 D4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum PaperMode {
    /// In-process `PaperExchange` engine (deterministic, offline).
    #[default]
    Embedded,
    /// Thin client of a `tantra paper serve` REST + `/ws/live` server.
    Server,
}

/// Arguments for `tantra study-loop` and upgraded `tantra elephant` (`Track T21.4`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Args)]
pub struct StudyLoopArgs {
    /// Registered strategy identifier (`elephant_edge_v3`, `orb_15`, etc.).
    #[arg(long)]
    pub strategy: Option<String>,
    /// Target symbol (`NSE:NIFTY50-INDEX`, `NSE:RELIANCE-EQ`, `MCX:GOLD`).
    #[arg(short, long)]
    pub symbol: Option<String>,
    /// Bar timeframe (`1d`, `5m`, `15m`, `1h`, `1m`).
    #[arg(long)]
    pub tf: Option<String>,
    /// Holding horizon in bars after signal entry.
    #[arg(long)]
    pub horizon: Option<usize>,
    /// Derivative expiry horizon (`day`, `weekly`, `monthly`, `commodity_cycle`).
    #[arg(long)]
    pub expiry_horizon: Option<String>,
    /// Outer stop-loss ATR buffer multiplier (`0.00..=3.00`, default `0.25`).
    #[arg(long)]
    pub sl_buffer: Option<rust_decimal::Decimal>,
    /// Entry filter mode (`auto`, `rejection_wick`, `zone_touch_close`, `orb_breakout`).
    #[arg(long)]
    pub entry_mode: Option<String>,
    /// Run IS (70%) / OOS (30%) + DSR parameter sweep & render leaderboard.
    #[arg(long)]
    pub sweep: bool,
    /// Synthesize F&O / Commodity expiry hedge legs & matrix.
    #[arg(long)]
    pub hedge: bool,
    /// Emit structured JSON output (`StrategyLoopViewModel`).
    #[arg(long)]
    pub json: bool,
    /// Export unified 6-layer `OverlayScene` JSON to file path.
    #[arg(long)]
    pub export_scene: Option<PathBuf>,
    /// Live runtime REST base URL (`POST /api/v2/studies/strategy-loop/*`).
    #[arg(long)]
    pub server: Option<String>,
    /// Optional start date (`YYYY-MM-DD`) for `--server` queries.
    #[arg(long)]
    pub start_date: Option<String>,
    /// Optional end date (`YYYY-MM-DD`) for `--server` queries.
    #[arg(long)]
    pub end_date: Option<String>,
}

impl StudyLoopArgs {
    /// True if any strategy-loop / scene-export / live-server flag was passed.
    pub fn has_loop_flags(&self) -> bool {
        self.strategy.is_some()
            || self.symbol.is_some()
            || self.tf.is_some()
            || self.horizon.is_some()
            || self.expiry_horizon.is_some()
            || self.sl_buffer.is_some()
            || self.entry_mode.is_some()
            || self.sweep
            || self.hedge
            || self.json
            || self.export_scene.is_some()
            || self.server.is_some()
            || self.start_date.is_some()
            || self.end_date.is_some()
    }
}

/// Arguments for `tantra paper`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct PaperArgs {
    /// Session mode: embedded (in-process engine) or server (thin client).
    #[arg(long, value_enum, default_value = "embedded")]
    pub mode: PaperMode,
    /// Initial INR cash for the embedded session (default 1000000).
    #[arg(long)]
    pub cash: Option<String>,
    /// Optional RNG seed for the embedded latency profile (deterministic replay).
    #[arg(long)]
    pub seed: Option<u64>,
    /// Server base URL for `--mode server` instructions.
    #[arg(long)]
    pub server: Option<String>,
}

/// Arguments for `tantra ask`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct AskArgs {
    /// The prompt to send to the agent.
    pub prompt: String,
    /// Maximum timeout in minutes (default 20).
    #[arg(long, default_value = "20")]
    pub timeout_min: u64,
    /// Specialized financial persona (quant, analyst, trader, risk-manager, etc.).
    #[arg(long)]
    pub persona: Option<String>,
    /// Runtime execution mode (paper, backtest).
    #[arg(long, default_value = "paper")]
    pub mode: String,
    /// Configuration profile.
    #[arg(long, default_value = "default")]
    pub profile: String,
    /// Output format (text or json).
    #[arg(long, value_enum, default_value = "text")]
    pub format: OutputFormat,
}

impl AskArgs {
    /// Construct a new AskArgs with default 20 min timeout.
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            timeout_min: 20,
            persona: None,
            mode: "paper".to_string(),
            profile: "default".to_string(),
            format: OutputFormat::Text,
        }
    }
}

/// Arguments for `tantra chat`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Args)]
pub struct ChatArgs {
    /// Optional system prompt instructions.
    #[arg(short = 's', long)]
    pub system_prompt: Option<String>,
}

/// Arguments for `tantra scan`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Args)]
pub struct ScanArgs {
    /// The screener query (e.g. `"roe>15 and pe between 10 30"`).
    pub query: String,
    /// Live screener server base URL (e.g. http://127.0.0.1:8000). When set, the
    /// scan runs against that server's `GET /api/v2/screener/scans`; when unset, the
    /// offline fixture universe is used.
    #[arg(long)]
    pub server: Option<String>,
    /// JSON array of instrument ids for the live path (e.g. '["TCS.NSE","INFY.NSE"]').
    /// Defaults to the fixture symbols when absent.
    #[arg(long)]
    pub universe: Option<String>,
    /// Bar timeframe for the live path (default "1d").
    #[arg(long, default_value = "1d")]
    pub tf: Option<String>,
    /// Evaluate on the bars known at this date (RFC3339 or ISO-8601, inclusive).
    #[arg(long)]
    pub as_of: Option<String>,
}

/// Arguments for `tantra serve`: boots `tantra-runtime-api-rest` as a standalone HTTP server
/// (market data, orders, `/profile` persistent-store routes) until `Ctrl-C`/`SIGTERM`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct ServeArgs {
    /// Market data directory (a `catalog/` tree or flat `SYMBOL.EXCHANGE.parquet` files).
    pub data_dir: PathBuf,
    /// Socket address to bind.
    #[arg(long, default_value = "127.0.0.1:8000")]
    pub addr: SocketAddr,
    /// Allowed CORS origin (repeatable); omit to disable CORS.
    #[arg(long = "cors-origin")]
    pub cors_origin: Vec<String>,
    /// Directory holding one JSON profile per user (`/profile` routes). Default `data/profiles`.
    #[arg(long)]
    pub profiles_dir: Option<PathBuf>,
    /// Directory holding one JSON portfolio per user (`/portfolio` routes). Default `data/portfolios`.
    #[arg(long)]
    pub portfolios_dir: Option<PathBuf>,
    /// Directory holding one sub-directory per backtest run. Default `data/journals`.
    #[arg(long)]
    pub journals_dir: Option<PathBuf>,
    /// Bearer-token file (TOML); enables auth when set. Omit to run with auth off.
    #[arg(long)]
    pub tokens_file: Option<PathBuf>,
}

/// Arguments for `tantra jobs`.
#[derive(Debug, Clone, PartialEq, Eq)]
// Clap command structs are built once per dispatch; boxing the Add field-set
// would ripple through `to_command`/tests without a real gain.
#[allow(clippy::large_enum_variant)]
pub enum JobsArgs {
    Add {
        goal: String,
        schedule: Option<String>,
        max_turns: usize,
        max_inr: Option<rust_decimal::Decimal>,
        max_usd: Option<rust_decimal::Decimal>,
        max_tokens: Option<usize>,
        max_tool_calls: Option<usize>,
        max_data_calls: Option<usize>,
        missed_policy: MissedPolicy,
        approval: ApprovalPolicyArg,
        strategy: Option<String>,
        symbol: Option<String>,
        capital: Option<rust_decimal::Decimal>,
        params: Option<String>,
        success: Option<String>,
        read_only: bool,
        once: bool,
    },
    List,
    Run {
        id: String,
    },
    Tick {
        all: bool,
    },
    Install,
    Logs {
        id: String,
    },
    Pause {
        id: String,
    },
    Resume {
        id: String,
    },
    Approvals,
    Approve {
        id: String,
    },
    Deny {
        id: String,
    },
}

/// Arguments for `tantra verify`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Args)]
pub struct VerifyArgs {
    /// Path to the strategy file or algorithm script to verify.
    pub file: Option<PathBuf>,
    /// Treat warnings (e.g. fit-before-split) as fatal errors.
    #[arg(long)]
    pub strict: bool,
}

/// Arguments for `tantra memory`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryArgs {
    Tick,
    Stats,
    Model { name: Option<String> },
    Import { path: Option<PathBuf> },
    Install,
}

/// Marketplace subcommands for plugin management.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketplaceArgs {
    Add { source: String },
    List,
    Remove { name: String },
    Update { name: Option<String> },
}

/// Arguments for `tantra plugin`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginArgs {
    List,
    Install { name: String },
    Uninstall { name: String },
    Enable { name: String },
    Disable { name: String },
    Validate { path: Option<PathBuf> },
    Marketplace(MarketplaceArgs),
}

/// Arguments for `tantra skills`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillsArgs {
    List,
    Show { name: String },
}

/// Arguments for `tantra mcp`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpArgs {
    List,
    Connect {
        name: String,
        command: String,
    },
    /// Generate MCP client config for an external coding agent.
    Config {
        client: String,
    },
}

/// Arguments for `tantra tools`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolsArgs {
    /// Search the quantitative MCP gateway tool catalog.
    Search {
        query: String,
        category: Option<String>,
    },
    /// Show the JSON schema for a gateway tool.
    Describe { name: String },
    /// Call a gateway tool and render its result envelope.
    Call { name: String, arguments: String },
}

/// Arguments for `tantra doctor`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Args)]
pub struct DoctorArgs {
    /// Automatically provision missing directories, tables, and sidecars.
    #[arg(long)]
    pub fix: bool,
    /// Emit health diagnostic report as JSON.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for `tantra purge`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Args)]
pub struct PurgeArgs {
    /// Display which files and caches would be removed without deleting.
    #[arg(long)]
    pub dry_run: bool,
    /// Confirm non-interactive deletion.
    #[arg(long)]
    pub yes: bool,
}

/// Arguments for `tantra strategy run`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct StrategyRunArgs {
    /// Strategy name or manifest id.
    pub name: String,
    /// Target trading symbol.
    #[arg(short, long, default_value = "NSE:NIFTY50-INDEX")]
    pub symbol: String,
    /// Parameter overrides as JSON (e.g. '{"fast_period":12}').
    #[arg(long)]
    pub params: Option<String>,
    /// Capital allocated for backtest (INR).
    #[arg(long, default_value = "1000000")]
    pub capital: rust_decimal::Decimal,
    /// Output format (text or json).
    #[arg(long, value_enum, default_value = "text")]
    pub format: OutputFormat,
}

/// Arguments for `tantra strategy tune`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct StrategyTuneArgs {
    /// Strategy name or manifest id to tune.
    pub name: String,
    /// Target trading symbol.
    #[arg(short, long, default_value = "NSE:NIFTY50-INDEX")]
    pub symbol: String,
    /// Optimization objective metric (sharpe or pnl).
    #[arg(long, default_value = "sharpe")]
    pub metric: String,
    /// Capital allocated for evaluation (INR).
    #[arg(long, default_value = "1000000")]
    pub capital: rust_decimal::Decimal,
    /// Output format (text or json).
    #[arg(long, value_enum, default_value = "text")]
    pub format: OutputFormat,
}

/// Arguments for `tantra strategy suggest`.
#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct StrategySuggestArgs {
    /// Strategy name or manifest id.
    pub name: String,
    /// Target trading symbol.
    #[arg(short, long, default_value = "NSE:NIFTY50-INDEX")]
    pub symbol: String,
    /// Account capital (INR).
    #[arg(long, default_value = "1000000")]
    pub capital: rust_decimal::Decimal,
    /// Percentage of account risked on trade (e.g. 1.0 for 1%).
    #[arg(long, default_value = "1.0")]
    pub risk_pct: rust_decimal::Decimal,
    /// Output format (text or json).
    #[arg(long, value_enum, default_value = "text")]
    pub format: OutputFormat,
}

/// Arguments for `tantra strategy`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StrategyArgs {
    List,
    Run(StrategyRunArgs),
    Tune(StrategyTuneArgs),
    Suggest(StrategySuggestArgs),
}

/// The CLI subcommands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Launch the Ratatui terminal workstation.
    Tui {
        /// Optional initial message or query.
        message: Option<String>,
    },
    /// Ask the agent one headless question.
    Ask(AskArgs),
    /// Interactive conversational chat with the agent.
    Chat(ChatArgs),
    /// Run a screener scan.
    Scan(ScanArgs),
    /// Boot the `tantra-runtime-api-rest` HTTP server.
    Serve(ServeArgs),
    /// Manage autonomous scheduled jobs and approvals.
    Jobs(JobsArgs),
    /// Verify a strategy file for lookahead bias and data leakage.
    Verify(VerifyArgs),
    /// Manage local semantic financial memory and distillation.
    Memory(MemoryArgs),
    /// Manage plugins, skills, and extensions.
    Plugin(PluginArgs),
    /// Manage financial skills and tool sets.
    Skills(SkillsArgs),
    /// Manage Model Context Protocol (MCP) servers and tools.
    Mcp(McpArgs),
    /// Inspect and call the quantitative MCP gateway tools.
    Tools(ToolsArgs),
    /// Diagnostic environment check and automatic repair.
    Doctor(DoctorArgs),
    /// Purge temporary caches and session traces.
    Purge(PurgeArgs),
    /// Print a system/environment health report.
    Health,
    /// Manage, run, tune, and get trade suggestions for algorithmic strategies.
    Strategy(StrategyArgs),
    /// Run a deterministic paper trading session (embedded engine or server client).
    Paper(PaperArgs),
    /// Print the EE+ Strategy Studio report or run live strategy-loop evaluation.
    Elephant(StudyLoopArgs),
    /// Generic Strategy Loop, Parameter Sweep, Expiry Hedge & OverlayScene CLI (`Track T21.4`).
    StudyLoop(StudyLoopArgs),
}

/// The parsed CLI arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct CliArgs {
    /// The command to run.
    pub command: Command,
    /// Optional path to a local GGUF model file.
    pub model: Option<String>,
    pub config: Option<PathBuf>,
    /// Sampling temperature (0.0 to 1.0).
    pub temperature: Option<f64>,
    /// Effective resolved log level.
    pub log_level: LogLevel,
    /// Verbose output flag.
    pub verbose: bool,
    /// Quiet output flag.
    pub quiet: bool,
    /// Skip tool permissions (auto-accept).
    pub skip_permissions: bool,
    /// Continue latest session.
    pub r#continue: bool,
    /// Resume session by ID.
    pub resume: Option<String>,
    /// Optional initial message for terminal launch.
    pub message: Option<String>,
    /// Runtime ACP endpoint the TUI connects to instead of the embedded agent.
    pub server: Option<String>,
    /// File holding the bearer token for `--server`.
    pub token_file: Option<PathBuf>,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            command: Command::Tui { message: None },
            model: None,
            config: None,
            temperature: None,
            log_level: LogLevel::Info,
            verbose: false,
            quiet: false,
            skip_permissions: false,
            r#continue: false,
            resume: None,
            message: None,
            server: None,
            token_file: None,
        }
    }
}

/// Parse CLI arguments using the clap derive API (used by the binary).
pub fn parse() -> CliArgs {
    <Cli as clap::Parser>::parse().into()
}

/// Resolve effective log level honoring Quantcept precedence:
/// 1. TANTRA_LOG_LEVEL or QUANTCEPT_LOG_LEVEL env var
/// 2. Explicit flag `--log-level`
/// 3. `--verbose` (debug) / `--quiet` (silent)
/// 4. settings.json (if present)
/// 5. default: info
pub fn resolve_log_level(flag_level: Option<LogLevel>, verbose: bool, quiet: bool) -> LogLevel {
    if quiet {
        return LogLevel::Silent;
    }
    if verbose {
        return LogLevel::Debug;
    }
    if let Ok(env_lvl) =
        std::env::var("TANTRA_LOG_LEVEL").or_else(|_| std::env::var("QUANTCEPT_LOG_LEVEL"))
    {
        match env_lvl.trim().to_lowercase().as_str() {
            "debug" => return LogLevel::Debug,
            "info" => return LogLevel::Info,
            "warn" | "warning" => return LogLevel::Warn,
            "error" => return LogLevel::Error,
            "silent" | "off" => return LogLevel::Silent,
            _ => {}
        }
    }
    if let Some(lvl) = flag_level {
        return lvl;
    }
    LogLevel::Info
}

/// The clap-defined CLI structure.
#[derive(Debug, Parser, Clone, PartialEq)]
#[command(
    name = "tantra",
    version,
    about = "Tantra Indian equities quantitative AI workstation & CLI",
    long_about = "tantra [message] | tantra ask <prompt> | tantra jobs | tantra verify | tantra doctor"
)]
pub struct Cli {
    /// Optional initial prompt or message to launch the workstation with.
    #[arg(value_name = "MESSAGE")]
    pub message: Option<String>,

    /// Optional path to a local GGUF model file (SmolLM2, etc.).
    #[arg(short = 'm', long = "model", global = true)]
    pub model: Option<String>,
    #[arg(long = "config", global = true)]
    pub config: Option<PathBuf>,

    /// Sampling temperature (0.0 to 1.0).
    #[arg(short = 't', long = "temperature", global = true)]
    pub temperature: Option<f64>,

    /// Log level (debug, info, warn, error, silent).
    #[arg(long = "log-level", global = true)]
    pub log_level: Option<LogLevel>,

    /// Verbose output (shortcut for --log-level debug).
    #[arg(short = 'v', long = "verbose", global = true)]
    pub verbose: bool,

    /// Quiet output (shortcut for --log-level silent).
    #[arg(short = 'q', long = "quiet", global = true)]
    pub quiet: bool,

    /// Auto-accept tool confirmation prompts.
    #[arg(
        long = "skip-permissions",
        aliases = ["dangerously-skip-permissions", "yolo"],
        global = true
    )]
    pub skip_permissions: bool,

    /// Resume the latest session in the current directory.
    #[arg(short = 'c', long = "continue", global = true)]
    pub r#continue: bool,

    /// Resume specific session by ID.
    #[arg(short = 'r', long = "resume", global = true)]
    pub resume: Option<String>,

    /// Connect the TUI to a runtime ACP endpoint, e.g. ws://127.0.0.1:8000/api/v2/acp (or just ws://127.0.0.1:8000), instead of the embedded agent.
    #[arg(long = "server", value_name = "WS_URL")]
    pub server: Option<String>,

    /// File holding the bearer token for --server.
    #[arg(long = "token-file", value_name = "PATH")]
    pub token_file: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<CliCommand>,
}

#[allow(clippy::derivable_impls)]
impl Default for Cli {
    fn default() -> Self {
        Self {
            message: None,
            model: None,
            config: None,
            temperature: None,
            log_level: None,
            verbose: false,
            quiet: false,
            skip_permissions: false,
            r#continue: false,
            resume: None,
            server: None,
            token_file: None,
            command: None,
        }
    }
}

/// The clap-defined subcommand enum.
#[derive(Debug, Subcommand, Clone, PartialEq, Eq)]
pub enum CliCommand {
    /// Launch the terminal workstation.
    Tui,
    /// Ask the agent one headless question.
    Ask {
        /// The prompt to send to the agent.
        prompt: String,
        /// Maximum timeout in minutes (default 20).
        #[arg(long, default_value = "20")]
        timeout_min: u64,
        /// Specialized financial persona (quant, analyst, trader, etc.).
        #[arg(long)]
        persona: Option<String>,
        /// Runtime execution mode (paper, backtest).
        #[arg(long, default_value = "paper")]
        mode: String,
        /// Configuration profile.
        #[arg(long, default_value = "default")]
        profile: String,
        /// Output format (text or json).
        #[arg(long, value_enum, default_value = "text")]
        format: OutputFormat,
    },
    /// Interactive conversational chat with the agent.
    Chat {
        /// Optional system prompt instructions.
        #[arg(short = 's', long)]
        system_prompt: Option<String>,
    },
    /// Run a screener scan.
    Scan {
        /// The screener query.
        query: String,
        /// Live screener server base URL; when unset the offline fixture is used.
        #[arg(long)]
        server: Option<String>,
        /// JSON array of instrument ids for the live path.
        #[arg(long)]
        universe: Option<String>,
        /// Bar timeframe for the live path (default "1d").
        #[arg(long, default_value = "1d")]
        tf: Option<String>,
        /// Evaluate on the bars known at this date.
        #[arg(long)]
        as_of: Option<String>,
    },
    /// Boot the `tantra-runtime-api-rest` HTTP server.
    Serve {
        /// Market data directory (a `catalog/` tree or flat `SYMBOL.EXCHANGE.parquet` files).
        data_dir: PathBuf,
        /// Socket address to bind.
        #[arg(long, default_value = "127.0.0.1:8000")]
        addr: SocketAddr,
        /// Allowed CORS origin (repeatable); omit to disable CORS.
        #[arg(long = "cors-origin")]
        cors_origin: Vec<String>,
        /// Directory holding one JSON profile per user (`/profile` routes). Default `data/profiles`.
        #[arg(long)]
        profiles_dir: Option<PathBuf>,
        /// Directory holding one JSON portfolio per user (`/portfolio` routes). Default `data/portfolios`.
        #[arg(long)]
        portfolios_dir: Option<PathBuf>,
        /// Directory holding one sub-directory per backtest run. Default `data/journals`.
        #[arg(long)]
        journals_dir: Option<PathBuf>,
        /// Bearer-token file (TOML); enables auth when set. Omit to run with auth off.
        #[arg(long)]
        tokens_file: Option<PathBuf>,
    },
    /// Manage autonomous scheduled jobs and approvals.
    Jobs {
        #[command(subcommand)]
        subcommand: Option<JobsSubcommand>,
    },
    /// Verify a strategy file for lookahead bias and data leakage.
    Verify {
        /// Path to the strategy file or algorithm script to verify.
        file: Option<PathBuf>,
        /// Treat warnings (e.g. fit-before-split) as fatal errors.
        #[arg(long)]
        strict: bool,
    },
    /// Manage local semantic financial memory and distillation.
    Memory {
        #[command(subcommand)]
        subcommand: Option<MemorySubcommand>,
    },
    /// Manage plugins, skills, and extensions.
    Plugin {
        #[command(subcommand)]
        subcommand: Option<PluginSubcommand>,
    },
    /// Manage installed financial skills and tool sets.
    Skills {
        #[command(subcommand)]
        subcommand: Option<SkillsSubcommand>,
    },
    /// Manage Model Context Protocol (MCP) servers and tools.
    Mcp {
        #[command(subcommand)]
        subcommand: Option<McpSubcommand>,
    },
    /// Inspect and call the quantitative MCP gateway tools.
    Tools {
        #[command(subcommand)]
        subcommand: Option<ToolsSubcommand>,
    },
    /// Diagnostic environment check and automatic repair.
    Doctor {
        /// Automatically provision missing directories, tables, and sidecars.
        #[arg(long)]
        fix: bool,
        /// Emit health diagnostic report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Purge temporary caches and session traces.
    Purge {
        /// Display which files and caches would be removed without deleting.
        #[arg(long)]
        dry_run: bool,
        /// Confirm non-interactive deletion.
        #[arg(long)]
        yes: bool,
    },
    /// Print a system/environment health report.
    Health,
    /// Manage, run, tune, and get trade suggestions for algorithmic strategies.
    Strategy {
        #[command(subcommand)]
        subcommand: Option<StrategySubcommand>,
    },
    /// Run a deterministic paper trading session (embedded engine or server client).
    Paper {
        /// Session mode: embedded (in-process engine) or server (thin client).
        #[arg(long, value_enum, default_value = "embedded")]
        mode: PaperMode,
        /// Initial INR cash for the embedded session (default 1000000).
        #[arg(long)]
        cash: Option<String>,
        /// Optional RNG seed for the embedded latency profile (deterministic replay).
        #[arg(long)]
        seed: Option<u64>,
        /// Server base URL for `--mode server` instructions.
        #[arg(long)]
        server: Option<String>,
    },
    /// Print the EE+ (Elephant Edge) Strategy Studio report or run strategy loop.
    Elephant(StudyLoopArgs),
    /// Run generic Strategy Loop, Parameter Sweep, Expiry Hedge & OverlayScene export.
    StudyLoop(StudyLoopArgs),
}

/// Jobs subcommands.
#[derive(Debug, Subcommand, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum JobsSubcommand {
    /// Add a new autonomous scheduled research job.
    Add {
        /// Natural language goal or task.
        #[arg(required = true, num_args = 1..)]
        goal: Vec<String>,
        /// Cron schedule expression (e.g. "30 7 * * 1-5") or schedule JSON.
        #[arg(long)]
        schedule: Option<String>,
        /// Max conversation turns per run (default: 20).
        #[arg(long, default_value = "20")]
        max_turns: usize,
        /// Max cost budget in INR (statutory/data/compute).
        #[arg(long)]
        max_inr: Option<rust_decimal::Decimal>,
        /// Max cost budget in USD.
        #[arg(long)]
        max_usd: Option<rust_decimal::Decimal>,
        /// Max token consumption cap.
        #[arg(long)]
        max_tokens: Option<usize>,
        /// Max tool calls permitted.
        #[arg(long)]
        max_tool_calls: Option<usize>,
        /// Max data calls permitted.
        #[arg(long)]
        max_data_calls: Option<usize>,
        /// Policy when a scheduled run is missed.
        #[arg(long, value_enum, default_value = "skip")]
        missed_policy: MissedPolicy,
        /// Approval gating before a run may execute.
        #[arg(long, value_enum, default_value = "auto")]
        approval: ApprovalPolicyArg,
        /// Strategy id the job executes (e.g. "sma_crossover"); jobs without a
        /// strategy are refused by `jobs run` because they have no payload.
        #[arg(long)]
        strategy: Option<String>,
        /// Canonical instrument the strategy targets (e.g. "NSE:RELIANCE-EQ").
        #[arg(long)]
        symbol: Option<String>,
        /// Backtest capital in INR (default 1_000_000).
        #[arg(long)]
        capital: Option<rust_decimal::Decimal>,
        /// Strategy parameter overrides as a JSON object (e.g. '{"fast":10}').
        #[arg(long)]
        params: Option<String>,
        /// Optional success criteria JSON.
        #[arg(long)]
        success: Option<String>,
        /// Enforce read-only advisory mode.
        #[arg(long, default_value = "true", action = clap::ArgAction::Set)]
        read_only: bool,
        /// Run immediately once and exit.
        #[arg(long)]
        once: bool,
    },
    /// List all scheduled and active jobs.
    List,
    /// Immediately run a specific job by ID.
    Run {
        /// The 8-character job ID.
        id: String,
    },
    /// Evaluate cron schedules against system time and run due jobs.
    Tick {
        /// Tick all workspaces rather than just the current directory.
        #[arg(long)]
        all: bool,
    },
    /// Print scheduler integration snippet (systemd timer or crontab).
    Install,
    /// Display logs and execution traces for a job.
    Logs {
        /// Job ID.
        id: String,
    },
    /// Pause a job.
    Pause {
        /// Job ID.
        id: String,
    },
    /// Resume a paused job.
    Resume {
        /// Job ID.
        id: String,
    },
    /// List pending actions requiring human approval.
    Approvals,
    /// Approve a pending job action.
    Approve {
        /// Action or Job ID.
        id: String,
    },
    /// Deny a pending job action.
    Deny {
        /// Action or Job ID.
        id: String,
    },
}

/// Memory subcommands.
#[derive(Debug, Subcommand, Clone, PartialEq, Eq)]
pub enum MemorySubcommand {
    /// Condense and distill recent conversation transcripts into semantic memory facts.
    Tick,
    /// Display statistics on semantic notes, vectors, and storage.
    Stats,
    /// Set or display the active embedding model name.
    Model {
        /// Embedding model identifier.
        name: Option<String>,
    },
    /// Import research notes, PDFs, or strategy markdown into local memory.
    Import {
        /// Path to directory or file to ingest.
        path: Option<PathBuf>,
    },
    /// Print systemd/cron timer snippet for memory auto-distillation.
    Install,
}

/// Plugin marketplace subcommands.
#[derive(Debug, Subcommand, Clone, PartialEq, Eq)]
pub enum MarketplaceSubcommand {
    /// Add an external marketplace source.
    Add {
        /// Marketplace source URL or path.
        source: String,
    },
    /// List marketplace sources and available packages.
    List,
    /// Remove an external marketplace source.
    Remove {
        /// Source identifier or name.
        name: String,
    },
    /// Update marketplace sources.
    Update {
        /// Optional marketplace name to update.
        name: Option<String>,
    },
}

/// Plugin subcommands.
#[derive(Debug, Subcommand, Clone, PartialEq, Eq)]
pub enum PluginSubcommand {
    /// List installed plugins and tools.
    List,
    /// Install a plugin by package or source.
    Install {
        /// Plugin name or package specification.
        name: String,
    },
    /// Uninstall a plugin.
    Uninstall {
        /// Plugin name.
        name: String,
    },
    /// Enable an installed plugin.
    Enable {
        /// Plugin name.
        name: String,
    },
    /// Disable an installed plugin.
    Disable {
        /// Plugin name.
        name: String,
    },
    /// Validate a plugin manifest at a given path.
    Validate {
        /// Path to plugin manifest directory.
        path: Option<PathBuf>,
    },
    /// Manage plugin marketplace sources.
    Marketplace {
        #[command(subcommand)]
        action: MarketplaceSubcommand,
    },
}

/// Skills subcommands.
#[derive(Debug, Subcommand, Clone, PartialEq, Eq)]
pub enum SkillsSubcommand {
    /// List all installed financial skills.
    List,
    /// Display details for a financial skill.
    Show {
        /// Skill name (e.g. quant, analyst, trader).
        name: String,
    },
}

/// MCP subcommands.
#[derive(Debug, Subcommand, Clone, PartialEq, Eq)]
pub enum McpSubcommand {
    /// List connected MCP servers.
    List,
    /// Connect a new MCP server.
    Connect {
        /// Server name.
        name: String,
        /// Command or endpoint to connect.
        command: String,
    },
    /// Generate MCP client config for an external coding agent.
    Config {
        /// Client name (claude-code, cursor, vscode, gemini).
        client: String,
    },
}

/// Tools subcommands.
#[derive(Debug, Subcommand, Clone, PartialEq, Eq)]
pub enum ToolsSubcommand {
    /// Search the quantitative MCP gateway tool catalog.
    Search {
        /// Free-text query against tool names, categories, and taglines.
        query: String,
        /// Optional category filter.
        #[arg(long)]
        category: Option<String>,
    },
    /// Show the JSON schema for a gateway tool.
    Describe {
        /// Tool name (e.g. tantra_garch_volatility).
        name: String,
    },
    /// Call a gateway tool and render its result envelope.
    Call {
        /// Tool name.
        name: String,
        /// Tool arguments as a JSON object.
        arguments: String,
    },
}

/// Strategy subcommands.
#[derive(Debug, Subcommand, Clone, PartialEq, Eq)]
pub enum StrategySubcommand {
    /// List all discovered built-in and manifest strategies.
    List,
    /// Run a strategy backtest.
    Run {
        /// Strategy name or id.
        name: String,
        /// Target instrument symbol.
        #[arg(short, long, default_value = "NSE:NIFTY50-INDEX")]
        symbol: String,
        /// Parameter overrides as JSON (e.g. '{"fast_period":12}').
        #[arg(long)]
        params: Option<String>,
        /// Allocated capital.
        #[arg(long, default_value = "1000000")]
        capital: rust_decimal::Decimal,
        /// Output format.
        #[arg(long, value_enum, default_value = "text")]
        format: OutputFormat,
    },
    /// Tune strategy parameters using grid search with overfitting guardrails.
    Tune {
        /// Strategy name or id.
        name: String,
        /// Target instrument symbol.
        #[arg(short, long, default_value = "NSE:NIFTY50-INDEX")]
        symbol: String,
        /// Target optimization metric (sharpe or pnl).
        #[arg(long, default_value = "sharpe")]
        metric: String,
        /// Allocated capital.
        #[arg(long, default_value = "1000000")]
        capital: rust_decimal::Decimal,
        /// Output format.
        #[arg(long, value_enum, default_value = "text")]
        format: OutputFormat,
    },
    /// Tune strategy parameters and generate an optimal trade recommendation.
    Suggest {
        /// Strategy name or id.
        name: String,
        /// Target instrument symbol.
        #[arg(short, long, default_value = "NSE:NIFTY50-INDEX")]
        symbol: String,
        /// Account capital.
        #[arg(long, default_value = "1000000")]
        capital: rust_decimal::Decimal,
        /// Percentage of account to risk (e.g. 1.0 for 1%).
        #[arg(long, default_value = "1.0")]
        risk_pct: rust_decimal::Decimal,
        /// Output format.
        #[arg(long, value_enum, default_value = "text")]
        format: OutputFormat,
    },
}

impl From<Cli> for CliArgs {
    fn from(cli: Cli) -> Self {
        let log_level = resolve_log_level(cli.log_level, cli.verbose, cli.quiet);
        let message = cli.message.clone();

        let command = match cli.command {
            None | Some(CliCommand::Tui) => Command::Tui {
                message: message.clone(),
            },
            Some(CliCommand::Ask {
                prompt,
                timeout_min,
                persona,
                mode,
                profile,
                format,
            }) => Command::Ask(AskArgs {
                prompt,
                timeout_min,
                persona,
                mode,
                profile,
                format,
            }),
            Some(CliCommand::Chat { system_prompt }) => Command::Chat(ChatArgs { system_prompt }),
            Some(CliCommand::Scan {
                query,
                server,
                universe,
                tf,
                as_of,
            }) => Command::Scan(ScanArgs {
                query,
                server,
                universe,
                tf,
                as_of,
            }),
            Some(CliCommand::Serve {
                data_dir,
                addr,
                cors_origin,
                profiles_dir,
                portfolios_dir,
                journals_dir,
                tokens_file,
            }) => Command::Serve(ServeArgs {
                data_dir,
                addr,
                cors_origin,
                profiles_dir,
                portfolios_dir,
                journals_dir,
                tokens_file,
            }),
            Some(CliCommand::Jobs { subcommand }) => Command::Jobs(match subcommand {
                None | Some(JobsSubcommand::List) => JobsArgs::List,
                Some(JobsSubcommand::Add {
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
                }) => JobsArgs::Add {
                    goal: goal.join(" "),
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
                },
                Some(JobsSubcommand::Run { id }) => JobsArgs::Run { id },
                Some(JobsSubcommand::Tick { all }) => JobsArgs::Tick { all },
                Some(JobsSubcommand::Install) => JobsArgs::Install,
                Some(JobsSubcommand::Logs { id }) => JobsArgs::Logs { id },
                Some(JobsSubcommand::Pause { id }) => JobsArgs::Pause { id },
                Some(JobsSubcommand::Resume { id }) => JobsArgs::Resume { id },
                Some(JobsSubcommand::Approvals) => JobsArgs::Approvals,
                Some(JobsSubcommand::Approve { id }) => JobsArgs::Approve { id },
                Some(JobsSubcommand::Deny { id }) => JobsArgs::Deny { id },
            }),
            Some(CliCommand::Verify { file, strict }) => {
                Command::Verify(VerifyArgs { file, strict })
            }
            Some(CliCommand::Memory { subcommand }) => Command::Memory(match subcommand {
                None | Some(MemorySubcommand::Stats) => MemoryArgs::Stats,
                Some(MemorySubcommand::Tick) => MemoryArgs::Tick,
                Some(MemorySubcommand::Model { name }) => MemoryArgs::Model { name },
                Some(MemorySubcommand::Import { path }) => MemoryArgs::Import { path },
                Some(MemorySubcommand::Install) => MemoryArgs::Install,
            }),
            Some(CliCommand::Plugin { subcommand }) => Command::Plugin(match subcommand {
                None | Some(PluginSubcommand::List) => PluginArgs::List,
                Some(PluginSubcommand::Install { name }) => PluginArgs::Install { name },
                Some(PluginSubcommand::Uninstall { name }) => PluginArgs::Uninstall { name },
                Some(PluginSubcommand::Enable { name }) => PluginArgs::Enable { name },
                Some(PluginSubcommand::Disable { name }) => PluginArgs::Disable { name },
                Some(PluginSubcommand::Validate { path }) => PluginArgs::Validate { path },
                Some(PluginSubcommand::Marketplace { action }) => {
                    PluginArgs::Marketplace(match action {
                        MarketplaceSubcommand::Add { source } => MarketplaceArgs::Add { source },
                        MarketplaceSubcommand::List => MarketplaceArgs::List,
                        MarketplaceSubcommand::Remove { name } => MarketplaceArgs::Remove { name },
                        MarketplaceSubcommand::Update { name } => MarketplaceArgs::Update { name },
                    })
                }
            }),
            Some(CliCommand::Skills { subcommand }) => Command::Skills(match subcommand {
                None | Some(SkillsSubcommand::List) => SkillsArgs::List,
                Some(SkillsSubcommand::Show { name }) => SkillsArgs::Show { name },
            }),
            Some(CliCommand::Mcp { subcommand }) => Command::Mcp(match subcommand {
                None | Some(McpSubcommand::List) => McpArgs::List,
                Some(McpSubcommand::Connect { name, command }) => {
                    McpArgs::Connect { name, command }
                }
                Some(McpSubcommand::Config { client }) => McpArgs::Config { client },
            }),
            Some(CliCommand::Tools { subcommand }) => Command::Tools(match subcommand {
                None => ToolsArgs::Search {
                    query: String::new(),
                    category: None,
                },
                Some(ToolsSubcommand::Search { query, category }) => {
                    ToolsArgs::Search { query, category }
                }
                Some(ToolsSubcommand::Describe { name }) => ToolsArgs::Describe { name },
                Some(ToolsSubcommand::Call { name, arguments }) => {
                    ToolsArgs::Call { name, arguments }
                }
            }),
            Some(CliCommand::Doctor { fix, json }) => Command::Doctor(DoctorArgs { fix, json }),
            Some(CliCommand::Purge { dry_run, yes }) => Command::Purge(PurgeArgs { dry_run, yes }),
            Some(CliCommand::Health) => Command::Health,
            Some(CliCommand::Strategy { subcommand }) => Command::Strategy(match subcommand {
                None | Some(StrategySubcommand::List) => StrategyArgs::List,
                Some(StrategySubcommand::Run {
                    name,
                    symbol,
                    params,
                    capital,
                    format,
                }) => StrategyArgs::Run(StrategyRunArgs {
                    name,
                    symbol,
                    params,
                    capital,
                    format,
                }),
                Some(StrategySubcommand::Tune {
                    name,
                    symbol,
                    metric,
                    capital,
                    format,
                }) => StrategyArgs::Tune(StrategyTuneArgs {
                    name,
                    symbol,
                    metric,
                    capital,
                    format,
                }),
                Some(StrategySubcommand::Suggest {
                    name,
                    symbol,
                    capital,
                    risk_pct,
                    format,
                }) => StrategyArgs::Suggest(StrategySuggestArgs {
                    name,
                    symbol,
                    capital,
                    risk_pct,
                    format,
                }),
            }),
            Some(CliCommand::Paper {
                mode,
                cash,
                seed,
                server,
            }) => Command::Paper(PaperArgs {
                mode,
                cash,
                seed,
                server,
            }),
            Some(CliCommand::Elephant(args)) => Command::Elephant(args),
            Some(CliCommand::StudyLoop(args)) => Command::StudyLoop(args),
        };

        Self {
            command,
            model: cli.model,
            config: cli.config,
            temperature: cli.temperature,
            log_level,
            verbose: cli.verbose,
            quiet: cli.quiet,
            skip_permissions: cli.skip_permissions,
            r#continue: cli.r#continue,
            resume: cli.resume,
            message,
            server: cli.server,
            token_file: cli.token_file,
        }
    }
}
