use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod guard;
mod ledger;
mod plan;
mod reclaim;
mod report;

#[derive(Parser)]
#[command(name = "hold-migrate", about = "Drain private target/ dirs into the shared hold, safely")]
struct Cli {
    /// Root directory to scan for repos
    #[arg(long, default_value = "~/wintermute")]
    root: String,
    /// Override timestamp for ledger lines (RFC 3339), useful for deterministic tests
    #[arg(long)]
    ts: Option<String>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Dry-run: classify all repos, print summary
    Plan,
    /// Clean safe-to-clean repos and record to ledger
    Apply {
        /// Only migrate this specific repo name
        #[arg(long)]
        repo: Option<String>,
    },
    /// Print the append-only ledger
    Ledger,
}

fn main() {
    sigpipe::reset();
    if let Err(e) = run() {
        eprintln!("Error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let root = expand_tilde(&cli.root);
    match cli.command {
        Commands::Plan => {
            let entries = plan::scan(&root)?;
            report::print_plan(&entries);
        }
        Commands::Apply { repo } => {
            let entries = plan::scan(&root)?;
            let safe: Vec<_> = entries
                .iter()
                .filter(|e| e.classification == plan::Classification::SafeToClean)
                .filter(|e| {
                    repo.as_ref()
                        .map(|r| e.name == *r)
                        .unwrap_or(true)
                })
                .collect();
            if safe.is_empty() {
                println!("Nothing to migrate.");
                return Ok(());
            }
            let ledger_path = ledger::default_path()?;
            for entry in safe {
                let bytes = reclaim::clean(&entry.path)?;
                let ts = cli.ts.clone().unwrap_or_else(|| {
                    chrono::Utc::now().to_rfc3339()
                });
                ledger::append(&ledger_path, &entry.name, bytes, &entry.classification, &ts)?;
                println!(
                    "Cleaned {} — freed {}",
                    entry.name,
                    humansize::format_size(bytes, humansize::DECIMAL)
                );
            }
        }
        Commands::Ledger => {
            ledger::print_all()?;
        }
    }
    Ok(())
}

pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        let home = std::env::var("HOME").unwrap_or_default();
        PathBuf::from(home).join(rest)
    } else {
        PathBuf::from(path)
    }
}
