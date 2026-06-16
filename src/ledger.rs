use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::plan::Classification;

#[derive(Debug, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub repo: String,
    pub bytes_freed: u64,
    pub classification: Classification,
    pub ts: String,
}

pub fn default_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;
    let hold_dir = PathBuf::from(home).join("wintermute").join(".hold");
    std::fs::create_dir_all(&hold_dir)
        .with_context(|| format!("creating {}", hold_dir.display()))?;
    Ok(hold_dir.join("migrate-ledger.jsonl"))
}

pub fn append(path: &Path, repo: &str, bytes_freed: u64, classification: &Classification, ts: &str) -> Result<()> {
    let entry = LedgerEntry {
        repo: repo.to_string(),
        bytes_freed,
        classification: classification.clone(),
        ts: ts.to_string(),
    };
    let line = serde_json::to_string(&entry).context("serializing ledger entry")?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("opening ledger at {}", path.display()))?;
    writeln!(file, "{line}").context("writing ledger entry")?;
    Ok(())
}

pub fn print_all() -> Result<()> {
    let path = default_path()?;
    if !path.exists() {
        println!("Ledger is empty.");
        return Ok(());
    }
    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    print!("{contents}");
    Ok(())
}
