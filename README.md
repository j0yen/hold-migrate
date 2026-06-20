# hold-migrate

Reclaims the per-repo `target/` directories left behind after a repo moves to the shared Cargo hold — but only the ones it can prove are safe to delete.

## Why it exists

Moving a repo onto a shared Cargo target (via `CARGO_TARGET_DIR` or `.cargo/config.toml`) doesn't remove the private `target/` it built up before the switch. That directory is now dead weight: gigabytes of artifacts nothing will read again. Deleting it by hand is the obvious move and the dangerous one — delete the wrong `target/` and you cold-rebuild a repo that wasn't actually anchored, or interrupt a build mid-flight.

hold-migrate makes the safe deletion the easy one. It scans your repos, classifies each private `target/`, and cleans only those it can show are redundant.

## Install

```sh
cargo install --path .
```

## Usage

```sh
# Dry-run: classify every repo under the root, print the summary. Touches nothing.
hold-migrate plan

# Clean the repos classified safe-to-clean, recording each to the ledger.
hold-migrate apply

# Clean a single repo by name.
hold-migrate apply --repo my-crate

# Print the append-only ledger of everything reclaimed.
hold-migrate ledger
```

The scan root defaults to `~/wintermute` (`--root` to change it). `--ts <rfc3339>` pins ledger timestamps for deterministic tests.

## How it works

For each repo with a `Cargo.toml` directly under the root, hold-migrate assigns one classification:

| Classification | Meaning | Cleaned? |
| --- | --- | --- |
| `safe-to-clean` | Anchored to the shared hold, installed binary is current, no build in flight | yes |
| `not-anchored` | No `CARGO_TARGET_DIR` and no `target-dir` in `.cargo/config.toml` — its private `target/` is still live | no |
| `binary-stale` | Source is newer than the installed binary, so the artifacts may not be reproducible yet | no |
| `build-in-flight` | A `.cargo-lock` is present | no |
| `no-target` | Nothing to reclaim | no |

Only `safe-to-clean` is ever deleted, and `plan` shows you the full classification before you commit to anything. The conservative cases are deliberately conservative: when hold-migrate can't prove a `target/` is redundant, it leaves it alone. Every `apply` appends to an append-only JSONL ledger recording what was cleaned and how many bytes it freed.

## Where it fits

hold-migrate is part of the `hold-*` family that manages the wintermute fleet's shared Cargo target:

- **hold-survey** — measures dependency duplication across the fleet
- **hold-migrate** — drains private `target/` dirs into the shared hold (this repo)
- **hold-anchor** — deduplicates artifacts across machines
- **hold-guard** — bounds the hold's budget with LRU eviction

hold-migrate gets repos onto the shared hold cleanly; hold-guard keeps that hold within budget afterward.
