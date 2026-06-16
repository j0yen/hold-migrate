use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Classification {
    SafeToClean,
    NotAnchored,
    BinaryStale,
    BuildInFlight,
    NoTarget,
}

impl std::fmt::Display for Classification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SafeToClean => write!(f, "safe-to-clean"),
            Self::NotAnchored => write!(f, "not-anchored"),
            Self::BinaryStale => write!(f, "binary-stale"),
            Self::BuildInFlight => write!(f, "build-in-flight"),
            Self::NoTarget => write!(f, "no-target"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RepoEntry {
    pub name: String,
    pub path: PathBuf,
    pub classification: Classification,
    pub target_bytes: u64,
}

pub fn scan(root: &Path) -> Result<Vec<RepoEntry>> {
    let mut entries = Vec::new();
    for entry in WalkDir::new(root).min_depth(1).max_depth(1).into_iter().filter_map(|e| e.ok()) {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let cargo_toml = dir.join("Cargo.toml");
        if !cargo_toml.exists() {
            continue;
        }
        let name = dir.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        let target_dir = dir.join("target");
        let classification = classify(dir, &target_dir, &name)?;
        let target_bytes = if target_dir.exists() {
            dir_size(&target_dir)
        } else {
            0
        };
        entries.push(RepoEntry { name, path: dir.to_path_buf(), classification, target_bytes });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

pub fn classify(repo: &Path, target_dir: &Path, name: &str) -> Result<Classification> {
    // 1. No target?
    if !target_dir.exists() {
        return Ok(Classification::NoTarget);
    }
    // 2. Build in flight?
    if target_dir.join(".cargo-lock").exists() {
        return Ok(Classification::BuildInFlight);
    }
    // 3. Anchored?
    if !is_anchored(repo) {
        return Ok(Classification::NotAnchored);
    }
    // 4. Binary stale?
    if is_binary_stale(repo, name)? {
        return Ok(Classification::BinaryStale);
    }
    Ok(Classification::SafeToClean)
}

pub fn is_anchored(repo: &Path) -> bool {
    // Check CARGO_TARGET_DIR env var
    if std::env::var("CARGO_TARGET_DIR").is_ok() {
        return true;
    }
    // Check .cargo/config.toml for target-dir
    let config_paths = [
        repo.join(".cargo").join("config.toml"),
        repo.join(".cargo").join("config"),
    ];
    for config_path in &config_paths {
        if let Ok(contents) = std::fs::read_to_string(config_path) {
            if contents.contains("target-dir") {
                return true;
            }
        }
    }
    false
}

pub fn is_binary_stale(repo: &Path, name: &str) -> Result<bool> {
    let home = std::env::var("HOME").unwrap_or_default();
    let bin_path = PathBuf::from(&home).join(".cargo").join("bin").join(name);
    if !bin_path.exists() {
        // No installed binary — treat as stale (can't verify it's current)
        return Ok(true);
    }
    let bin_mtime = bin_path.metadata()
        .and_then(|m| m.modified())
        .context("reading binary mtime")?;
    // Find latest .rs file mtime under src/
    let src_dir = repo.join("src");
    if !src_dir.exists() {
        return Ok(false); // No src dir — assume not stale
    }
    let latest_src = WalkDir::new(&src_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "rs").unwrap_or(false))
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .max();
    match latest_src {
        Some(src_mtime) => Ok(src_mtime > bin_mtime),
        None => Ok(false),
    }
}

pub fn dir_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}
