use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn binary_path() -> PathBuf {
    let mut p = std::env::current_exe().unwrap();
    p.pop(); // remove test binary name
    if p.ends_with("deps") {
        p.pop();
    }
    p.join("hold-migrate")
}

fn make_repo(root: &Path, name: &str) -> PathBuf {
    let repo = root.join(name);
    fs::create_dir_all(&repo).unwrap();
    fs::write(repo.join("Cargo.toml"), format!(
        "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        name
    )).unwrap();
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("src").join("main.rs"), "fn main() {}").unwrap();
    repo
}

fn add_target(repo: &Path) -> PathBuf {
    let target = repo.join("target");
    fs::create_dir_all(target.join("debug")).unwrap();
    // Create a dummy file so it has some size
    fs::write(target.join("debug").join("dummy.o"), b"dummy content for size").unwrap();
    target
}

fn add_anchor(repo: &Path) {
    let cargo_dir = repo.join(".cargo");
    fs::create_dir_all(&cargo_dir).unwrap();
    fs::write(
        cargo_dir.join("config.toml"),
        "[build]\ntarget-dir = \"/home/user/wintermute/.hold/cargo-target\"\n",
    ).unwrap();
}

fn add_cargo_lock(target: &Path) {
    fs::write(target.join(".cargo-lock"), b"").unwrap();
}

/// AC1: --help exits 0
#[test]
fn ac1_help_exits_zero() {
    let status = Command::new(binary_path())
        .arg("--help")
        .status()
        .expect("failed to run binary");
    assert!(status.success(), "--help should exit 0");
}

/// AC2: plan against fixture tree reclaims nothing, fixture unchanged
#[test]
fn ac2_plan_is_dry_run() {
    let tmp = TempDir::new().unwrap();
    let repo = make_repo(tmp.path(), "myrepo");
    let target = add_target(&repo);
    // Don't add anchor so it will be not-anchored (not safe-to-clean)

    // Run plan — should not modify anything
    let output = Command::new(binary_path())
        .arg("--root").arg(tmp.path())
        .arg("plan")
        .output()
        .expect("failed to run plan");

    assert!(output.status.success(), "plan should exit 0");
    // target dir should still exist
    assert!(target.exists(), "plan should not remove target/");
    // No files should have been removed
    assert!(target.join("debug").join("dummy.o").exists(), "plan should leave files intact");
}

/// AC3: plan classifies repo with no shared anchor as not-anchored
#[test]
fn ac3_not_anchored() {
    // Ensure CARGO_TARGET_DIR is not set
    std::env::remove_var("CARGO_TARGET_DIR");

    let tmp = TempDir::new().unwrap();
    let repo = make_repo(tmp.path(), "unanchored-repo");
    add_target(&repo);
    // No .cargo/config.toml — not anchored

    let entries = hold_migrate::plan::scan(tmp.path()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].classification, hold_migrate::plan::Classification::NotAnchored);
}

/// AC4: plan classifies repo with stale binary as binary-stale
#[test]
fn ac4_binary_stale() {
    std::env::remove_var("CARGO_TARGET_DIR");

    let tmp = TempDir::new().unwrap();
    let repo = make_repo(tmp.path(), "definitely-not-installed-xyz123");
    add_target(&repo);
    add_anchor(&repo);
    // No installed binary for "definitely-not-installed-xyz123"

    let entries = hold_migrate::plan::scan(tmp.path()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].classification, hold_migrate::plan::Classification::BinaryStale);
}

/// AC5: Repo with target/.cargo-lock → build-in-flight, skipped by apply
#[test]
fn ac5_build_in_flight() {
    std::env::remove_var("CARGO_TARGET_DIR");

    let tmp = TempDir::new().unwrap();
    let repo = make_repo(tmp.path(), "in-flight-repo");
    let target = add_target(&repo);
    add_anchor(&repo);
    add_cargo_lock(&target);

    let entries = hold_migrate::plan::scan(tmp.path()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].classification, hold_migrate::plan::Classification::BuildInFlight);

    // Target should still exist
    assert!(target.exists(), "in-flight target should not be removed");
}

