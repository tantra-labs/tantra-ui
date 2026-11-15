//! On-disk memory store backing `tantra memory` (gap `0003`, T10.1).
//!
//! Entries are appended as JSONL records to `<home>/memory/entries.jsonl`; the
//! active model name lives in `<home>/config.toml`. These are pure filesystem
//! helpers so integration tests can drive the store from a tempdir. This CLI is
//! native, so `chrono::Utc` timestamps are acceptable for reporting here (the
//! zero-clock rule applies to domain engines, not CLI state).

use std::fs;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Directory that holds the JSONL memory entries.
pub fn store_dir(home: &Path) -> PathBuf {
    home.join("memory")
}

/// Read all entries from `entries.jsonl` (missing file → empty).
pub fn load_entries(dir: &Path) -> Vec<serde_json::Value> {
    let file = dir.join("entries.jsonl");
    let Ok(content) = fs::read_to_string(&file) else {
        return Vec::new();
    };
    content
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// Append one JSON entry as a JSONL record, creating the store dir if needed.
pub fn append_entry(dir: &Path, entry: &serde_json::Value) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let mut line =
        serde_json::to_string(entry).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    line.push('\n');
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("entries.jsonl"))?
        .write_all(line.as_bytes())
}

/// Total bytes of `entries.jsonl`.
pub fn store_bytes(dir: &Path) -> u64 {
    fs::metadata(dir.join("entries.jsonl"))
        .map(|m| m.len())
        .unwrap_or(0)
}

/// List session transcript files under `<home>/sessions/` (top-level files).
pub fn list_transcripts(sessions: &Path) -> Vec<PathBuf> {
    let Ok(rd) = fs::read_dir(sessions) else {
        return Vec::new();
    };
    rd.flatten()
        .filter(|e| e.path().is_file())
        .map(|e| e.path())
        .collect()
}

/// Number of session transcripts under `<home>/sessions/`.
pub fn count_transcripts(sessions: &Path) -> usize {
    list_transcripts(sessions).len()
}

/// Collect `.md`/`.txt` files to import: the path itself, or every matching
/// file under it (recursively).
pub fn collect_docs(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return if is_doc(path) {
            vec![path.to_path_buf()]
        } else {
            Vec::new()
        };
    }
    let Ok(rd) = fs::read_dir(path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            out.extend(collect_docs(&p));
        } else if is_doc(&p) {
            out.push(p);
        }
    }
    out
}

fn is_doc(p: &Path) -> bool {
    matches!(
        p.extension().and_then(|e| e.to_str()),
        Some("md") | Some("txt")
    )
}

/// Set the model name in `<home>/config.toml` under a `[memory]` table.
pub fn set_model_name(config: &Path, name: &str) -> io::Result<()> {
    if let Some(parent) = config.parent() {
        fs::create_dir_all(parent)?;
    }
    let existing = fs::read_to_string(config).unwrap_or_default();
    let mut root: toml::map::Map<String, toml::Value> =
        toml::from_str(&existing).unwrap_or_default();
    let memory = root
        .entry("memory".to_string())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    if let toml::Value::Table(m) = memory {
        m.insert("model".to_string(), toml::Value::String(name.to_string()));
    }
    let out = toml::to_string(&toml::Value::Table(root))
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(config, out)
}

/// Read the model name from `<home>/config.toml` (empty when unset).
pub fn get_model_name(config: &Path) -> String {
    let Ok(content) = fs::read_to_string(config) else {
        return String::new();
    };
    let Ok(root) = toml::from_str::<toml::Value>(&content) else {
        return String::new();
    };
    root.get("memory")
        .and_then(|m| m.get("model"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}
