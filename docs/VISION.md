# cc-metrics — Claude Code Usage Metrics CLI

> Repo name `cc-metrics` is a working name. Product/brand name TBD — will emerge as we define user stories and find the right angle.

## What This Is

A CLI tool that reads Claude Code's local session data and surfaces **honest, disaggregated token metrics** with correct cost estimates.

Born out of research that found every existing tool gets the math wrong in different ways — streaming chunk duplication, cache cost miscalculation, missing subagent data, wrong dedup keys. The research is published at [ishpreet95.me](https://ishpreet95.me).

## The Problem (proven)

The same usage data produces wildly different numbers:

| Tool | Reports | What's actually happening |
|---|---|---|
| claudelytics | 8.2B tokens | No dedup + sums all token types including cache reads |
| `/stats` | 9.4M tokens | input + output only, no cache, no breakdown |
| ccusage | ~correct dedup | But undercounts output tokens ~5x (first-seen-wins bug) |
| **This tool** | ? | Correct dedup, disaggregated types, cache-aware costs |

## Core Principles

1. **Correctness over features** — get the math right before adding views
2. **Transparent methodology** — publish exactly how we count so users can verify
3. **Disaggregated by default** — never show a single "total" without context
4. **Minimal** — do one thing well before expanding scope
5. **Fast** — parse 1,000+ JSONL files in under a second

## What Correct Parsing Requires

From our research, a correct parser must:

1. **Deduplicate by `requestId`** (or `message.id` — they're 1:1), NOT `uuid`
2. **Keep the last chunk** per requestId (the one with `stop_reason != null`) — first chunk has placeholder output_tokens
3. **Scan recursively** including `subagents/` directories
4. **Separate main thread** (`isSidechain: false`) from subagent usage
5. **Distinguish 5 token types:** input, output, cache_read, cache_write_5m, cache_write_1h
6. **Apply per-model, per-type pricing** from Anthropic's published rates
7. **Handle pricing modifiers:** fast mode (6x), data residency (1.1x), long context (2x)

## Target Users

- Claude Code power users who want to understand their usage
- Developers anxious about token consumption / rate limits
- Teams evaluating Claude Code ROI
- Tool builders who need a reference implementation

## Status

**Phase:** Scaffolding — architecture and docs, implementation next.

## Related

- **Research:** [idea-incubator/research/20260322-claude-code-token-metrics-study.md](../idea-incubator/research/20260322-claude-code-token-metrics-study.md)
- **Blog post:** [blog/content/understanding-claude-code-token-metrics.md](../blog/content/understanding-claude-code-token-metrics.md)
- **Idea doc:** [idea-incubator/ideas/20260321-claude-code-token-usage-cli/idea.md](../idea-incubator/ideas/20260321-claude-code-token-usage-cli/idea.md)
