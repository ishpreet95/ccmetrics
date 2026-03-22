# ccmetrics

Honest token usage metrics for Claude Code.

## What it does

Parses Claude Code JSONL session files, correctly deduplicates streaming chunks, disaggregates 5 token types with per-tier pricing, and calculates accurate API-equivalent costs.

## Why

Every Claude Code usage tool gets the math wrong:

| Tool | Output Tokens | Total Cost | Problem |
|------|:---:|:---:|---|
| **ccmetrics** | 8,625,351 | $2,376 | Correct (final chunk, 5-type split) |
| ccusage | 2,975,552 | $2,032 | First-seen-wins keeps placeholder tokens |
| claudelytics | 12,750,257 | $17,703 | No dedup, counts every streaming chunk |

## Install

```bash
cargo install --path .
```

## Usage

```bash
ccmetrics              # Summary table
ccmetrics --json       # JSON output
ccmetrics --verbose    # Detailed stats
```

## What makes it different

- **Correct dedup** -- groups by `requestId`, keeps final chunk (`stop_reason != null`) with real token counts
- **5-type token split** -- input, output, cache read, cache write 5m, cache write 1h (each at different pricing)
- **Main vs subagent** -- separates main thread from subagent usage (subagents are 43% of requests)
- **Pricing modifiers** -- fast mode (6x), data residency (1.1x), long context (2x/1.5x)
- **No runtime** -- single Rust binary, no network, no database

## Docs

- [PRD](docs/PRD.md) -- product requirements (v1.3, scored 86/100)
- [Architecture](docs/ARCHITECTURE.md) -- module layout, data flow
- [Pricing](docs/PRICING.md) -- embedded pricing table reference

## Status

Phase 1 MVP complete. 30 tests passing. Running against real data.
