mod analysis;
mod dedup;
mod explain;
mod filters;
mod output;
mod parser;
mod pipeline;
mod pricing;
mod scanner;
mod types;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rayon::prelude::*;

use filters::Filters;
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

    /// Suppress pipeline progress output
    #[arg(long, short)]
    quiet: bool,

    /// Filter: only include entries from this date onward (2026-03-01, 7d, 2w, today). Dates are UTC.
    #[arg(long)]
    since: Option<String>,

    /// Filter: only include entries up to this date, inclusive (2026-03-01, 7d, today). Dates are UTC.
    #[arg(long)]
    until: Option<String>,

    /// Filter: only include entries matching this model (fuzzy: opus, sonnet, haiku)
    #[arg(long)]
    model: Option<String>,

    /// Filter: only include entries from projects matching this pattern
    #[arg(long)]
    project: Option<String>,

    /// Path to Claude Code projects directory
    #[arg(long, default_value_os_t = default_claude_path())]
    path: PathBuf,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Daily breakdown — one row per day
    Daily,
    /// List sessions, or drill into one by ID
    Session {
        /// Session ID to drill into (prefix match, 8+ chars)
        id: Option<String>,
    },
    /// Walk through the methodology on your own data — show your work
    Explain,
}

fn default_claude_path() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
        .join(".claude")
        .join("projects")
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Parse filter flags
    let filter = build_filters(&cli)?;

    // Should we show the streaming pipeline?
    let show_pipeline = !cli.json && !cli.quiet && pipeline::should_show();

    if show_pipeline {
        pipeline::header();
    }

    // Step 1: Scan for JSONL files
    let step = if show_pipeline {
        Some(pipeline::PipelineStep::start("Scanning for session files"))
    } else {
        None
    };

    let files = scanner::scan_jsonl_files(&cli.path)
        .with_context(|| format!("Failed to scan {}", cli.path.display()))?;

    if files.is_empty() {
        if let Some(s) = step {
            s.warn("No JSONL files found");
        }
        if cli.json {
            println!("{{\"error\": \"No JSONL files found\"}}");
        } else {
            eprintln!("No JSONL files found in {}", cli.path.display());
            eprintln!("Expected Claude Code sessions at ~/.claude/projects/**/*.jsonl");
        }
        return Ok(());
    }

    let main_file_count = files
        .iter()
        .filter(|f| !scanner::is_subagent_path(f))
        .count();
    let sub_file_count = files.len() - main_file_count;

    if let Some(s) = step {
        s.done(&format!(
            "Found {} files ({} main + {} subagent)",
            files.len(),
            main_file_count,
            sub_file_count
        ));
    }

    // Step 2: Parse all files in parallel
    let step = if show_pipeline {
        Some(pipeline::PipelineStep::start("Parsing JSONL entries"))
    } else {
        None
    };

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

    if let Some(s) = step {
        let mut detail = format!("{} assistant entries", stats.assistant_lines);
        if stats.skipped_lines > 0 {
            detail.push_str(&format!(", {} skipped", stats.skipped_lines));
        }
        if stats.synthetic_messages > 0 {
            detail.push_str(&format!(
                ", {} synthetic excluded",
                stats.synthetic_messages
            ));
        }
        s.done(&detail);
    }

    // Step 3: Deduplicate
    let step = if show_pipeline {
        Some(pipeline::PipelineStep::start("Deduplicating by requestId"))
    } else {
        None
    };

    // Only clone raw entries when explain mode needs them
    let raw_entries_for_explain = if matches!(cli.command, Some(Commands::Explain)) {
        Some(all_entries.clone())
    } else {
        None
    };
    let (deduped, no_id_count) = dedup::deduplicate(all_entries);
    stats.no_id_entries = no_id_count;

    // Store pre-filter unique count for accurate dedup ratio
    // (analysis.rs uses this, not the post-filter count)
    let pre_filter_unique = deduped.len();
    stats.unique_after_dedup = pre_filter_unique;

    let dedup_ratio = if pre_filter_unique == 0 {
        0.0
    } else {
        stats.assistant_lines as f64 / pre_filter_unique as f64
    };

    if let Some(s) = step {
        s.done(&format!(
            "{} unique requests ({:.1}x reduction)",
            pre_filter_unique, dedup_ratio
        ));
    }

    // Step 4: Apply filters (if any)
    let deduped = if filter.is_active() {
        let step = if show_pipeline {
            Some(pipeline::PipelineStep::start("Applying filters"))
        } else {
            None
        };

        let before = deduped.len();
        let filtered = filter.apply(deduped);

        if let Some(s) = step {
            s.done(&format!(
                "{} → {} entries ({})",
                before,
                filtered.len(),
                filter.describe()
            ));
        }

        filtered
    } else {
        deduped
    };

    // Step 5: Analyze
    let step = if show_pipeline {
        Some(pipeline::PipelineStep::start("Calculating costs"))
    } else {
        None
    };

    let summary = analysis::analyze(&deduped, &stats);

    if let Some(s) = step {
        let model_count = summary.by_model.len();
        s.done(&format!(
            "${:.2} total ({} model{}, {} token types)",
            summary.cost.total(),
            model_count,
            if model_count == 1 { "" } else { "s" },
            5
        ));
    }

    if show_pipeline {
        pipeline::separator();
    }

    // Step 6: Route to output mode
    match cli.command {
        Some(Commands::Daily) => {
            let days = analysis::analyze_daily(&deduped);
            if cli.json {
                let json = output::daily::render_json(&days, &summary.version, &filter)
                    .context("Failed to serialize JSON")?;
                println!("{json}");
            } else {
                let table = output::daily::render(&days, &summary.version, &filter);
                print!("{table}");
            }
        }
        Some(Commands::Session { id }) => {
            let sessions = analysis::analyze_sessions(&deduped);
            match id {
                Some(needle) => {
                    // Prefer exact match, then fall back to prefix match
                    let exact: Vec<_> =
                        sessions.iter().filter(|s| s.session_id == needle).collect();
                    let matches = if exact.is_empty() {
                        sessions
                            .iter()
                            .filter(|s| s.session_id.starts_with(&needle))
                            .collect()
                    } else {
                        exact
                    };

                    match matches.len() {
                        0 => {
                            anyhow::bail!(
                                "No session found matching '{needle}'. Run 'ccmetrics session' to see available session IDs."
                            );
                        }
                        1 => {
                            let session = matches[0];
                            if cli.json {
                                let json = output::session::render_detail_json(
                                    session,
                                    &summary.version,
                                    &filter,
                                )
                                .context("Failed to serialize JSON")?;
                                println!("{json}");
                            } else {
                                let detail = output::session::render_detail(session);
                                print!("{detail}");
                            }
                        }
                        n => {
                            let shown = n.min(5);
                            let mut msg = format!(
                                "Ambiguous: '{needle}' matches {n} sessions. Provide more characters.\n"
                            );
                            for m in &matches[..shown] {
                                msg.push_str(&format!("  {} ({})\n", &m.session_id, m.date));
                            }
                            if n > shown {
                                msg.push_str(&format!("  ... and {} more\n", n - shown));
                            }
                            anyhow::bail!("{}", msg.trim_end());
                        }
                    }
                }
                None => {
                    // List mode
                    if cli.json {
                        let json =
                            output::session::render_list_json(&sessions, &summary.version, &filter)
                                .context("Failed to serialize JSON")?;
                        println!("{json}");
                    } else {
                        let table =
                            output::session::render_list(&sessions, &summary.version, &filter);
                        print!("{table}");
                    }
                }
            }
        }
        Some(Commands::Explain) => {
            let raw = raw_entries_for_explain
                .context("internal error: raw entries not captured for explain mode")?;
            let data = explain::build_explain(&raw, &deduped, &summary);
            let rendered = output::explain::render(&data, &summary.version);
            print!("{rendered}");
        }
        None => {
            if cli.json {
                let json =
                    output::json::render(&summary, &filter).context("Failed to serialize JSON")?;
                println!("{json}");
            } else {
                let table = output::table::render(&summary, &filter);
                print!("{table}");

                if cli.verbose {
                    println!();
                    println!("Verbose Details");
                    println!("{}", "─".repeat(60));
                    println!(
                        "Files scanned:     {} ({} main + {} subagent)",
                        stats.total_files, stats.main_files, stats.subagent_files
                    );
                    println!(
                        "Skipped lines:     {} (malformed JSON)",
                        stats.skipped_lines
                    );
                    println!(
                        "No-ID entries:     {} (counted once, not deduplicated)",
                        stats.no_id_entries
                    );
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

/// Build filter criteria from CLI flags.
fn build_filters(cli: &Cli) -> Result<Filters> {
    let since = cli
        .since
        .as_deref()
        .map(filters::parse_date)
        .transpose()
        .map_err(|e| anyhow::anyhow!(e))?;

    let until = cli
        .until
        .as_deref()
        .map(|s| {
            filters::parse_date(s).map(|dt| {
                // --until is inclusive: "until 2026-03-15" means the entire day.
                // We use a half-open interval: entries with timestamp < (start of next day).
                dt + chrono::Duration::days(1)
            })
        })
        .transpose()
        .map_err(|e| anyhow::anyhow!(e))?;

    Ok(Filters {
        since,
        until,
        model: cli.model.clone(),
        project: cli.project.clone(),
    })
}
