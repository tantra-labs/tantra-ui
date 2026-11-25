//! Unit tests for the Tantra CLI implementing the Quantcept command hierarchy.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use rust_decimal::Decimal;
use tantra_agent_llm::ScriptedFakeLlm;
use tantra_cli::args::{
    AskArgs, ChatArgs, Cli, CliArgs, CliCommand, Command, DoctorArgs, JobsArgs, LogLevel,
    MarketplaceArgs, McpArgs, MemoryArgs, OutputFormat, PluginArgs, PurgeArgs, ScanArgs, ServeArgs,
    SkillsArgs, VerifyArgs, resolve_log_level,
};
use tantra_cli::runner::{
    CliError, CommandOutcome, run, run_doctor_with_home, run_health_with_home, run_jobs_with_home,
    run_mcp_with_home, run_memory_with_home, run_plugin_with_home, run_purge_with_home,
    run_skills_with_home, run_with_provider,
};
use tantra_cli::scan::{parse_query, scan_universe};
use tantra_core::screener::{FilterOp, LogicalOp, PredicateValue, ScreenerScanRequest};

#[test]
fn parse_cli_roundtrip_tui() {
    let cli = Cli {
        command: Some(CliCommand::Tui),
        ..Default::default()
    };
    let args: CliArgs = cli.into();
    assert_eq!(args.command, Command::Tui { message: None });
    assert_eq!(args.model, None);
    assert_eq!(args.config, None);
}

#[test]
fn parse_cli_with_initial_message() {
    let cli = Cli {
        message: Some("What is NIFTY?".into()),
        ..Default::default()
    };
    let args: CliArgs = cli.into();
    assert_eq!(
        args.command,
        Command::Tui {
            message: Some("What is NIFTY?".into())
        }
    );
}

#[test]
fn parse_cli_roundtrip_ask() {
    let cli = Cli {
        command: Some(CliCommand::Ask {
            prompt: "what is NIFTY?".into(),
            timeout_min: 20,
            persona: Some("quant".into()),
            mode: "paper".into(),
            profile: "default".into(),
            format: OutputFormat::Json,
        }),
        ..Default::default()
    };
    let args: CliArgs = cli.into();
    assert_eq!(
        args.command,
        Command::Ask(AskArgs {
            prompt: "what is NIFTY?".into(),
            timeout_min: 20,
            persona: Some("quant".into()),
            mode: "paper".into(),
            profile: "default".into(),
            format: OutputFormat::Json,
        })
    );
}

#[test]
fn test_log_level_precedence() {
    // 1. Quiet flag sets Silent
    assert_eq!(
        resolve_log_level(Some(LogLevel::Debug), false, true),
        LogLevel::Silent
    );
    // 2. Verbose flag sets Debug
    assert_eq!(
        resolve_log_level(Some(LogLevel::Info), true, false),
        LogLevel::Debug
    );
    // 3. Explicit flag
    assert_eq!(
        resolve_log_level(Some(LogLevel::Warn), false, false),
        LogLevel::Warn
    );
    // 4. Default is Info
    assert_eq!(resolve_log_level(None, false, false), LogLevel::Info);
}

#[test]
fn parse_cli_roundtrip_chat() {
    let cli = Cli {
        command: Some(CliCommand::Chat {
            system_prompt: Some("trade NSE only".into()),
        }),
        ..Default::default()
    };
    let args: CliArgs = cli.into();
    assert_eq!(
        args.command,
        Command::Chat(ChatArgs {
            system_prompt: Some("trade NSE only".into()),
        })
    );
}

