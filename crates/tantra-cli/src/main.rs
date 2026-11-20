//! The `tantra` binary entrypoint.

use std::io::IsTerminal;
use std::process::ExitCode;
use std::sync::Arc;

use tantra_agent_llm::{CandleGgufConfig, CandleGgufProvider};
use tantra_cli::args::parse;
use tantra_cli::jobs_runtime::boot_recover;
use tantra_cli::runner::{CliError, CommandOutcome, resolve_provider, run, tantra_home_dir};
use tantra_tui::{ActiveTab, TuiApp};

fn main() -> ExitCode {
    let cli = parse();
    // Crash recovery sweep of interrupted run journals (job & tool runs):
    // Pending/Running snapshots are normalized to Failed before commands run.
    let recovered = boot_recover(Some(&tantra_home_dir()));
    if recovered > 0 && !cli.quiet {
        eprintln!("tantra: crash recovery normalized {recovered} interrupted run journal(s)");
    }
    match run(&cli) {
        Ok(outcome) => {
            match outcome {
                CommandOutcome::LaunchedTui => {
                    if !std::io::stdout().is_terminal() {
                        println!("tantra: tui would launch");
                        return ExitCode::SUCCESS;
                    }

                    let provider = resolve_provider(cli.model_path.as_deref(), cli.temperature)
                        .ok()
                        .flatten()
                        .unwrap_or_else(|| {
                            Arc::new(CandleGgufProvider::new(CandleGgufConfig::default()))
                        });

                    let mut app = TuiApp::new().with_model_name(provider.model_name());
                    if cli.skip_permissions {
                        app.auto_approve = true;
                    }
                    if let Some(ref msg) = cli.message {
                        app.active_tab = ActiveTab::AgentChat;
                        app.pending_prompt = Some(msg.clone());
                        app.is_loading = true;
                    }

                    let rt = tokio::runtime::Runtime::new().expect("failed to start tokio runtime");

                    if let Err(e) = rt.block_on(tantra_tui::run_terminal(app, provider)) {
                        eprintln!("tantra: tui error: {e}");
                        return ExitCode::from(1);
                    }
                }
                CommandOutcome::Answer(answer) => {
                    if answer.is_empty() {
                        println!("(empty response)");
                    }
                }
                CommandOutcome::ChatCompleted => {}
                CommandOutcome::ScanResults { rows } => {
                    println!("tantra: scan returned {rows} rows")
                }
                CommandOutcome::JobsSuccess(msg) => println!("{msg}"),
                CommandOutcome::VerifySuccess { message, .. } => println!("{message}"),
                CommandOutcome::MemorySuccess(msg) => println!("{msg}"),
                CommandOutcome::PluginSuccess(msg) => println!("{msg}"),
                CommandOutcome::SkillsSuccess(msg) => println!("{msg}"),
                CommandOutcome::McpSuccess(msg) => println!("{msg}"),
                CommandOutcome::ToolsSuccess(msg) => println!("{msg}"),
                CommandOutcome::DoctorReport(report) => println!("{report}"),
                CommandOutcome::PurgeSuccess(msg) => println!("{msg}"),
                CommandOutcome::HealthReport(report) => println!("{report}"),
                CommandOutcome::StrategySuccess(msg) => println!("{msg}"),
                CommandOutcome::ServeSuccess(msg) => println!("{msg}"),
                CommandOutcome::PaperSuccess(msg) => println!("{msg}"),
                CommandOutcome::ElephantReport(report) => println!("{report}"),
                CommandOutcome::StudyLoopReport(report) => println!("{report}"),
            }
            ExitCode::SUCCESS
        }
        Err(err) => match &err {
            CliError::MissingFile(msg) => {
                eprintln!("tantra: file error: {msg}");
                ExitCode::from(2)
            }
            CliError::VerificationFailed(msg) => {
                eprintln!("tantra: verification failed: {msg}");
                ExitCode::from(1)
            }
            other => {
                eprintln!("tantra: error: {other}");
                ExitCode::from(1)
            }
        },
    }
}
