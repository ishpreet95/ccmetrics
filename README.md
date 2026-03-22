# ccmetrics

[![CI](https://github.com/ishpreet95/ccmetrics/actions/workflows/ci.yml/badge.svg)](https://github.com/ishpreet95/ccmetrics/actions/workflows/ci.yml)

Honest token usage metrics for Claude Code.

## What it does

Parses Claude Code JSONL session files, correctly deduplicates streaming chunks, disaggregates 5 token types with per-tier pricing, and calculates accurate API-equivalent costs.

## Why

Every Claude Code usage tool gets the math wrong. We [researched why](https://ishpreet95.me/blog/understanding-claude-code-token-metrics) and built the correct implementation:

| Tool | Output Tokens | Total Cost | Problem |
|------|:---:|:---:|---|
| **ccmetrics** | 8,625,351 | $2,376 | Correct (final chunk, 5-type split) |
| ccusage | 2,975,552 | $2,032 | First-seen-wins keeps placeholder tokens |
| claudelytics | 12,750,257 | $17,703 | No dedup, counts every streaming chunk |

## Install

```bash
cargo install ccmetrics
```

Or build from source:

```bash
cargo install --path .
```

## Usage

```bash
ccmetrics                        # Dashboard with token breakdown, cost, by-model/project tables
ccmetrics daily                  # Daily breakdown (one row per day, totals, averages)
ccmetrics session                # List 20 most recent sessions
ccmetrics session <id>           # Drill into a session by ID (prefix match)
ccmetrics explain                # Walk through the methodology on your data
```

### Filters

```bash
ccmetrics --since 7d             # Last 7 days (also: 2w, 30d, today, 2026-03-01)
ccmetrics --until 2026-03-15     # Up to a date (inclusive)
ccmetrics --model opus           # Filter by model (case-insensitive substring)
ccmetrics --project myapp        # Filter by project name
ccmetrics daily --since 7d       # Filters work with all subcommands
```

### Output options

```bash
ccmetrics --json                 # JSON output (stable machine-readable contract)
ccmetrics --verbose              # Detailed stats (file counts, warnings, dedup details)
ccmetrics --quiet                # Suppress streaming pipeline output
```

## What makes it different

- **Correct dedup** -- groups by `requestId`, keeps final chunk (`stop_reason != null`) with real token counts
- **5-type token split** -- input, output, cache read, cache write 5m, cache write 1h (each at different pricing)
- **Per-model breakdown** -- token and cost split by model for independent verification
- **Per-project breakdown** -- usage grouped by project (shown when 2+ projects)
- **Main vs subagent** -- separates main thread from subagent usage
- **Daily and session views** -- track usage over time, drill into individual sessions
- **Streaming pipeline** -- real-time progress with step summaries (scan, parse, dedup, filter, calculate)
- **Date, model, and project filters** -- slice data by time range, model, or project
- **Pricing modifiers** -- fast mode (6x), data residency (1.1x), long context (2x/1.5x)
- **Explain mode** -- `ccmetrics explain` walks through dedup, pricing, and cache tiers using your own data
- **Abbreviated numbers** -- large token counts display as 2.86B, 6.1M, 260K for readability
- **No runtime** -- single Rust binary, no network, no database

## Docs

- [PRD](docs/PRD.md) -- product requirements (v1.3)
- [Architecture](docs/ARCHITECTURE.md) -- module layout, data flow
- [Pricing](docs/PRICING.md) -- embedded pricing table reference
- [Research blog](https://ishpreet95.me/blog/understanding-claude-code-token-metrics) -- full analysis of why tools disagree

## License

MIT