#[test]
fn parse_cli_roundtrip_scan() {
    let cli = Cli {
        command: Some(CliCommand::Scan {
            query: "roe>15".into(),
            server: Some("http://127.0.0.1:8000".into()),
            universe: Some(r#"["TCS.NSE","INFY.NSE"]"#.into()),
            tf: Some("1d".into()),
            as_of: Some("2024-01-05".into()),
        }),
        ..Default::default()
    };
    let args: CliArgs = cli.into();
    assert_eq!(
        args.command,
        Command::Scan(ScanArgs {
            query: "roe>15".into(),
            server: Some("http://127.0.0.1:8000".into()),
            universe: Some(r#"["TCS.NSE","INFY.NSE"]"#.into()),
            tf: Some("1d".into()),
            as_of: Some("2024-01-05".into()),
        })
    );
}

#[test]
fn parse_cli_roundtrip_serve() {
    let cli = Cli {
        command: Some(CliCommand::Serve {
            data_dir: PathBuf::from("data/catalog"),
            addr: "127.0.0.1:9000".parse().unwrap(),
            cors_origin: vec!["http://localhost:3000".to_string()],
            profiles_dir: Some(PathBuf::from("data/profiles")),
            portfolios_dir: None,
            journals_dir: None,
            tokens_file: None,
            allow_insecure_bind: true,
        }),
        ..Default::default()
    };
    let args: CliArgs = cli.into();
    assert_eq!(
        args.command,
        Command::Serve(ServeArgs {
            data_dir: PathBuf::from("data/catalog"),
            addr: "127.0.0.1:9000".parse().unwrap(),
            cors_origin: vec!["http://localhost:3000".to_string()],
            profiles_dir: Some(PathBuf::from("data/profiles")),
            portfolios_dir: None,
            journals_dir: None,
            tokens_file: None,
            allow_insecure_bind: true,
        })
    );
}

#[test]
fn parse_cli_roundtrip_doctor_and_purge() {
    let doc_cli = Cli {
        command: Some(CliCommand::Doctor {
            fix: true,
            json: true,
        }),
        ..Default::default()
    };
    let doc_args: CliArgs = doc_cli.into();
    assert_eq!(
        doc_args.command,
        Command::Doctor(DoctorArgs {
            fix: true,
            json: true
        })
    );

    let purge_cli = Cli {
        command: Some(CliCommand::Purge {
            dry_run: true,
            yes: false,
        }),
        ..Default::default()
    };
    let purge_args: CliArgs = purge_cli.into();
    assert_eq!(
        purge_args.command,
        Command::Purge(PurgeArgs {
            dry_run: true,
            yes: false
        })
    );
}

#[test]
fn run_tui_reports_launch() {
    let cli = CliArgs {
        command: Command::Tui { message: None },
        ..Default::default()
    };
    assert_eq!(run(&cli).unwrap(), CommandOutcome::LaunchedTui);
}

#[test]
fn run_ask_fails_with_not_wired_without_provider() {
    let cli = CliArgs {
        command: Command::Ask(AskArgs::new("hello")),
        ..Default::default()
    };
    assert_eq!(
        run_with_provider(&cli, None),
        Err(CliError::NotWired(
            "no LLM configured — add a provider to tantra.jsonc (see tantra-agent/docs/design/provider-config.md) or pass --config"
                .into()
        ))
    );
}

#[test]
fn run_ask_succeeds_with_injected_provider() {
    let fake = ScriptedFakeLlm::new("test-fake-llm");
    fake.push_text_response("NIFTY 50 is the benchmark index of the NSE.");

    let cli = CliArgs {
        command: Command::Ask(AskArgs::new("what is NIFTY?")),
        ..Default::default()
    };

    let outcome = run_with_provider(&cli, Some(Arc::new(fake))).unwrap();
    match outcome {
        CommandOutcome::Answer(ans) => {
            assert!(ans.contains("NIFTY 50 is the benchmark"));
        }
        other => panic!("expected Answer, got {other:?}"),
    }
}

#[test]
fn run_ask_json_format() {
    let fake = ScriptedFakeLlm::new("test-fake-llm");
    fake.push_text_response("Target return achieved.");

    let mut ask_args = AskArgs::new("calculate Sharpe");
    ask_args.format = OutputFormat::Json;
    ask_args.persona = Some("quant".into());

    let cli = CliArgs {
        command: Command::Ask(ask_args),
        ..Default::default()
    };

    let outcome = run_with_provider(&cli, Some(Arc::new(fake))).unwrap();
    match outcome {
        CommandOutcome::Answer(ans) => {
            assert!(ans.contains("Target return achieved."));
        }
        other => panic!("expected Answer, got {other:?}"),
    }
}

#[test]
fn run_scan_returns_matching_rows() {
    // Fixture: RELIANCE pe=25 roe=15, TCS pe=30 roe=20, INFY pe=28 roe=18,
    // HDFCBANK pe=22 roe=12. Only HDFCBANK (pe=22) satisfies pe<25.
    let cli = CliArgs {
        command: Command::Scan(ScanArgs {
            query: "pe<25".into(),
            ..Default::default()
        }),
        ..Default::default()
    };
    match run(&cli).unwrap() {
        CommandOutcome::ScanResults { rows } => {
            assert_eq!(rows, 1, "expected only HDFCBANK under pe<25");
        }
        other => panic!("expected ScanResults, got {other:?}"),
    }
}

#[test]
fn run_scan_returns_zero_rows_when_no_match() {
    let cli = CliArgs {
        command: Command::Scan(ScanArgs {
            query: "pe<5".into(),
            ..Default::default()
        }),
        ..Default::default()
    };
    match run(&cli).unwrap() {
        CommandOutcome::ScanResults { rows } => {
            assert_eq!(rows, 0);
        }
        other => panic!("expected ScanResults, got {other:?}"),
    }
}

#[test]
fn run_scan_compound_and_query() {
    // roe>15 and pe<30: INFY (roe=18, pe=28) only -> 1 row.
    let cli = CliArgs {
        command: Command::Scan(ScanArgs {
            query: "roe>15 and pe<30".into(),
            ..Default::default()
        }),
        ..Default::default()
    };
    match run(&cli).unwrap() {
        CommandOutcome::ScanResults { rows } => {
            assert_eq!(rows, 1);
        }
        other => panic!("expected ScanResults, got {other:?}"),
    }
}

#[test]
fn run_scan_rejects_empty_query() {
    let cli = CliArgs {
        command: Command::Scan(ScanArgs {
            query: "".into(),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert_eq!(
        run(&cli),
        Err(CliError::Execution("query cannot be empty".to_string()))
    );
}

#[test]
fn scan_parse_single_predicate() {
    let req: ScreenerScanRequest = parse_query("pe<25").unwrap();
    let preds = req.filters.predicates();
    assert_eq!(preds.len(), 1);
    let p = preds[0];
    assert_eq!(p.metric.0.key, "pe");
    assert!(matches!(p.op, FilterOp::Lt));
    assert_eq!(p.value, PredicateValue::Decimal(Decimal::from(25)));
    assert_eq!(p.between_bounds(), None);
}

#[test]
fn scan_parse_between() {
    let req: ScreenerScanRequest = parse_query("pe between 10 30").unwrap();
    let preds = req.filters.predicates();
    let p = preds[0];
    assert_eq!(p.metric.0.key, "pe");
    assert!(matches!(p.op, FilterOp::Between));
    assert_eq!(
        p.between_bounds(),
        Some((
            &PredicateValue::Decimal(Decimal::from(10)),
            &PredicateValue::Decimal(Decimal::from(30))
        ))
    );
}

#[test]
fn scan_parse_compound_and() {
    let req: ScreenerScanRequest = parse_query("roe>15 and pe<30").unwrap();
    let preds = req.filters.predicates();
    assert_eq!(preds.len(), 2);
    assert_eq!(req.filters.operator, LogicalOp::And);
    assert!(matches!(preds[0].op, FilterOp::Gt));
    assert!(matches!(preds[1].op, FilterOp::Lt));
}

#[test]
fn scan_apply_matching_rows() {
    let req = parse_query("pe<25").unwrap();
    let resp = tantra_ui_api_wasm::screener::apply_scan(&scan_universe(), &req).unwrap();
    assert_eq!(resp.rows.len(), 1);
    assert_eq!(resp.rows[0].instrument.symbol(), "HDFCBANK");
    assert_eq!(
        resp.rows[0].instrument.exchange(),
        tantra_core::types::Exchange::Nse
    );
}

#[test]
fn scan_apply_no_match() {
    let req = parse_query("pe<5").unwrap();
    let resp = tantra_ui_api_wasm::screener::apply_scan(&scan_universe(), &req).unwrap();
    assert_eq!(resp.rows.len(), 0);
    assert_eq!(resp.total_matched, 0);
}

#[test]
fn run_health_reports_environment() {
    let cli = CliArgs {
        command: Command::Health,
        ..Default::default()
    };
    let outcome = run(&cli).unwrap();
    match outcome {
        CommandOutcome::HealthReport(report) => {
            assert!(report.contains("tantra"));
            assert!(report.contains("tui"));
            assert!(report.contains("scan"));
        }
        other => panic!("expected health report, got {other:?}"),
    }
}

#[test]
fn run_doctor_reports_diagnostics() {
    // A fresh home dir has no lakehouse/cache: the text report must say so
    // honestly ([MISSING]) instead of canned "healthy" prose.
    let temp = tempfile::tempdir().unwrap();
    let outcome = run_doctor_with_home(
        temp.path(),
        &DoctorArgs {
            fix: false,
            json: false,
        },
    )
    .unwrap();
    match outcome {
        CommandOutcome::DoctorReport(report) => {
            assert!(report.contains("Tantra system diagnostic"), "{report}");
            assert!(report.contains("[OK]"), "home_dir exists: {report}");
            assert!(
                report.contains("[MISSING]"),
                "lakehouse/cache absent: {report}"
            );
        }
        other => panic!("expected doctor report, got {other:?}"),
    }
}

#[test]
fn run_purge_dry_run_lists_targets() {
    let cli = CliArgs {
        command: Command::Purge(PurgeArgs {
            dry_run: true,
            yes: false,
        }),
        ..Default::default()
    };
    let outcome = run(&cli).unwrap();
    match outcome {
        CommandOutcome::PurgeSuccess(msg) => {
            assert!(msg.contains("dry-run preview"));
        }
        other => panic!("expected purge preview, got {other:?}"),
    }
}

#[test]
fn run_purge_without_yes_refuses() {
    let cli = CliArgs {
        command: Command::Purge(PurgeArgs {
            dry_run: false,
            yes: false,
        }),
        ..Default::default()
    };
    assert!(run(&cli).is_err());
}

#[test]
fn run_jobs_lifecycle() {
    let temp = tempfile::tempdir().unwrap();
    let add_args = JobsArgs::Add {
        goal: "Scan momentum break in Nifty midcaps".into(),
        schedule: Some("0 9 * * 1-5".into()),
        max_turns: 20,
        max_inr: None,
        max_usd: None,
        max_tokens: Some(4000),
        max_tool_calls: Some(10),
        max_data_calls: Some(5),
        missed_policy: tantra_cli::args::MissedPolicy::Skip,
        approval: tantra_cli::args::ApprovalPolicyArg::Auto,
        strategy: Some(String::from("sma_crossover")),
        symbol: Some(String::from("NSE:RELIANCE-EQ")),
        capital: Some(rust_decimal::Decimal::new(1_000_000, 0)),
        params: Some(String::from("{\"fast\":10}")),
        success: None,
        read_only: true,
        once: false,
    };
    let outcome = run_jobs_with_home(&add_args, temp.path()).unwrap();
    match outcome {
        CommandOutcome::JobsSuccess(summary) => {
            assert!(summary.contains("Created job"));
            assert!(summary.contains("0 9 * * 1-5"));
            assert!(summary.contains("sma_crossover"));
            assert!(summary.contains("NSE:RELIANCE-EQ"));
        }
        other => panic!("expected JobsSuccess, got {other:?}"),
    }

    let outcome = run_jobs_with_home(&JobsArgs::List, temp.path()).unwrap();
    match outcome {
        CommandOutcome::JobsSuccess(list) => {
            assert!(list.contains("STATUS") || list.contains("registered"));
            assert!(
                list.contains("Scan momentum break in Nifty midcaps"),
                "job list must show the goal: {list}"
            );
        }
        other => panic!("expected JobsSuccess, got {other:?}"),
    }

    let outcome = run_jobs_with_home(&JobsArgs::Install, temp.path()).unwrap();
    match outcome {
        CommandOutcome::JobsSuccess(snippet) => {
            assert!(snippet.contains("cron scheduler setup"));
        }
        other => panic!("expected JobsSuccess, got {other:?}"),
    }
}

#[test]
fn run_verify_detects_lookahead_bias() {
    let tmp_dir = std::env::temp_dir();
    let bad_strategy = tmp_dir.join("test_bad_strategy.py");
    fs::write(
        &bad_strategy,
        "import pandas as pd\n\
         df['target'] = df['close'].shift(-1)\n\
         print('bad lookahead')\n",
    )
    .unwrap();

    let cli = CliArgs {
        command: Command::Verify(VerifyArgs {
            file: Some(bad_strategy.clone()),
            strict: false,
        }),
        ..Default::default()
    };

    let err = run(&cli).unwrap_err();
    match err {
        CliError::VerificationFailed(msg) => {
            assert!(msg.contains("Lookahead bias detected"));
            assert!(msg.contains("Negative shift access"));
        }
        other => panic!("expected VerificationFailed, got {other:?}"),
    }

    let _ = fs::remove_file(bad_strategy);
}

#[test]
fn run_verify_passes_clean_strategy() {
    let tmp_dir = std::env::temp_dir();
    let good_strategy = tmp_dir.join("test_good_strategy.py");
    fs::write(
        &good_strategy,
        "import pandas as pd\n\
         df['sma20'] = df['close'].rolling(20).mean()\n\
         signal = df['close'] > df['sma20']\n",
    )
    .unwrap();

    let cli = CliArgs {
        command: Command::Verify(VerifyArgs {
            file: Some(good_strategy.clone()),
            strict: true,
        }),
        ..Default::default()
    };

    let outcome = run(&cli).unwrap();
    match outcome {
        CommandOutcome::VerifySuccess { passed, message } => {
            assert!(passed);
            assert!(message.contains("0 lookahead errors"));
        }
        other => panic!("expected VerifySuccess, got {other:?}"),
    }

    let _ = fs::remove_file(good_strategy);
}

#[test]
fn run_verify_missing_file_returns_file_error() {
    let cli = CliArgs {
        command: Command::Verify(VerifyArgs {
            file: Some(PathBuf::from("/nonexistent/file/path.py")),
            strict: false,
        }),
        ..Default::default()
    };

    let err = run(&cli).unwrap_err();
    match err {
        CliError::MissingFile(msg) => {
            assert!(msg.contains("does not exist"));
        }
        other => panic!("expected MissingFile, got {other:?}"),
    }
}

#[test]
fn run_memory_and_plugin_commands() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();

    let mem = run_memory_with_home(&MemoryArgs::Stats, home).unwrap();
    match mem {
        CommandOutcome::MemorySuccess(stats) => {
            assert!(stats.contains("Local Semantic Memory"));
            assert!(stats.contains("0 notes, 0 active semantic facts"));
        }
        other => panic!("expected MemorySuccess, got {other:?}"),
    }

    let plug = run_plugin_with_home(&PluginArgs::List, home).unwrap();
    match plug {
        CommandOutcome::PluginSuccess(list) => {
            assert!(list.contains("Installed plugins"));
            assert!(list.contains("(builtin)"));
        }
        other => panic!("expected PluginSuccess, got {other:?}"),
    }

    let market =
        run_plugin_with_home(&PluginArgs::Marketplace(MarketplaceArgs::List), home).unwrap();
    match market {
        CommandOutcome::PluginSuccess(list) => {
            assert!(list.contains("No marketplace sources configured"), "{list}");
        }
        other => panic!("expected Marketplace, got {other:?}"),
    }
}

#[test]
fn run_skills_and_mcp_commands() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();

    // A fresh home has no skills: the list must say so honestly rather than
    // printing a canned 8-persona catalog.
    let skills = run_skills_with_home(&SkillsArgs::List, home).unwrap();
    match skills {
        CommandOutcome::SkillsSuccess(msg) => {
            assert!(msg.contains("No skills installed under"), "{msg}");
        }
        other => panic!("expected SkillsSuccess, got {other:?}"),
    }

    // The builtin stdio server is always listed; disk rows are absent.
    let mcp = run_mcp_with_home(&McpArgs::List, home).unwrap();
    match mcp {
        CommandOutcome::McpSuccess(msg) => {
            assert!(msg.contains("tantra"), "builtin stdio server: {msg}");
        }
        other => panic!("expected McpSuccess, got {other:?}"),
    }
}

#[test]
fn run_ask_empty_prompt_errors() {
    let cli = CliArgs {
        command: Command::Ask(AskArgs::new("  ")),
        ..Default::default()
    };
    assert!(run(&cli).is_err());
}

#[test]
fn test_parse_backtest_query() {
    use tantra_cli::backtest_gen::parse_backtest_query;
    let spec =
        parse_backtest_query("backtest 10L on Alpha30 stocks ; with 15 day rebalancing").unwrap();
    assert_eq!(spec.capital, rust_decimal::Decimal::from(1_000_000));
    assert_eq!(spec.universe, "Alpha30");
    assert_eq!(spec.rebalance_days, 15);
}

#[test]
fn test_run_ask_backtest_synthesis() {
    let cli = CliArgs {
        command: Command::Ask(AskArgs::new(
            "backtest 10L on Alpha30 stocks ; with 15 day rebalancing",
        )),
        ..Default::default()
    };
    let outcome = run(&cli).unwrap();
    match outcome {
        CommandOutcome::Answer(ans) => {
            assert!(ans.contains("Backtest Execution Report"));
            assert!(ans.contains("Alpha30"));
            assert!(ans.contains("15-Day Rebalance"));
            assert!(std::path::Path::new("backtest_alpha30.py").exists());
        }
        other => panic!("expected Answer with backtest report, got {other:?}"),
    }
}

// ── T10.1 (gap ui 0003): real on-disk command reports ────────────────────────
// Every test drives the `*_with_home` entry points against a `tempfile` home so
// the assertions reflect genuine filesystem state, not canned output.

#[test]
fn run_memory_import_then_stats_counts_real_entries() {
    let temp = tempfile::tempdir().unwrap();
    let docs = temp.path().join("notes");
    fs::create_dir_all(&docs).unwrap();
    fs::write(
        docs.join("alpha.md"),
        "# Alpha\nMomentum factor study for Nifty midcaps.",
    )
    .unwrap();

    let import =
        run_memory_with_home(&MemoryArgs::Import { path: Some(docs) }, temp.path()).unwrap();
    match import {
        CommandOutcome::MemorySuccess(msg) => {
            assert!(msg.contains("Imported 1 note(s)"), "{msg}");
        }
        other => panic!("expected MemorySuccess, got {other:?}"),
    }

    let stats = run_memory_with_home(&MemoryArgs::Stats, temp.path()).unwrap();
    match stats {
        CommandOutcome::MemorySuccess(msg) => {
            assert!(msg.contains("1 notes, 1 active semantic facts"), "{msg}");
            assert!(msg.contains("KB"), "{msg}");
        }
        other => panic!("expected MemorySuccess, got {other:?}"),
    }

    // The store is really on disk: one JSONL record exists.
    assert_eq!(
        tantra_cli::memory_store::load_entries(&temp.path().join("memory")).len(),
        1
    );
}

#[test]
fn run_memory_import_missing_path_is_file_error() {
    let temp = tempfile::tempdir().unwrap();
    let err = run_memory_with_home(
        &MemoryArgs::Import {
            path: Some(temp.path().join("nope")),
        },
        temp.path(),
    );
    assert!(matches!(err, Err(CliError::MissingFile(_))));
}

#[test]
fn run_memory_model_set_and_read_roundtrip() {
    let temp = tempfile::tempdir().unwrap();
    let set = run_memory_with_home(
        &MemoryArgs::Model {
            name: Some("nomic-embed".into()),
        },
        temp.path(),
    )
    .unwrap();
    assert!(matches!(set, CommandOutcome::MemorySuccess(_)));

    let read = run_memory_with_home(&MemoryArgs::Model { name: None }, temp.path()).unwrap();
    match read {
        CommandOutcome::MemorySuccess(msg) => assert!(msg.contains("nomic-embed"), "{msg}"),
        other => panic!("expected MemorySuccess, got {other:?}"),
    }
}

#[test]
fn run_memory_tick_distills_session_transcripts() {
    let temp = tempfile::tempdir().unwrap();
    let sessions = temp.path().join("sessions");
    fs::create_dir_all(&sessions).unwrap();
    fs::write(sessions.join("ses_abc.jsonl"), "{\"role\":\"user\"}\n").unwrap();
    fs::write(sessions.join("ses_def.jsonl"), "{\"role\":\"assistant\"}\n").unwrap();

    let out = run_memory_with_home(&MemoryArgs::Tick, temp.path()).unwrap();
    match out {
        CommandOutcome::MemorySuccess(msg) => {
            assert!(
                msg.contains("distilled 2 session transcript(s) into 2 memory note(s)"),
                "{msg}"
            );
        }
        other => panic!("expected MemorySuccess, got {other:?}"),
    }
    assert_eq!(
        tantra_cli::memory_store::load_entries(&temp.path().join("memory")).len(),
        2
    );
}

#[test]
fn run_plugin_install_list_disable_enable_validate() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();

    let install = run_plugin_with_home(
        &PluginArgs::Install {
            name: "alpha-flow".into(),
        },
        home,
    )
    .unwrap();
    assert!(matches!(install, CommandOutcome::PluginSuccess(_)));

    // Disk-installed plugins are listed as enabled, next to the builtins.
    let list = run_plugin_with_home(&PluginArgs::List, home).unwrap();
    match list {
        CommandOutcome::PluginSuccess(msg) => {
            assert!(msg.contains("alpha-flow (enabled)"), "{msg}");
            assert!(msg.contains("(builtin)"), "{msg}");
        }
        other => panic!("expected PluginSuccess, got {other:?}"),
    }

    let disable = run_plugin_with_home(
        &PluginArgs::Disable {
            name: "alpha-flow".into(),
        },
        home,
    )
    .unwrap();
    assert!(matches!(disable, CommandOutcome::PluginSuccess(_)));
    let list = run_plugin_with_home(&PluginArgs::List, home).unwrap();
    match list {
        CommandOutcome::PluginSuccess(msg) => {
            assert!(msg.contains("alpha-flow (disabled)"), "{msg}");
        }
        other => panic!("expected PluginSuccess, got {other:?}"),
    }

    let enable = run_plugin_with_home(
        &PluginArgs::Enable {
            name: "alpha-flow".into(),
        },
        home,
    )
    .unwrap();
    assert!(matches!(enable, CommandOutcome::PluginSuccess(_)));

    let validate = run_plugin_with_home(
        &PluginArgs::Validate {
            path: Some(home.join("plugins/alpha-flow")),
        },
        home,
    )
    .unwrap();
    match validate {
        CommandOutcome::PluginSuccess(msg) => {
            assert!(msg.contains("Validated plugin manifest"), "{msg}");
        }
        other => panic!("expected PluginSuccess, got {other:?}"),
    }

    // Re-installing an existing plugin and touching an absent one are honest errors.
    let dup = run_plugin_with_home(
        &PluginArgs::Install {
            name: "alpha-flow".into(),
        },
        home,
    );
    assert!(dup.is_err());
    let uninstall_missing = run_plugin_with_home(
        &PluginArgs::Uninstall {
            name: "ghost".into(),
        },
        home,
    );
    assert!(uninstall_missing.is_err());
}

#[test]
fn run_plugin_marketplace_add_list_remove_roundtrip() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();
    let src = "alpha=https://hub.tantra.dev/alpha".to_string();

    let add = run_plugin_with_home(
        &PluginArgs::Marketplace(MarketplaceArgs::Add {
            source: src.clone(),
        }),
        home,
    )
    .unwrap();
    assert!(matches!(add, CommandOutcome::PluginSuccess(_)));

    let list = run_plugin_with_home(&PluginArgs::Marketplace(MarketplaceArgs::List), home).unwrap();
    match list {
        CommandOutcome::PluginSuccess(msg) => {
            assert!(
                msg.contains("alpha") && msg.contains("hub.tantra.dev"),
                "{msg}"
            );
        }
        other => panic!("expected PluginSuccess, got {other:?}"),
    }

    let dup = run_plugin_with_home(
        &PluginArgs::Marketplace(MarketplaceArgs::Add {
            source: src.clone(),
        }),
        home,
    );
    assert!(dup.is_err(), "duplicate source must error");

    let remove = run_plugin_with_home(
        &PluginArgs::Marketplace(MarketplaceArgs::Remove {
            name: "alpha".into(),
        }),
        home,
    )
    .unwrap();
    assert!(matches!(remove, CommandOutcome::PluginSuccess(_)));

    let gone = run_plugin_with_home(
        &PluginArgs::Marketplace(MarketplaceArgs::Remove {
            name: "alpha".into(),
        }),
        home,
    );
    assert!(gone.is_err(), "removing an absent source must error");
}

#[test]
fn run_skills_empty_home_prints_honest_message() {
    let temp = tempfile::tempdir().unwrap();
    let out = run_skills_with_home(&SkillsArgs::List, temp.path()).unwrap();
    match out {
        CommandOutcome::SkillsSuccess(msg) => {
            assert!(msg.contains("No skills installed under"), "{msg}");
        }
        other => panic!("expected SkillsSuccess, got {other:?}"),
    }

    let missing = run_skills_with_home(
        &SkillsArgs::Show {
            name: "quant".into(),
        },
        temp.path(),
    );
    assert!(matches!(missing, Err(CliError::MissingFile(_))));
}

#[test]
fn run_skills_show_reads_real_skill_file() {
    let temp = tempfile::tempdir().unwrap();
    let skill_dir = temp.path().join("skills").join("quant");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        "# Quant Desk\n\nLine two.\nLine three.\n",
    )
    .unwrap();

    let show = run_skills_with_home(
        &SkillsArgs::Show {
            name: "quant".into(),
        },
        temp.path(),
    )
    .unwrap();
    match show {
        CommandOutcome::SkillsSuccess(msg) => assert!(msg.contains("# Quant Desk"), "{msg}"),
        other => panic!("expected SkillsSuccess, got {other:?}"),
    }

    let list = run_skills_with_home(&SkillsArgs::List, temp.path()).unwrap();
    match list {
        CommandOutcome::SkillsSuccess(msg) => assert!(msg.contains("• quant"), "{msg}"),
        other => panic!("expected SkillsSuccess, got {other:?}"),
    }
}

