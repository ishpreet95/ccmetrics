mod analysis;
mod dedup;
mod explain;
mod output;
mod parser;
mod pricing;
mod scanner;
mod types;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rayon::prelude::*;

use types::ParseStats;

/// Honest token metrics for Claude Code.
///
/// Parses JSONL session files, deduplicates streaming chunks,
/// disaggregates 5 token types, and calculates accurate costs.
#[derive(Parser, Debug)]
#[command(name = "ccmetrics", version, about)]
struct Cli {
    /// Output as JSON instead of table
    #[arg(long)]
    json: bool,

    /// Show verbose details (dedup stats, file counts, modifiers)
    #[arg(long, short)]
    verbose: bool,

    /// Path to Claude Code projects directory
    #[arg(long, default_value_os_t = default_claude_path())]
    path: PathBuf,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Walk through the methodology on your own data — show your work
    Explain,
}

fn default_claude_path() -> PathBuf {
    PathBuf::from(
        std::env::var("HOME").unwrap_or_else(|_| ".".to_string()),
    )
    .join(".claude")
    .join("projects")
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Step 1: Scan for JSONL files
    let files = scanner::scan_jsonl_files(&cli.path)
        .with_context(|| format!("Failed to scan {}", cli.path.display()))?;

    if files.is_empty() {
        if cli.json {
            println!("{{\"error\": \"No JSONL files found\"}}");
        } else {
            eprintln!("No JSONL files found in {}", cli.path.display());
            eprintln!("Expected Claude Code sessions at ~/.claude/projects/**/*.jsonl");
        }
        return Ok(());
    }

    let main_file_count = files.iter().filter(|f| !scanner::is_subagent_path(f)).count();
    let sub_file_count = files.len() - main_file_count;

    // Step 2: Parse all files in parallel
    let parse_results: Vec<_> = files
        .par_iter()
        .map(|file| {
            let project_path = scanner::extract_project_path(file);
            parser::parse_jsonl_file(file, &project_path)
        })
        .collect();

    // Collect results
    let mut all_entries = Vec::new();
    let mut all_warnings = Vec::new();
    let mut stats = ParseStats {
        total_files: files.len(),
        main_files: main_file_count,
        subagent_files: sub_file_count,
        ..Default::default()
    };

    for result in parse_results {
        stats.raw_lines += result.raw_line_count;
        stats.assistant_lines += result.assistant_lines;
        stats.skipped_lines += result.skipped_lines;
        stats.synthetic_messages += result.synthetic_count;
        all_entries.extend(result.entries);
        all_warnings.extend(result.warnings);
    }

    // Step 3: Deduplicate
    // Only clone raw entries when explain mode needs them (avoids ~90K entry copy otherwise)
    let raw_entries_for_explain = if matches!(cli.command, Some(Commands::Explain)) {
        Some(all_entries.clone())
    } else {
        None
    };
    let (deduped, no_id_count) = dedup::deduplicate(all_entries);
    stats.no_id_entries = no_id_count;

    // Step 4: Analyze
    let summary = analysis::analyze(&deduped, &stats);

    // Step 5: Route to output mode
    match cli.command {
        Some(Commands::Explain) => {
            let raw = raw_entries_for_explain.expect("raw entries should be cloned for explain mode");
            let data = explain::build_explain(&raw, &deduped, &summary);
            let rendered = output::explain::render(&data, &summary.version);
            print!("{rendered}");
        }
        None => {
            if cli.json {
                let json = output::json::render(&summary)
                    .context("Failed to serialize JSON")?;
                println!("{json}");
            } else {
                let table = output::table::render(&summary);
                print!("{table}");

                if cli.verbose {
                    println!();
                    println!("Verbose Details");
                    println!("{}", "─".repeat(60));
                    println!(
                        "Files scanned:     {} ({} main + {} subagent)",
                        stats.total_files, stats.main_files, stats.subagent_files
                    );
                    println!("Skipped lines:     {} (malformed JSON)", stats.skipped_lines);
                    println!("No-ID entries:     {} (counted once, not deduplicated)", stats.no_id_entries);
                    println!("Synthetic msgs:    {} (excluded)", stats.synthetic_messages);

                    if !all_warnings.is_empty() {
                        println!();
                        println!("Warnings:");
                        for w in &all_warnings {
                            if let Some(line) = w.line {
                                eprintln!("  {}:{}: {}", w.file.display(), line, w.message);
                            } else {
                                eprintln!("  {}: {}", w.file.display(), w.message);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
