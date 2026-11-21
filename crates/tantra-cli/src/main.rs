//! The `tantra` binary entrypoint.

use std::io::IsTerminal;
use std::process::ExitCode;

use tantra_cli::args::parse;
use tantra_cli::jobs_runtime::boot_recover;
use tantra_cli::runner::{CliError, CommandOutcome, resolve_catalog, run, tantra_home_dir};
use tantra_tui::{AgentConnection, TuiApp};

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

                    let (conn, label) = if let Some(url) = cli.server.clone() {
                        let token = match cli.token_file.as_deref() {
                            Some(path) => match std::fs::read_to_string(path) {
                                Ok(t) => Some(t.trim().to_string()),
                                Err(e) => {
                                    eprintln!("tantra: cannot read --token-file: {}", e.kind());
                                    return ExitCode::from(2);
                                }
                            },
                            None => None,
                        };
                        let label = url.clone();
                        (AgentConnection::Remote { url, token }, label)
                    } else {
                        let catalog = match resolve_catalog(
                            cli.config.as_deref(),
                            cli.model.as_deref(),
                        ) {
                            Ok(Some(c)) => c,
                            Ok(None) => {
                                eprintln!(
                                    "tantra: no LLM configured — create tantra.jsonc (see tantra-agent/docs/design/provider-config.md) or pass --config <path>"
                                );
                                return ExitCode::from(2);
                            }
                            Err(e) => {
                                eprintln!("tantra: {e}");
                                return ExitCode::from(2);
                            }
                        };
                        let label = cli.model.clone().unwrap_or_else(|| catalog.default_model());
                        (
                            AgentConnection::Embedded {
                                catalog,
                                model: cli.model.clone(),
                            },
                            label,
                        )
                    };

                    let mut app = TuiApp::new().with_model_name(label);
                    if cli.skip_permissions {
                        app.auto_approve = true;
                    }
                    if let Some(ref msg) = cli.message {
                        app.submit_prompt(msg.clone());
                    }

                    let rt = tokio::runtime::Runtime::new().expect("failed to start tokio runtime");

                    if let Err(e) = rt.block_on(tantra_tui::run_terminal(app, conn)) {
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