#[test]
fn run_mcp_connect_then_list_roundtrip() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();

    let conn = run_mcp_with_home(
        &McpArgs::Connect {
            name: "context7".into(),
            command: "npx @upstash/context7-mcp".into(),
        },
        home,
    )
    .unwrap();
    assert!(matches!(conn, CommandOutcome::McpSuccess(_)));

    let list = run_mcp_with_home(&McpArgs::List, home).unwrap();
    match list {
        CommandOutcome::McpSuccess(msg) => {
            assert!(msg.contains("tantra"), "builtin stdio server: {msg}");
            assert!(msg.contains("context7"), "{msg}");
        }
        other => panic!("expected McpSuccess, got {other:?}"),
    }

    let dup = run_mcp_with_home(
        &McpArgs::Connect {
            name: "context7".into(),
            command: "npx @upstash/context7-mcp".into(),
        },
        home,
    );
    assert!(dup.is_err(), "duplicate MCP connect must error");
}

#[test]
fn run_purge_dry_run_lists_only_existing_paths() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();
    fs::create_dir_all(home.join("cache")).unwrap();
    fs::write(home.join("cache/tick.parquet"), b"x").unwrap();
    // home/tmp is deliberately absent.

    let preview = run_purge_with_home(
        home,
        &PurgeArgs {
            dry_run: true,
            yes: false,
        },
    )
    .unwrap();
    match preview {
        CommandOutcome::PurgeSuccess(msg) => {
            assert!(msg.contains("Would remove"), "{msg}");
            assert!(
                msg.contains(&home.join("cache").display().to_string()),
                "existing cache must be listed: {msg}"
            );
            assert!(
                !msg.contains(&home.join("tmp").display().to_string()),
                "missing paths must not be listed: {msg}"
            );
        }
        other => panic!("expected PurgeSuccess, got {other:?}"),
    }

    // Non-interactive purge without --yes/--dry-run refuses.
    let err = run_purge_with_home(
        home,
        &PurgeArgs {
            dry_run: false,
            yes: false,
        },
    );
    assert!(err.is_err());

    // With --yes, only the existing paths are really removed.
    let purge = run_purge_with_home(
        home,
        &PurgeArgs {
            dry_run: false,
            yes: true,
        },
    )
    .unwrap();
    match purge {
        CommandOutcome::PurgeSuccess(msg) => {
            assert!(msg.contains("removed") && msg.contains("cache"), "{msg}");
        }
        other => panic!("expected PurgeSuccess, got {other:?}"),
    }
    assert!(!home.join("cache").exists());
}

