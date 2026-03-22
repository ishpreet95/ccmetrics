# Product Requirements Document: ccmetrics

**Version:** 1.3
**Date:** 2026-03-22
**Author:** Ishpreet (owner, sole contributor)
**Status:** v0.1.0 published on crates.io — Sprints 1-3 complete, UX pass shipped
**Scoring History:** `evaluations/prd/2026-03-22-prd-score-v1.md` (v1.0: 60/100), `evaluations/prd/2026-03-22-prd-score-v1.1.md` (v1.1: 78/100)

### Changelog

| Version | Date       | Score      | Changes                                                                                                                                                                                                           |
| ------- | ---------- | ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1.0     | 2026-03-22 | 60/100 (C) | Initial draft — problem, personas, user stories, NFRs, competitive analysis, phased delivery                                                                                                                      |
| 1.1     | 2026-03-22 | 78/100 (B) | Added: JSONL schema (S5), architecture + Rust struct (S6), output mockups (S7), risk assessment (S9), dependencies (S10), glossary (S15). Revised: phase decomposition into 28 tasks with dependency graphs (S13) |
| 1.2     | 2026-03-22 | 86/100 (A) | Added: named persona scenarios (S2), SMART success metrics (S11), FR-ID cross-references (S4→S7→S13), pricing modifier composition example (S7), open question owners (S14)                                       |
| 1.3     | 2026-03-22 | —          | Added: US-15 Explain Mode (FR-06) — "show your work" trust-building feature with output mockup (S7), Phase 3 task 3.6. Based on real comparison data: cc-metrics vs ccusage vs claudelytics                        |

## Table of Contents

