use std::path::Path;

/// Returns true if a build is currently holding the target dir.
#[allow(dead_code)]
pub fn is_in_flight(target_dir: &Path) -> bool {
    target_dir.join(".cargo-lock").exists()
}