#[test]
fn run_doctor_json_booleans_reflect_tempdir_state() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();

    let before = run_doctor_with_home(
        home,
        &DoctorArgs {
            fix: false,
            json: true,
        },
    )
    .unwrap();
    match before {
        CommandOutcome::DoctorReport(report) => {
            let v: serde_json::Value =
                serde_json::from_str(&report).expect("doctor --json must be JSON");
            assert_eq!(
                v["home_dir"],
                serde_json::json!(home.display().to_string()),
                "{report}"
            );
            assert_eq!(v["lakehouse"], serde_json::json!(false), "{report}");
            assert_eq!(v["cache"], serde_json::json!(false), "{report}");
            assert!(v["model"].is_string() || v["model"].is_null(), "{report}");
        }
        other => panic!("expected DoctorReport, got {other:?}"),
    }

    let after = run_doctor_with_home(
        home,
        &DoctorArgs {
            fix: true,
            json: true,
        },
    )
    .unwrap();
    match after {
        CommandOutcome::DoctorReport(report) => {
            let v: serde_json::Value = serde_json::from_str(&report).unwrap();
            assert_eq!(v["lakehouse"], serde_json::json!(true), "{report}");
            assert_eq!(v["cache"], serde_json::json!(true), "{report}");
        }
        other => panic!("expected DoctorReport, got {other:?}"),
    }
}

#[test]
fn run_health_reports_real_wiring_not_pending_universe() {
    let temp = tempfile::tempdir().unwrap();
    let out = run_health_with_home(temp.path()).unwrap();
    match out {
        CommandOutcome::HealthReport(report) => {
            assert!(report.contains("scan"), "{report}");
            assert!(report.contains("serve"), "{report}");
            assert!(!report.contains("pending universe"), "{report}");
            assert!(!report.contains("SEBI pre-trade risk ONLINE"), "{report}");
        }
        other => panic!("expected HealthReport, got {other:?}"),
    }
}