1. [Problem Statement](#1-problem-statement)
2. [Target Users](#2-target-users)
3. [Product Vision](#3-product-vision)
4. [User Stories](#4-user-stories)
5. [Data Model & JSONL Schema](#5-data-model--jsonl-schema)
6. [Architecture & Data Flow](#6-architecture--data-flow)
7. [Output Specifications](#7-output-specifications)
8. [Non-Functional Requirements](#8-non-functional-requirements)
9. [Risk Assessment](#9-risk-assessment)
10. [Dependencies](#10-dependencies)
11. [Success Metrics](#11-success-metrics)
12. [Competitive Differentiation](#12-competitive-differentiation)
13. [Phased Delivery](#13-phased-delivery)
14. [Open Questions](#14-open-questions)
15. [Glossary](#15-glossary)

---

## 1. Problem Statement

Every Claude Code token usage tool gets the math wrong. Users see wildly different numbers from different tools — 2.36M, 9.4M, 8.2B — from the same data. This happens because:

1. **Streaming chunk duplication** — Claude writes 3-6 JSONL lines per response; naive tools count each line as a separate response (2.85x inflation)
2. **Wrong dedup key** — community tools use `uuid` (per-line) instead of `requestId` (per-API-request), overcounting by 1.6-6x
3. **First-seen-wins bug** — ccusage (11.8k stars) keeps the first streaming chunk which has placeholder output_tokens (~1-11), undercounting output tokens by ~5x
4. **Cache cost illusion** — tools calculate cost at a single rate, ignoring that cache reads cost 1/10th of input. This produces phantom costs (5.7x-29x overcounting)
5. **Missing token type granularity** — no tool distinguishes 5m vs 1h cache write pricing (1.25x vs 2x), applies fast mode (6x), data residency (1.1x), or long context (2x) modifiers
6. **Subagent blindspot** — subagents account for 53% of requests and 66% of tokens, but are either missed entirely or lumped in without distinction

The result: users see terrifying phantom costs ("\\$16,573 estimated"), can't understand what's actually consuming their token budget, and make decisions based on wrong numbers.

### Evidence

| Source                               | Finding                                                                            |
| ------------------------------------ | ---------------------------------------------------------------------------------- |
| Own research (77 days, 298 sessions) | 4 tools produce 4 different numbers from same data                                 |
| ccusage Issue #888                   | Output tokens undercounted ~5x (first-seen-wins bug, open)                         |
| ccusage Issue #899                   | 5m vs 1h cache write not distinguished, ~19% cost underestimate (open)             |
| ccusage Issue #313                   | Subagent tokens not tracked (open since Jul 2025)                                  |
| ccusage Issue #389                   | Potential double counting of tokens (open since Aug 2025)                          |
| Claude Code Issue #22686             | Output tokens incorrectly recorded in JSONL (Feb 2026)                             |
| HN 47096937                          | Anthropic team member confirmed MCP tools + CLAUDE.md eat context before real work |
| Reddit (Mar 2026)                    | Consensus: "Claude Code is higher quality but unusable" due to limits              |

---

## 2. Target Users

### Primary: Sarah — Claude Code Power User

Senior backend engineer at a Series B startup. Uses Claude Code 6+ hours/day on Max (\$200/mo). Hits rate limits 2-3 times per week and has no idea why some days burn faster than others.

**Characteristics:**

- Uses Claude Code 5+ hours/day across 3-4 projects
- On Pro (\$20/mo) or Max (\$100-200/mo) plans
- Hits rate limits regularly
- Has tried ccusage and /stats — numbers don't match each other or her experience
- Technically sophisticated, comfortable with CLI tools

**Scenario:** Sarah hits her third rate limit of the week on Tuesday afternoon. She runs `cc-metrics` and discovers that 66% of her tokens went to subagents she didn't realize were spawned — her Agent tool calls triggered 14 parallel research tasks. She runs `cc-metrics project` and sees her work project uses 4x more than her side project because of a bloated CLAUDE.md. She adjusts her workflow and stops hitting limits.

**Drives:** US-1 (token breakdown), US-3 (main vs subagent split), US-8 (per-project breakdown)

### Secondary: Marcus — Cost-Anxious Developer

Junior developer, 3 months into using Claude Code on a Pro plan (\$20/mo). Saw a Reddit post saying "the \$20 plan runs out after 12 prompts" and is worried he's being ripped off.

**Characteristics:**

- Recently started using Claude Code
- Concerned about the "\$20 plan runs out after 12 prompts" reports
- Wants to understand ROI ("am I getting \$2,184 of API value from a \$200/mo plan?")
- Less CLI-experienced but can follow install instructions

**Scenario:** Marcus installs ccusage and sees "\$4,200 estimated cost" — panic. He then installs `cc-metrics` and sees the disaggregated breakdown: \$4,200 was phantom cost from naive calculation. His real API-equivalent cost is \$340, meaning his \$20 Pro plan is giving him 17x ROI. The cache efficiency section shows 96% cache hit rate — his setup is actually working well. He stops worrying.

**Drives:** US-2 (cache-aware cost), US-11 (cache efficiency)

### Tertiary: Priya — Tool Builder & Researcher

Data engineer building an internal dashboard for her team's Claude Code usage across 8 developers. Needs programmatic access to correct token data.

**Characteristics:**

- Needs a reference implementation with documented methodology
- Wants `--json` output to pipe into team dashboards
- Cares about correctness proofs and reproducibility, not pretty terminal output

**Scenario:** Priya pipes `cc-metrics --json` into a Python script that aggregates usage across her team's machines (collected via a cron job). She validates the numbers against the published methodology doc and confirms they match the Python reference script within 0.1%. She trusts the tool enough to base budget forecasts on it.

**Drives:** US-4 (JSON output), US-5 (correct dedup), US-12 (verify command)

---

## 3. Product Vision

**One-liner:** The first Claude Code usage tool that publishes its methodology and proves the math is right.

**Positioning:** NOT "another ccusage." The tool that gets the numbers right, explains exactly how it counts, and lets users verify the math themselves.

**Core belief:** Trust through transparency. Every number should be traceable to a specific calculation with a documented methodology. Users shouldn't have to trust a black box.

---

## 4. User Stories

### P0 — Must Have (MVP)

#### FR-01 (US-1): Honest Token Summary

> **As a** Claude Code user,
> **I want to** see my token usage broken down by type (input, output, cache_read, cache_write_5m, cache_write_1h),
> **so that** I understand what's actually happening instead of seeing a single meaningless "total tokens" number.

**Acceptance criteria:**

- Default `cc-metrics` command shows a disaggregated summary table (see Section 7: Default Table Output)
- Each token type shown separately with count and percentage
- Deduplicated by `requestId`, keeping final chunk (with `stop_reason != null`) (see Section 5: Streaming Chunks)
- Scans all JSONL files including `subagents/` directories recursively
- Output fits in a standard terminal without scrolling (80+ columns, <30 rows)

**Implements:** Tasks 1.2, 1.3, 1.4, 1.6, 1.7 | **Persona:** Sarah

#### FR-02 (US-2): Cache-Aware Cost Estimate

> **As a** Claude Code user,
> **I want to** see what my usage would cost at API rates with correct per-type pricing,
> **so that** I can understand the value I'm getting from my subscription and stop panicking about phantom costs.

**Acceptance criteria:**

- Cost calculated per-model, per-token-type using Anthropic's published rates (see `docs/PRICING.md`)
- 5 distinct rates applied: input, output, cache_read (0.1x), cache_write_5m (1.25x), cache_write_1h (2x)
- Total cost shown alongside breakdown by type (see Section 7: Default Table Output, "API-Equiv Cost" column)
- Pricing table embedded in binary (no network requests)
- Shows "API-equivalent cost" framing, not "you owe this"

**Implements:** Task 1.5 | **Persona:** Marcus

#### FR-03 (US-3): Main Thread vs Subagent Split

> **As a** Claude Code user,
> **I want to** see my usage separated into "my direct work" vs "automated subagent work,"
> **so that** I understand where my tokens actually go.

**Acceptance criteria:**

- Default summary separates main thread (`isSidechain: false`) from subagent (`isSidechain: true`) (see Section 7: "Main vs Subagent" table)
- Shows percentage split (e.g., "34% direct / 66% subagent")
- `--main-only` flag to show only direct usage
- `--subagents-only` flag to show only subagent usage

**Implements:** Task 1.6 | **Persona:** Sarah

#### FR-04 (US-4): JSON Output

> **As a** developer building on top of this data,
> **I want** machine-readable JSON output,
> **so that** I can pipe the results into my own tools, scripts, or dashboards.

**Acceptance criteria:**

- `--json` flag outputs valid JSON to stdout (see Section 7: JSON Output Schema for complete schema)
- Same data as table output, structured as a JSON object
- No ANSI colors or formatting in JSON mode
- Stable schema (breaking changes require major version bump)

**Implements:** Task 1.8 | **Persona:** Priya

#### FR-05 (US-5): Correct Deduplication

> **As a** user who cares about accuracy,
> **I want** each API request counted exactly once,
> **so that** streaming chunks, log rehydration, and session restarts don't inflate my numbers.

**Acceptance criteria:**

- Deduplicate by `requestId` (primary) or `message.id` (fallback, 1:1 equivalent) (see Section 5: Streaming Chunks for worked example)
- Keep the entry with `stop_reason != null` (final chunk with real output_tokens)
- Entries without requestId or message.id: count once, flag in `--verbose`
- `--verbose` shows dedup statistics: raw lines → unique requests, dedup ratio (see Section 7: Verbose Output)

**Implements:** Task 1.4 | **Persona:** Priya

### P1 — Should Have (Phase 2)

#### US-6: Time-Scoped Views

> **As a** daily user,
> **I want to** see my usage for today, yesterday, or over a date range,
> **so that** I can track trends and understand daily consumption patterns.

**Acceptance criteria:**

- `cc-metrics today` — today's usage
- `cc-metrics yesterday` — yesterday's usage
- `cc-metrics daily` — last 7 days, one row per day
- `cc-metrics daily --days 30` — configurable lookback
- `--since` and `--until` date filters work with all views

#### US-7: Per-Model Breakdown

> **As a** user who switches between models,
> **I want to** see usage and cost broken down by model,
> **so that** I understand which models cost more and how I use them differently.

**Acceptance criteria:**

- `cc-metrics model` shows table with one row per model
- Columns: model name, requests, input, output, cache_read, cache_write, estimated cost
- Sorted by cost descending

#### US-8: Per-Project Breakdown

> **As a** user working across multiple projects,
> **I want to** see which projects consume the most tokens,
> **so that** I can identify heavy projects and optimize my workflow.

**Acceptance criteria:**

- `cc-metrics project` shows table with one row per project
- Project name derived from directory path (human-readable, not the encoded form)
- Columns: project name, sessions, requests, tokens (in+out), estimated cost
- Sorted by cost descending

#### US-9: Session Drill-Down

> **As a** user investigating a specific session,
> **I want to** drill into a session's usage details,
> **so that** I can understand what happened in that conversation.

**Acceptance criteria:**

- `cc-metrics session` lists recent sessions with summary stats
- `cc-metrics session <id>` shows detailed breakdown for one session
- Shows: start time, duration, model(s) used, token breakdown, cost, request count
- Links to subagent files if any

#### US-10: Pricing Modifier Support

> **As a** user who uses fast mode, data residency, or long context,
> **I want** these pricing modifiers reflected in my cost estimates,
> **so that** the numbers are actually accurate for my usage pattern.

**Acceptance criteria:**

- Fast mode (6x) applied when `speed: "fast"` detected
- Data residency (1.1x) applied when `inference_geo` indicates US-only
- Long context (2x input, 1.5x output) applied when input > 200k tokens (Sonnet models only)
- `--verbose` shows which modifiers were applied and their impact

#### US-15: Explain Mode — Show Your Work (FR-06)

> **As a** skeptical user who has seen different numbers from different tools,
> **I want** cc-metrics to walk me through its methodology on my own data,
> **so that** I can verify the dedup logic myself and trust the output.
>
> *Persona: Sarah (IC engineer) — "I've been burned by wrong numbers. Show me why yours are right."*
> *Drives: FR-06 → Section 7 (Explain Output) + Section 12 (Competitive Differentiation)*

**Acceptance criteria:**

- `cc-metrics explain` picks one real multi-chunk streaming request from the user's data
- Shows the raw JSONL chunks: requestId, stop_reason, output_tokens for each chunk
- Highlights which chunk was kept (final, with stop_reason) and which were discarded
- Shows the pricing calculation step-by-step for that request (base rate × modifiers)
- Shows what other tools would report for that same request (first-seen-wins, no-dedup, sum-all)
- Explains cache tier distinction with one real example: "this 5m write costs $X, this 1h write costs $Y"
- If `--json` is also passed, outputs the explanation as structured JSON

**Why this matters for launch:**
- Users don't trust claims — they trust reproducible evidence from their own data
- This is the "show your work" that turns a blog post claim into verifiable proof
- It's the feature that makes the HN post compelling: "run this yourself and see"

### P2 — Nice to Have (Phase 3)

#### US-11: Cache Efficiency Metrics

> **As a** user optimizing my Claude Code setup,
> **I want to** see cache hit rate and efficiency trends,
> **so that** I can understand if my caching is working well and how to improve it.

**Acceptance criteria:**

- Shows cache efficiency ratio: `cache_read / (cache_read + input + cache_creation)`
- Trends over time (daily cache efficiency)
- Comparison: "your caching saved you \$X vs no-cache pricing"

#### US-12: Cross-Reference with /stats

> **As a** user who wants to verify accuracy,
> **I want to** compare our numbers against `/stats`'s stats-cache.json,
> **so that** I can see where the numbers agree and where they diverge.

**Acceptance criteria:**

- `cc-metrics verify` reads `~/.claude/stats-cache.json`
- Compares input+output totals between our dedup and /stats
- Shows discrepancies with explanations (e.g., "ongoing sessions not yet finalized")
- Pass/fail verdict with tolerance margin

#### US-13: Server Tool Use Costs

> **As a** user who uses web search and code execution,
> **I want** these costs included in my estimates,
> **so that** I see the complete picture.

**Acceptance criteria:**

- Web search count and cost (\$10/1k searches) shown
- Web fetch count shown (free, token cost only)
- Included in total cost estimate

#### US-14: Model Filter

> **As a** user who wants to analyze specific model usage,
> **I want to** filter results to a specific model,
> **so that** I can focus on Opus vs Sonnet vs Haiku usage separately.

**Acceptance criteria:**

- `--model opus` filters to all Opus variants
- `--model sonnet-4-5` filters to specific model
- Works with all views and subcommands

### Explicitly Out of Scope

| Feature                  | Why excluded                                           |
| ------------------------ | ------------------------------------------------------ |
| TUI dashboard            | Scope creep — ship CLI first, add TUI if demand exists |
| Real-time monitoring     | Different use case — Usage Monitor already does this   |
| MCP server mode          | Phase 4 at earliest — validate core first              |
| Web dashboard            | Way out of scope for a CLI tool                        |
| Team/multi-user features | Single-user tool by design                             |
| Conversation viewer      | Not our job — claudelytics does this                   |
| Multi-currency           | Premature — add if users request                       |
| Rate limit prediction    | Requires undocumented Anthropic rate limit data        |

---

## 5. Data Model & JSONL Schema

### File Structure

```
~/.claude/
  projects/
    -Users-name-project/                  # one dir per project (path-encoded with dashes)
      <session-uuid>.jsonl                # main conversation log
      <session-uuid>/subagents/
        agent-<id>.jsonl                  # subagent conversations (identical schema)
  stats-cache.json                        # /stats data (for verify command)
```

### JSONL Line Types

Each `.jsonl` file contains one JSON object per line. Only `type: "assistant"` lines carry usage data.

| `type` value            | Contains usage? | Notes                                |
| ----------------------- | --------------- | ------------------------------------ |
| `assistant`             | Yes             | Model responses with `message.usage` |
| `user`                  | No              | User messages                        |
| `progress`              | No              | Tool execution progress              |
| `file-history-snapshot` | No              | File state snapshots                 |

### Usage Object Schema (on `type: "assistant"` lines)

```jsonc
{
  "type": "assistant", // FILTER: only process these
  "message": {
    "id": "msg_01ABC...", // message-level ID (dedup fallback key)
    "model": "claude-opus-4-6", // model identifier → pricing table lookup
    "stop_reason": "end_turn", // NON-NULL = final chunk (keep this one)
    // null = intermediate chunk (discard)
    "usage": {
      "input_tokens": 241, // new context tokens (full input rate)
      "output_tokens": 168, // generated response (full output rate)
      "cache_creation_input_tokens": 492, // sum of 5m + 1h below
      "cache_read_input_tokens": 49336, // cached context reuse (0.1x input rate)
      "cache_creation": {
        "ephemeral_5m_input_tokens": 0, // 5-min cache write (1.25x input rate)
        "ephemeral_1h_input_tokens": 492, // 1-hr cache write (2.0x input rate)
      },
      "service_tier": "standard", // "standard" or "enterprise"
      "speed": "standard", // "standard" or "fast" (fast = 6x pricing)
      "inference_geo": "", // "" or "us" (us = 1.1x pricing, Opus 4.6+)
      "server_tool_use": {
        "web_search_requests": 0, // \$10 per 1,000 searches
        "web_fetch_requests": 0, // free (token cost only)
      },
    },
  },
  "requestId": "req_01XYZ...", // PRIMARY DEDUP KEY — shared across all
  // streaming chunks for one API request
  "uuid": "unique-per-line", // DO NOT USE FOR DEDUP — unique per chunk
  "sessionId": "session-uuid", // maps to session file name
  "isSidechain": false, // false = main thread, true = subagent
}
```

### Streaming Chunks: Why Dedup Matters

A single API response produces 2-6+ JSONL lines (one per content block: thinking, text, tool_use). All share the same `requestId`.

**Example: one API request → 3 JSONL lines:**

| Line         | `uuid`    | `requestId` | `stop_reason` | `output_tokens`  |
| ------------ | --------- | ----------- | ------------- | ---------------- |
| 1 (thinking) | `aaa-111` | `req_01XYZ` | `null`        | 8 (placeholder)  |
| 2 (text)     | `bbb-222` | `req_01XYZ` | `null`        | 11 (placeholder) |
| 3 (tool_use) | `ccc-333` | `req_01XYZ` | `end_turn`    | 168 (real value) |

**Correct approach:** Group by `requestId`, keep the line where `stop_reason != null`. This gives us 1 entry with `output_tokens: 168`.

**Wrong approaches:**

- No dedup → 3 entries, 187 output_tokens (3.3x overcounted, wrong values)
- Dedup by `uuid` → 3 entries (uuid is unique per line, so no dedup happens)
- Dedup by `requestId`, first-seen → 1 entry, but `output_tokens: 8` (placeholder)

---

## 6. Architecture & Data Flow

### Pipeline

```
┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
│ Scanner  │───►│ Parser   │───►│  Dedup   │───►│ Pricing  │───►│ Analysis │───►│ Output   │
│          │    │          │    │          │    │          │    │          │    │          │
│ glob     │    │ filter   │    │ group by │    │ per-model│    │ aggregate│    │ table    │
│ **/*.jsonl│    │ type=    │    │ requestId│    │ per-type │    │ by model,│    │ (default)│
│ recursive│    │ assistant │    │ keep last│    │ per-      │    │ project, │    │ json     │
│ includes │    │ extract  │    │ chunk    │    │ modifier │    │ day,     │    │ (--json) │
│ subagents│    │ usage    │    │ (stop_   │    │ calculate│    │ session  │    │          │
│          │    │ + meta   │    │ reason)  │    │          │    │ main vs  │    │          │
│          │    │          │    │          │    │          │    │ subagent │    │          │
└──────────┘    └──────────┘    └──────────┘    └──────────┘    └──────────┘    └──────────┘
  glob crate     serde_json      HashMap         PRICING.md      iterators       comfy-table
  rayon           line-by-line    <requestId,     embedded        + rayon         serde_json
                                  UsageEntry>     in binary
```

### Internal Data Model (Rust)

```rust
/// A single deduplicated API request with its usage data
struct UsageEntry {
    request_id: String,
    session_id: String,
    model: String,            // e.g. "claude-opus-4-6"
    is_sidechain: bool,       // true = subagent, false = main thread
    timestamp: DateTime<Utc>, // from JSONL line metadata
    input_tokens: u64,
    output_tokens: u64,
    cache_read_input_tokens: u64,
    cache_write_5m_tokens: u64,   // ephemeral_5m_input_tokens
    cache_write_1h_tokens: u64,   // ephemeral_1h_input_tokens
    speed: Speed,                 // Standard | Fast
    inference_geo: Option<String>,// "" or "us"
    web_search_requests: u32,
    web_fetch_requests: u32,
    source_file: PathBuf,         // which JSONL file this came from
    project_path: String,         // decoded project directory
}

enum Speed { Standard, Fast }
```

### Error Handling

| Scenario                             | Behavior                                                            |
| ------------------------------------ | ------------------------------------------------------------------- |
| Malformed JSONL line                 | Skip line, increment `skipped_lines` counter (shown in `--verbose`) |
| Missing `requestId` AND `message.id` | Count once (no dedup possible), flag in `--verbose`                 |
| Unknown model ID                     | Use `\$0` pricing, warn in `--verbose`                              |
| File permission error                | Skip file, warn to stderr                                           |
| `<synthetic>` model messages         | Exclude from counts, flag in `--verbose`                            |
| Empty JSONL file                     | Skip silently                                                       |

---

## 7. Output Specifications

### Default Table Output (`cc-metrics`)

```
cc-metrics v0.1.0 — 77 days, 298 sessions, 10 projects

 Token Breakdown                     Count          %     API-Equiv Cost
─────────────────────────────────────────────────────────────────────────
 Input tokens                    1,140,000       0.04%         \$  5.70
 Output tokens                   6,120,000       0.21%         \$153.00
 Cache read                  2,730,000,000      95.80%         \$ 13.65
 Cache write (5m)               35,080,000       1.23%         \$  2.19
 Cache write (1h)               83,800,000       2.94%         \$  8.38
─────────────────────────────────────────────────────────────────────────
 Total                       2,856,140,000     100.00%         \$182.92

 Main vs Subagent         Requests    In+Out Tokens     Cost
──────────────────────────────────────────────────────────────
 Main thread (34%)          14,555        2,490,000    \$ 62.18
 Subagents (66%)            16,191        4,770,000    \$120.74

 Dedup: 87,684 raw lines → 30,746 unique requests (2.85x reduction)
 Pricing: Anthropic rates as of 2026-03-22 (embedded, no network)
```

### JSON Output Schema (`cc-metrics --json`)

```json
{
  "version": "0.1.0",
  "generated_at": "2026-03-22T14:30:00Z",
  "data_range": {
    "first_session": "2026-01-04T09:15:00Z",
    "last_session": "2026-03-22T13:45:00Z",
    "days": 77,
    "sessions": 298,
    "projects": 10
  },
  "dedup": {
    "raw_lines": 87684,
    "unique_requests": 30746,
    "skipped_lines": 12,
    "ratio": 2.85
  },
  "tokens": {
    "input": 1140000,
    "output": 6120000,
    "cache_read": 2730000000,
    "cache_write_5m": 35080000,
    "cache_write_1h": 83800000
  },
  "cost": {
    "total": 182.92,
    "by_type": {
      "input": 5.7,
      "output": 153.0,
      "cache_read": 13.65,
      "cache_write_5m": 2.19,
      "cache_write_1h": 8.38
    },
    "currency": "USD",
    "pricing_date": "2026-03-22",
    "note": "API-equivalent cost at published Anthropic rates"
  },
  "split": {
    "main": {
      "requests": 14555,
      "input_output_tokens": 2490000,
      "cost": 62.18
    },
    "subagent": {
      "requests": 16191,
      "input_output_tokens": 4770000,
      "cost": 120.74
    }
  }
}
```

### Pricing Modifier Composition

Modifiers are **multiplicative** and apply to the base per-model rates. They stack independently.

**Worked example:** An Opus 4.6 request with `speed: "fast"` and `inference_geo: "us"`:

```
Base rates (Opus 4.6):  input = \$5.00/M,  output = \$25.00/M

Modifiers:
  fast mode:       6.0x  (applies to all token types including cache)
  data residency:  1.1x  (applies to all token types including cache)
  combined:        6.0 × 1.1 = 6.6x

Effective rates:   input = \$33.00/M,  output = \$165.00/M
                   cache_read = \$3.30/M,  cache_write_5m = \$41.25/M,  cache_write_1h = \$66.00/M

For 1,000 input + 500 output + 10,000 cache_read:
  input:      1,000 × \$33.00 / 1M  = \$0.033
  output:       500 × \$165.00 / 1M = \$0.083
  cache_read: 10,000 × \$3.30 / 1M  = \$0.033
  total:                              \$0.149
```

**Long context** (Sonnet only, >200k input) applies BEFORE fast/residency modifiers:

- Input base × 2.0 → then × fast × residency
- Output base × 1.5 → then × fast × residency

**Fast mode** applies to cache tokens (cache_read, cache_write_5m, cache_write_1h) — the 6x multiplier covers all token types.

### Explain Output (`cc-metrics explain`)

```
cc-metrics v0.1.0 — Methodology Walkthrough

━━━ STEP 1: Streaming Chunk Deduplication ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

One API request → multiple JSONL lines. Here's a real example from YOUR data:

  requestId: req_011CZGxNTvMEdTTiFAoP64X6
  5 JSONL lines found (1 API request):

  Line   Content Type   stop_reason   output_tokens
  ────   ────────────   ───────────   ─────────────
  1      thinking       null          10 ← placeholder
  2      text           null          10 ← placeholder
  3      tool_use       null          10 ← placeholder
  4      tool_use       null          10 ← placeholder
  5      tool_use       tool_use      365 ← REAL VALUE ✓

  cc-metrics keeps:  line 5 (stop_reason = "tool_use") → 365 output tokens
  ccusage keeps:     line 1 (first-seen-wins)          → 10 output tokens
  claudelytics:      all 5 lines (no dedup)            → 405 output tokens

  Impact: ccusage undercounts by 97.3%. claudelytics overcounts by 11%.

━━━ STEP 2: Pricing Calculation ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

For this request (model: claude-opus-4-6, speed: standard):

  Token Type            Count      Rate ($/M)    Cost
  ──────────            ─────      ──────────    ────
  Input                 3          $5.00         $0.000015
  Output                365        $25.00        $0.009125
  Cache read            0          $0.50         $0.000000
  Cache write (1h)      10,000     $10.00        $0.100000

  Modifiers: none (standard speed, no geo, <200k input)
  Request total: $0.109140

━━━ STEP 3: Cache Tier Distinction ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Your data uses TWO cache tiers with DIFFERENT pricing:

  Main thread:   1h cache writes (ephemeral_1h) at $10.00/M
  Subagents:     5m cache writes (ephemeral_5m) at $6.25/M

  In your data: 86.3M tokens at 1h rate = $862.55
                37.3M tokens at 5m rate = $233.00
  vs single-rate: 123.5M tokens at $10.00/M = $1,235.26 (42% overcounted)

  Other tools lump both tiers together → wrong cost.

━━━ STEP 4: Your Aggregate Numbers ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  Tool           Output Tokens    Total Cost    Methodology
  ────           ─────────────    ──────────    ───────────
  cc-metrics     8,625,351        $2,376        Final chunk, 5-type split, modifiers
  ccusage        2,975,552        $2,032        First-seen-wins (placeholder tokens)
  claudelytics   12,750,257       $17,703       No dedup (all chunks counted)
```

### Verbose Output Additions (`--verbose`)

Appended after the default table:

```
 Verbose Details
──────────────────────────────────────────────────────────────
 Files scanned:     1,337 (169 main + 1,168 subagent)
 Skipped lines:     12 (malformed JSON)
 No-ID entries:     3 (counted once, not deduplicated)
 Synthetic msgs:    65 (excluded, model="<synthetic>")
 Unknown models:    0

 Pricing Modifiers Applied
──────────────────────────────────────────────────────────────
 Fast mode (6x):    0 requests
 Data residency:    0 requests
 Long context:      0 requests
```

---

## 8. Non-Functional Requirements

### Performance

| Metric                  | Target        | Notes |
| ----------------------- | ------------- | ----- |
| Parse time              | Scales with dataset | ~6s for 1,420 files on Apple M4 Max |
| Binary size             | < 5 MB        | |
| Memory usage            | < 50 MB peak  | |
| Network requests        | Zero (always) | |

### Correctness

- Dedup must match Python reference script within 0.1% (floating point tolerance only)
- Cost calculations must use Anthropic's published rates exactly
- All numbers must be reproducible: same input → same output, always

### Privacy

- No network requests, ever
- No telemetry, analytics, or crash reporting
- No data leaves the user's machine
- No state files written (parse from source on every run)

### Distribution

- Single static binary for macOS (arm64, x86_64), Linux (x86_64, arm64), Windows (x86_64)
- `cargo install cc-metrics` for Rust users
- GitHub Releases with pre-built binaries
- Homebrew formula (stretch goal)

---

## 9. Risk Assessment

| #   | Risk                                                                                                                                                | Probability   | Impact | Mitigation                                                                                                                                 |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------- | ------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------ |
| R1  | **Anthropic builds native analytics** — HN thread already has Anthropic team member responding. `/context` exists but is buggy.                     | High (6-12mo) | Fatal  | Ship fast, build community, publish methodology. Native tools will likely be simpler; our depth is the moat.                               |
| R2  | **JSONL format changes** — format has changed 5+ times in 10 months (costUSD removed, thinking blocks added, subagents added, cache tiers added)    | High          | High   | Dedup by `requestId` is resilient (API-level, not format-level). Fall back to `message.id`. Keep parser tolerant of unknown fields.        |
| R3  | **ccusage fixes their bugs** — Issues #888, #899, #313 are all open. If fixed, our primary differentiation weakens.                                 | Medium        | High   | Our value isn't "ccusage is buggy" — it's methodology transparency and disaggregated output. Blog post establishes credibility regardless. |
| R4  | **0.1% tolerance unachievable** — floating point, timezone edge cases, or undiscovered JSONL quirks could prevent exact match with reference script | Medium        | Medium | Define tolerance as "within 0.1% on token counts, within \$0.01 on cost per request." Document known divergence sources.                   |
| R5  | **Pricing table staleness** — Anthropic changes rates between our releases                                                                          | High          | Medium | Pricing embedded at compile time from `docs/PRICING.md`. `--pricing-file` flag for user override. Document "pricing as of" date in output. |
| R6  | **Compacted/cleared sessions** — `/compact` and `/clear` modify JSONL structure in ways we haven't investigated                                     | Medium        | Medium | Test against compacted session files. If structure is incompatible, document limitation and skip gracefully.                               |

---

## 10. Dependencies

### External (not controlled by us)

| Dependency                                | Type              | Risk                    | Notes                                                              |
| ----------------------------------------- | ----------------- | ----------------------- | ------------------------------------------------------------------ |
| JSONL file format (`~/.claude/projects/`) | Data source       | Format instability (R2) | No documented schema; reverse-engineered from observation          |
| Anthropic pricing page                    | Pricing reference | Rate changes (R5)       | Manually verified, embedded at compile time                        |
| `requestId` field stability               | Dedup correctness | Field removal (R2)      | API-level identifier, unlikely to change; `message.id` as fallback |
| `stats-cache.json` format                 | Verify command    | Format changes          | Only used by optional `verify` subcommand (P2)                     |

### Internal

| Dependency              | Type             | Notes                                                                  |
| ----------------------- | ---------------- | ---------------------------------------------------------------------- |
| `docs/PRICING.md`       | Build input      | Source of truth for embedded pricing table                             |
| `docs/ARCHITECTURE.md`  | Design reference | Module layout and CLI interface spec                                   |
| Python reference script | Validation       | `tests/reference/dedup.py` — ground truth for correctness verification |

### Build Dependencies

See `docs/ARCHITECTURE.md` for full `Cargo.toml`. Key crates: `clap` 4, `serde` 1, `serde_json` 1, `rayon` 1, `chrono` 0.4, `glob` 0.3, `comfy-table` 7.

MSRV: Rust 1.75+ (for stable async traits, though we don't use async).

---

## 11. Success Metrics

### P0 Metrics (product success — must hit)

| Metric                                                                                        | Baseline     | Target                         | By      | How measured                                                                                            |
| --------------------------------------------------------------------------------------------- | ------------ | ------------------------------ | ------- | ------------------------------------------------------------------------------------------------------- |
| **Correctness** — dedup output matches Python reference script                                | 0% validated | Within 0.1% on all token types | Launch  | Automated test in CI comparing `cc-metrics --json` vs `tests/reference/dedup.py` output on fixture data |
| **Adoption** — GitHub stars                                                                   | 0            | 100+                           | Month 1 | GitHub API / repo insights                                                                              |
| **Trust** — independent verifications (users confirming numbers match their own calculations) | 0            | 3+                             | Month 1 | Count GitHub issues/comments/tweets where users report verification. Track in `evaluations/launch/`     |
| **Zero disputed methodology** — no open issues challenging our math                           | N/A          | 0 open disputes                | Month 3 | GitHub issues labeled `methodology`                                                                     |

### P1 Metrics (growth signals — track but don't gate on)

| Metric                                                                                                    | Target | By      | How measured                      |
| --------------------------------------------------------------------------------------------------------- | ------ | ------- | --------------------------------- |
| Community mentions — referenced in HN/Reddit Claude Code discussions                                      | 1+     | Month 3 | Manual tracking via search alerts |
| Community contribution — external PR or issue with fix                                                    | 1+     | Month 3 | GitHub contributor count          |
| Competing tool acknowledgment — ccusage/claudelytics user publicly confirms our numbers are more accurate | 1+     | Month 3 | GitHub/HN/Reddit mentions         |

### Launch Checklist (tasks, not metrics)

- [ ] README with compelling screenshot showing disaggregated output
- [ ] Blog post: "I found out my Claude Code usage tools were lying to me"
- [ ] HN post: "Show HN: cc-metrics — honest token metrics for Claude Code"

### Failure Criteria

If by Month 3: fewer than 50 stars AND zero independent verifications → reassess positioning. The tool may be correct but not compelling. Pivot options: (a) contribute methodology to ccusage instead of competing, (b) reposition as a library/crate rather than CLI, (c) add MCP server mode to reach users inside Claude Code.

---

## 12. Competitive Differentiation

| Dimension              | ccusage (11.8k stars)                             | claudelytics (70)      | ccost (6)                         | **cc-metrics**              |
| ---------------------- | ------------------------------------------------- | ---------------------- | --------------------------------- | --------------------------- |
| Dedup strategy         | `message.id:requestId` (correct key, wrong chunk) | None                   | `message.id+requestId` (correct)  | `requestId`, last-seen-wins |
| Output token accuracy  | ~5x undercount (first-seen-wins)                  | Overcounted (no dedup) | Unknown (pre-thinking-blocks era) | Correct (final chunk)       |
| Cache type split       | No (single category)                              | No                     | No                                | Yes (5m vs 1h)              |
| Fast mode pricing      | Yes (added Mar 2026)                              | No                     | No                                | Yes                         |
| Data residency pricing | No                                                | No                     | No                                | Yes                         |
| Long context pricing   | Yes                                               | No                     | No                                | Yes                         |
| Subagent scanning      | No (Issue #313, open)                             | Yes (recursive)        | No                                | Yes                         |
| Sidechain filtering    | No                                                | No                     | No                                | Yes                         |
| Server tool costs      | No                                                | No                     | No                                | Yes                         |
| Runtime dependency     | Node.js                                           | None (Rust)            | None (Rust)                       | None (Rust)                 |
| Published methodology  | No                                                | No                     | Partial                           | Yes (in README + blog)      |
| State/DB               | None                                              | None                   | SQLite                            | None                        |

**Our unfair advantage:** We have a published research study proving every other tool wrong, with source code verification. The tool is the proof.

---

## 13. Phased Delivery

### Phase 1: MVP

Ship `cc-metrics` with disaggregated summary, correct dedup, cache-aware costs, main/subagent split, and `--json`.

**Exit criteria:** `cc-metrics` produces correct, disaggregated output. Numbers match Python reference script within 0.1%.

**Task decomposition (build order):**

| #    | Task                                                                                         | Depends on    | Delivers                          |
| ---- | -------------------------------------------------------------------------------------------- | ------------- | --------------------------------- |
| 1.1  | **Scaffold** — `cargo init`, `Cargo.toml` with deps, module stubs, clap CLI skeleton         | —             | Compiling binary with `--help`    |
| 1.2  | **Scanner** — recursive glob of `~/.claude/projects/**/*.jsonl`, including `subagents/` dirs | 1.1           | `Vec<PathBuf>` of all JSONL files |
| 1.3  | **Parser** — read JSONL lines, filter `type: "assistant"`, extract `UsageEntry` structs      | 1.1           | `Vec<RawEntry>` per file          |
| 1.4  | **Dedup engine** — group by `requestId`, keep entry with `stop_reason != null`               | 1.3           | `Vec<UsageEntry>` (deduplicated)  |
| 1.5  | **Pricing engine** — embed `PRICING.md` rates, calculate per-model per-type costs            | 1.4           | `CostResult` per entry            |
| 1.6  | **Analysis** — aggregate tokens by type, split main vs subagent, compute totals              | 1.4, 1.5      | `Summary` struct                  |
| 1.7  | **Table output** — format `Summary` as terminal table using comfy-table                      | 1.6           | Default CLI output                |
| 1.8  | **JSON output** — serialize `Summary` as JSON matching schema in Section 7                   | 1.6           | `--json` flag works               |
| 1.9  | **Test fixtures** — create sample JSONL files exercising all edge cases                      | 1.3           | `tests/fixtures/*.jsonl`          |
| 1.10 | **Integration tests** — dedup correctness, pricing correctness, scanner correctness          | 1.4, 1.5, 1.2 | `cargo test` passes               |
| 1.11 | **Reference validation** — compare output against Python reference script on real data       | 1.8           | Numbers match within 0.1%         |

### Phase 2: Views

Add time and dimension slicing subcommands.

**Exit criteria:** Users can slice usage by time, model, project, and session.

**Task decomposition:**

| #   | Task                                                                                           | Depends on | Delivers                     |
| --- | ---------------------------------------------------------------------------------------------- | ---------- | ---------------------------- |
| 2.1 | **Timestamp extraction** — parse timestamps from JSONL entries, attach to `UsageEntry`         | Phase 1    | Date-aware entries           |
| 2.2 | **Date filters** — `--since`, `--until` flags filtering `UsageEntry` by date                   | 2.1        | Filtered datasets            |
| 2.3 | **Today/yesterday subcommands** — syntactic sugar over date filters                            | 2.2        | `cc-metrics today`           |
| 2.4 | **Daily breakdown** — group by day, one row per day                                            | 2.1        | `cc-metrics daily`           |
| 2.5 | **Model breakdown** — group by `model` field                                                   | Phase 1    | `cc-metrics model`           |
| 2.6 | **Project breakdown** — group by project path (decoded)                                        | Phase 1    | `cc-metrics project`         |
| 2.7 | **Session drill-down** — list sessions or show single session detail                           | Phase 1    | `cc-metrics session [id]`    |
| 2.8 | **Model filter** — `--model` flag with fuzzy matching (e.g., `opus` matches all Opus variants) | Phase 1    | `--model opus` works         |
| 2.9 | **Pricing modifiers** — detect `speed: "fast"`, `inference_geo: "us"`, long context            | Phase 1    | Correct modifier application |
| 2.10 | **CI pipeline** — GitHub Actions workflow: lint, test, build on push/PR (ubuntu + macOS)      | Phase 1    | Automated quality gate       |

### Phase 3: Polish & Launch

Validation, documentation, and launch.

**Exit criteria:** Tool is launch-ready with compelling README, methodology docs, and HN post.

**Task decomposition:**

| #   | Task                                                                                     | Depends on | Delivers                     |
| --- | ---------------------------------------------------------------------------------------- | ---------- | ---------------------------- |
| 3.1 | **Verbose mode** — `--verbose` flag showing dedup stats, file counts, modifiers applied  | Phase 1    | Verbose output per Section 7 |
| 3.2 | **Verify command** — compare against `stats-cache.json`, show discrepancies              | Phase 1    | `cc-metrics verify`          |
| 3.3 | **Cache efficiency** — compute cache hit ratio, savings vs no-cache                      | Phase 1    | Cache metrics in output      |
| 3.4 | **Server tool costs** — web search/fetch counts and costs                                | Phase 1    | Complete cost picture        |
| 3.5 | **Error resilience** — handle malformed JSONL, permission errors, empty files gracefully | Phase 1    | No panics on bad input       |
| 3.6 | **Explain mode** — `cc-metrics explain` walks through dedup + pricing on user's own data (FR-06) | Phase 1 | Trust-building proof output |
| 3.7 | **README** — screenshot, installation, methodology, comparison table                     | 3.5, 3.6  | Compelling README.md         |
| 3.8 | **Cross-platform builds** — GitHub Actions for macOS/Linux/Windows binaries              | 3.5        | Release artifacts            |
| 3.9 | **Blog post + HN submission**                                                            | 3.7        | Launch                       |

---

## 14. Open Questions

| #   | Question                                                                                                                           | Owner    | Resolve by                 | Blocks                       |
| --- | ---------------------------------------------------------------------------------------------------------------------------------- | -------- | -------------------------- | ---------------------------- |
| Q1  | **Name:** `cctrue`? `cc-metrics`? `ccreal`? `cchonest`? Check crates.io/GitHub availability.                                       | Ishpreet | Before Task 1.1 (scaffold) | Phase 1 start                |
| Q2  | **What does /stats actually count?** Its 9.4M doesn't match any dedup strategy. Investigate `stats-cache.json` update logic.       | Ishpreet | Before Task 3.2 (verify)   | Phase 3 only                 |
| Q3  | **Compacted sessions:** How does `/compact` or `/clear` affect JSONL structure? Test with compacted files.                         | Ishpreet | During Task 1.9 (fixtures) | Risk R6 mitigation           |
| Q4  | **`<synthetic>` model messages:** 65 found with model="<synthetic>". Determine origin. Current decision: exclude, flag in verbose. | Ishpreet | During Task 1.3 (parser)   | Non-blocking (decision made) |
| Q5  | **Naming the "three numbers":** "Unique Content", "Session Workload", "Raw Log Volume" — validate with 2-3 users.                  | Ishpreet | Before Task 3.6 (README)   | Phase 3 only                 |
| Q6  | **Subscription framing:** Cost labeled "API-equivalent value." Current decision: yes. Validate in blog draft.                      | Ishpreet | Before Task 3.8 (blog)     | Phase 3 only                 |

---

## 15. Glossary

| Term                        | Definition                                                                                                                                       |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| **requestId**               | API-level identifier shared by all streaming chunks from a single Anthropic API request. Primary dedup key.                                      |
| **uuid**                    | Per-JSONL-line unique identifier. NOT a valid dedup key — each streaming chunk gets its own uuid.                                                |
| **message.id**              | Message-level identifier (`msg_01...`). 1:1 equivalent to `requestId` in all observed data. Fallback dedup key.                                  |
| **stop_reason**             | Set to `"end_turn"` or `"tool_use"` on the final streaming chunk. `null` on intermediate chunks. Indicates which chunk has real `output_tokens`. |
| **streaming chunk**         | A single JSONL line within a multi-line response. One API request → 2-6+ chunks (thinking, text, tool_use).                                      |
| **sidechain / isSidechain** | Boolean field on JSONL entries. `true` = message from a subagent (spawned by the Agent tool). `false` = main conversation thread.                |
| **subagent**                | A Claude Code subprocess spawned by the Agent tool for parallel tasks. Writes to `subagents/` directories. All entries have `isSidechain: true`. |
| **cache_read_input_tokens** | Tokens reused from prompt cache. Cost: 0.1x input rate. Typically 95%+ of all tokens.                                                            |
| **cache_write_5m**          | Tokens written to 5-minute ephemeral cache. Cost: 1.25x input rate. Field: `cache_creation.ephemeral_5m_input_tokens`.                           |
| **cache_write_1h**          | Tokens written to 1-hour ephemeral cache. Cost: 2.0x input rate. Field: `cache_creation.ephemeral_1h_input_tokens`.                              |
| **fast mode**               | Opus 4.6 speed option. All token rates multiplied by 6x. Detected via `speed: "fast"` in usage object.                                           |
| **data residency**          | US-only inference option. All rates multiplied by 1.1x. Detected via `inference_geo: "us"`.                                                      |
| **long context**            | Triggered when input exceeds 200k tokens on Sonnet models. Input 2x, output 1.5x.                                                                |
| **phantom cost**            | The gap between naive cost calculation (all tokens at input rate) and correct cache-aware calculation. Typically 5.7x-29x overcounting.          |
| **dedup ratio**             | Raw JSONL lines / unique API requests. Measures streaming chunk inflation. Our data: 2.85x.                                                      |
| **API-equivalent cost**     | What the usage would cost at Anthropic's published API rates. For subscription users, this represents value received, not amount owed.           |
