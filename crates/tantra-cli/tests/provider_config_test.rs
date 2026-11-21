//! Provider-config resolution and CLI arg parsing.

use std::path::PathBuf;

use clap::Parser;
use tantra_cli::args::{Cli, CliArgs};
use tantra_cli::runner::resolve_provider;

const CONFIG: &str = r#"{"model":"local/m1","provider":{"local":{"options":{"baseURL":"http://127.0.0.1:9/v1"},"models":{"m1":{},"m2":{"id":"upstream-two"}}}}}"#;

fn write_config(tag: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("tantra-cli-provider-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("tantra.jsonc");
    std::fs::write(&path, CONFIG).unwrap();
    path
}

#[test]
fn default_model_from_config() {
    let path = write_config("default");
    let r = resolve_provider(Some(&path), None).unwrap().unwrap();
    assert_eq!(r.id, "local/m1");
}

#[test]
fn explicit_model_selects_upstream_id() {
    let path = write_config("explicit");
    let r = resolve_provider(Some(&path), Some("local/m2"))
        .unwrap()
        .unwrap();
    assert_eq!(r.provider.model_name(), "upstream-two");
    assert_eq!(r.models.len(), 2);
}

#[test]
fn unknown_model_lists_configured() {
    let path = write_config("unknown");
    let err = resolve_provider(Some(&path), Some("nope"))
        .err()
        .unwrap()
        .to_string();
    assert!(err.contains("local/m1"), "{err}");
}

#[test]
fn missing_explicit_path_is_error() {
    let path = std::env::temp_dir().join("tantra-cli-provider-absent/tantra.jsonc");
    assert!(resolve_provider(Some(&path), None).is_err());
}

#[test]
fn model_and_config_flags_parse() {
    let cli = Cli::try_parse_from([
        "tantra",
        "--model",
        "local/m2",
        "--config",
        "/x/tantra.jsonc",
        "tui",
    ])
    .unwrap();
    let args: CliArgs = cli.into();
    assert_eq!(args.model.as_deref(), Some("local/m2"));
    assert_eq!(args.config, Some(PathBuf::from("/x/tantra.jsonc")));
}

#[test]
fn server_and_token_file_flags_parse_into_cli_args() {
    let args: CliArgs = Cli::try_parse_from([
        "tantra",
        "--server",
        "ws://h/v1/acp",
        "--token-file",
        "/tmp/t",
    ])
    .expect("parse cli")
    .into();
    assert_eq!(args.server.as_deref(), Some("ws://h/v1/acp"));
    assert_eq!(args.token_file, Some(PathBuf::from("/tmp/t")));
}