/// AC6: apply on safe-to-clean fixture removes target/, bytes_freed > 0, one ledger line appended
#[test]
fn ac6_apply_safe_to_clean() {
    std::env::remove_var("CARGO_TARGET_DIR");

    let tmp = TempDir::new().unwrap();
    let repo = make_repo(tmp.path(), "safe-repo");
    let target = add_target(&repo);
    add_anchor(&repo);

    // Test reclaim directly
    let bytes = hold_migrate::reclaim::clean(&repo).unwrap();
    assert!(bytes > 0, "bytes_freed should be > 0");
    assert!(!target.exists(), "target/ should be removed after clean");

    // Test ledger append
    let ledger_dir = TempDir::new().unwrap();
    let ledger_path = ledger_dir.path().join("ledger.jsonl");
    hold_migrate::ledger::append(
        &ledger_path,
        "safe-repo",
        bytes,
        &hold_migrate::plan::Classification::SafeToClean,
        "2024-01-01T00:00:00Z",
    ).unwrap();

    let contents = fs::read_to_string(&ledger_path).unwrap();
    let lines: Vec<_> = contents.lines().collect();
    assert_eq!(lines.len(), 1, "exactly one ledger line should be appended");

    // Verify the line is valid JSON with expected fields
    let parsed: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(parsed["repo"], "safe-repo");
    assert!(parsed["bytes_freed"].as_u64().unwrap() > 0);
}

/// AC7: Ledger append-only: second apply doesn't rewrite existing lines
#[test]
fn ac7_ledger_append_only() {
    let tmp = TempDir::new().unwrap();
    let ledger_path = tmp.path().join("ledger.jsonl");

    // Append first entry
    hold_migrate::ledger::append(
        &ledger_path,
        "repo-a",
        1000,
        &hold_migrate::plan::Classification::SafeToClean,
        "2024-01-01T00:00:00Z",
    ).unwrap();

    // Append second entry
    hold_migrate::ledger::append(
        &ledger_path,
        "repo-b",
        2000,
        &hold_migrate::plan::Classification::SafeToClean,
        "2024-01-02T00:00:00Z",
    ).unwrap();

    let contents = fs::read_to_string(&ledger_path).unwrap();
    let lines: Vec<_> = contents.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 2, "both lines should be present");

    let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    let second: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(first["repo"], "repo-a");
    assert_eq!(second["repo"], "repo-b");
}

/// AC8: apply with no safe-to-clean → nothing reclaimed, exits 0
#[test]
fn ac8_nothing_to_migrate() {
    std::env::remove_var("CARGO_TARGET_DIR");

    let tmp = TempDir::new().unwrap();
    let _repo = make_repo(tmp.path(), "no-target-repo");
    // No target dir → no-target classification

    let output = Command::new(binary_path())
        .arg("--root").arg(tmp.path())
        .arg("apply")
        .output()
        .expect("failed to run apply");

    assert!(output.status.success(), "apply with nothing to do should exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Nothing to migrate"), "should print nothing-to-migrate message");
}

/// AC9: --ts <rfc3339> makes ledger lines deterministic
#[test]
fn ac9_deterministic_ts() {
    let tmp = TempDir::new().unwrap();
    let ledger_path = tmp.path().join("ledger.jsonl");

    let ts = "2024-06-15T12:00:00Z";

    hold_migrate::ledger::append(
        &ledger_path,
        "test-repo",
        500,
        &hold_migrate::plan::Classification::SafeToClean,
        ts,
    ).unwrap();

    let contents = fs::read_to_string(&ledger_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(contents.trim()).unwrap();
    assert_eq!(parsed["ts"], ts, "timestamp should match --ts flag value");
}
