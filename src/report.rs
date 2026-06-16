use crate::plan::{Classification, RepoEntry};
use humansize::{format_size, DECIMAL};

pub fn print_plan(entries: &[RepoEntry]) {
    if entries.is_empty() {
        println!("No repos found.");
        return;
    }
    let mut safe_bytes: u64 = 0;
    let mut counts = std::collections::HashMap::new();
    for e in entries {
        *counts.entry(e.classification.to_string()).or_insert(0u32) += 1;
        if e.classification == Classification::SafeToClean {
            safe_bytes += e.target_bytes;
        }
    }
    println!("{:<30} {:<20} TARGET SIZE", "REPO", "CLASSIFICATION");
    println!("{}", "-".repeat(70));
    for e in entries {
        println!(
            "{:<30} {:<20} {}",
            e.name,
            e.classification.to_string(),
            if e.target_bytes > 0 { format_size(e.target_bytes, DECIMAL) } else { "-".to_string() }
        );
    }
    println!("{}", "-".repeat(70));
    println!("Summary:");
    let mut sorted_counts: Vec<_> = counts.iter().collect();
    sorted_counts.sort_by_key(|(k, _)| k.as_str());
    for (cls, count) in sorted_counts {
        println!("  {cls}: {count}");
    }
    println!("Total reclaimable: {}", format_size(safe_bytes, DECIMAL));
}
