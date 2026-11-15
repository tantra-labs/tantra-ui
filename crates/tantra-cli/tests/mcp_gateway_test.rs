//! MCP gateway integration tests: result-block rendering (`block_render`),
//! `tantra tools` gateway commands, and `tantra mcp config` exporter
//! (plan 0003).

use tantra_agent_core::result::{DisplayBlock, MetricItem};
use tantra_cli::args::{McpArgs, ToolsArgs};
use tantra_cli::block_render::render_blocks;
use tantra_cli::runner::{CliError, CommandOutcome, run_mcp, run_tools};

#[test]
fn render_blocks_renders_table_metric_chart_alert() {
    let blocks = vec![
        DisplayBlock::Table {
            title: "Strategy Summary".into(),
            columns: vec!["Symbol".into(), "Alpha".into()],
            rows: vec![
                vec!["RELIANCE".into(), "+1.25".into()],
                vec!["TCS".into(), "-0.50".into()],
            ],
        },
        DisplayBlock::MetricGrid {
            items: vec![MetricItem {
                label: "Sharpe".into(),
                value: "+2.10".into(),
            }],
        },
        DisplayBlock::Chart {
            title: "Equity Curve".into(),
            chart_type: "line".into(),
            series_ref: "$tantra/ref/equity-001".into(),
        },
        DisplayBlock::Alert {
            level: "INFO".into(),
            message: "Strategy started".into(),
        },
    ];
    let out = render_blocks(&blocks);
    for needle in [
        "Strategy Summary",
        "Symbol",
        "Alpha",
        "RELIANCE",
        "+1.25",
        "TCS",
        "-0.50",
        "Sharpe:",
        "+2.10",
        "Equity Curve",
        "$tantra/ref/equity-001",
        "[INFO]",
        "Strategy started",
    ] {
        assert!(
            out.contains(needle),
            "missing {needle:?} in rendered blocks:\n{out}"
        );
    }
}

#[test]
fn tools_search_lists_matching_tools() {
    let outcome = run_tools(&ToolsArgs::Search {
        query: "garch".into(),
        category: None,
    })
    .expect("search should succeed");
    match outcome {
        CommandOutcome::ToolsSuccess(msg) => {
            assert!(msg.contains("tantra_garch_volatility"), "got: {msg}");
        }
        other => panic!("expected ToolsSuccess, got {other:?}"),
    }
}

#[test]
fn tools_describe_shows_schema() {
    let outcome = run_tools(&ToolsArgs::Describe {
        name: "tantra_garch_volatility".into(),
    })
    .expect("describe should succeed");
    match outcome {
        CommandOutcome::ToolsSuccess(msg) => {
            assert!(msg.contains("tantra_garch_volatility"), "got: {msg}");
            assert!(msg.contains("volatility"), "got: {msg}");
        }
        other => panic!("expected ToolsSuccess, got {other:?}"),
    }
}

#[test]
fn tools_call_renders_envelope() {
    let outcome = run_tools(&ToolsArgs::Call {
        name: "get_indicator_catalog".into(),
        arguments: "{}".into(),
    })
    .expect("call should succeed");
    match outcome {
        CommandOutcome::ToolsSuccess(msg) => {
            assert!(msg.contains("Indicator catalog"), "got: {msg}");
        }
        other => panic!("expected ToolsSuccess, got {other:?}"),
    }
}

// Test name mandated by plan 0003; rustc's snake-case lint is allowed.
#[allow(non_snake_case)]
#[test]
fn mcp_config_cursor_emits_mcpServers() {
    let outcome = run_mcp(&McpArgs::Config {
        client: "cursor".into(),
    })
    .expect("cursor config should generate");
    match outcome {
        CommandOutcome::McpSuccess(msg) => {
            assert!(msg.contains("mcpServers"), "got: {msg}");
            assert!(msg.contains("\"tantra\""), "got: {msg}");
            assert!(msg.contains("\"mcp\""), "got: {msg}");
        }
        other => panic!("expected McpSuccess, got {other:?}"),
    }
}

#[test]
fn mcp_config_unknown_client_errors() {
    let result = run_mcp(&McpArgs::Config {
        client: "windsurf".into(),
    });
    match result {
        Err(CliError::Execution(msg)) => {
            for client in ["claude-code", "cursor", "vscode", "gemini"] {
                assert!(msg.contains(client), "missing {client} in: {msg}");
            }
        }
        Err(_) => panic!("expected CliError::Execution for unknown client"),
        Ok(_) => panic!("expected an error for unknown MCP client"),
    }
}
