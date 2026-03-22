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
cargo install --path .
```

## Usage

```bash
ccmetrics              # Summary table
ccmetrics --json       # JSON output (includes per-model breakdown)
ccmetrics --verbose    # Detailed stats
ccmetrics explain      # Walk through the methodology on your data
```

## What makes it different

- **Correct dedup** -- groups by `requestId`, keeps final chunk (`stop_reason != null`) with real token counts
- **5-type token split** -- input, output, cache read, cache write 5m, cache write 1h (each at different pricing)
- **Per-model breakdown** -- token and cost split by model for independent verification
- **Main vs subagent** -- separates main thread from subagent usage
- **Pricing modifiers** -- fast mode (6x), data residency (1.1x), long context (2x/1.5x)
- **Explain mode** -- `ccmetrics explain` walks through dedup, pricing, and cache tiers using your own data
- **No runtime** -- single Rust binary, no network, no database

## Docs

- [PRD](docs/PRD.md) -- product requirements (v1.3)
- [Architecture](docs/ARCHITECTURE.md) -- module layout, data flow
- [Pricing](docs/PRICING.md) -- embedded pricing table reference
- [Research blog](https://ishpreet95.me/blog/understanding-claude-code-token-metrics) -- full analysis of why tools disagree

## Status

Phase 1 MVP complete. 47 tests passing (37 unit + 10 integration). CI on GitHub Actions (ubuntu + macOS).
