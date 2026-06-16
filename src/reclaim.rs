use anyhow::{Context, Result};
use std::path::Path;
use walkdir::WalkDir;

/// Remove the private target/ directory and return bytes freed.
pub fn clean(repo_path: &Path) -> Result<u64> {
    let target = repo_path.join("target");
    if !target.exists() {
        return Ok(0);
    }
    let bytes = measure_dir(&target);
    std::fs::remove_dir_all(&target)
        .with_context(|| format!("removing {}", target.display()))?;
    Ok(bytes)
}

pub fn measure_dir(path: &Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}
