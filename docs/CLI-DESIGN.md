# CLI Design: ccmetrics

**Status:** Draft — open for discussion
**Date:** 2026-03-22
**Context:** Phase 2 planning. First time we're formally designing the CLI UX.

---

## Design Principles

1. **One screen answers 80% of questions** — the default view should be a complete dashboard
2. **Subcommands only for structurally different output** — not for filtered versions of the same data
3. **Flags compose with every view** — `--since 7d --model opus` works everywhere
4. **Less surface area = more adoption** — ccusage (6 subcommands, 11.8k stars) vs par_cc_usage (18+ subcommands, 84 stars)

---

## Proposed CLI Surface

### Subcommands (4 total)

```
ccmetrics                    # Dashboard — the one view most users need
ccmetrics daily              # Timeline — one row per day
ccmetrics session [id]       # Sessions — list all, or drill into one
ccmetrics explain            # Methodology — show your work (already built)
```

### Global Flags

```
Filters (compose with any view):
  --since <date>             # 2026-03-01, 7d, 30d, today
  --until <date>             # End date
  --model <pattern>          # Fuzzy match: opus, sonnet, haiku
  --project <pattern>        # Filter by project name

Output:
  --json                     # JSON format (already built)
  --verbose                  # Extra stats (already built)
  --path <dir>               # Custom data path (already built)
```

### What Was Cut (and Why)

| PRD Proposal | Why cut | Alternative |
|---|---|---|
| `ccmetrics today` | Syntactic sugar | `ccmetrics --since today` |
| `ccmetrics yesterday` | Syntactic sugar | `ccmetrics --since 1d --until today` |
| `ccmetrics model` | Same data as default view | By Model table already in dashboard |
| `ccmetrics project` | Same grouping pattern | By Project table added to dashboard |

---

## View Designs

### 1. Default Dashboard (`ccmetrics`)

The primary view. Should answer: "How much have I used, what does it cost, where is it going?"

```
ccmetrics v0.1.0 — 77 days, 298 sessions, 10 projects

┌─────────────────┬───────────────┬────────┬─────────┐
│ Token Breakdown │         Count │      % │    Cost │
├─────────────────┼───────────────┼────────┼─────────┤
│ Input tokens    │     1,140,000 │  0.04% │   $5.70 │
│ Output tokens   │     6,120,000 │  0.21% │ $153.00 │
│ Cache read      │ 2,730,000,000 │ 95.80% │   $1.37 │
│ Cache write 5m  │    59,440,000 │  2.10% │   $0.37 │
│ Cache write 1h  │    59,440,000 │  2.10% │   $0.59 │
│ Total           │ 2,856,140,000 │   100% │ $161.03 │
└─────────────────┴───────────────┴────────┴─────────┘

┌───────────────────┬──────────┬──────────────┬─────────┐
│ Main vs Subagent  │ Requests │ In+Out Toks  │    Cost │
├───────────────────┼──────────┼──────────────┼─────────┤
│ Main thread  (47%)│   14,450 │    3,421,200 │  $75.66 │
│ Subagents    (53%)│   16,296 │    3,838,800 │  $85.37 │
└───────────────────┴──────────┴──────────────┴─────────┘

┌───────────────────┬──────────┬──────────────┬─────────┐
│ By Model          │ Requests │ In+Out Toks  │    Cost │
├───────────────────┼──────────┼──────────────┼─────────┤
│ claude-sonnet-4-6 │   25,100 │    5,800,000 │ $102.00 │
│ claude-opus-4-6   │    4,200 │    1,200,000 │  $48.00 │
│ claude-haiku-4-5  │    1,446 │      260,000 │  $11.03 │
└───────────────────┴──────────┴──────────────┴─────────┘

┌───────────────────┬──────────┬──────────────┬─────────┐
│ By Project        │ Sessions │ In+Out Toks  │    Cost │
├───────────────────┼──────────┼──────────────┼─────────┤
│ cc-metrics        │       42 │    2,100,000 │  $52.00 │
│ my-saas-app       │      180 │    3,500,000 │  $71.00 │
│ dotfiles          │       76 │    1,660,000 │  $38.03 │
└───────────────────┴──────────┴──────────────┴─────────┘

Dedup: 87,684 → 30,746 unique requests (2.85x reduction)
Pricing: Anthropic rates as of 2026-03-22 (embedded, no network)
```

**With filters applied:**
```
$ ccmetrics --since 7d --model opus

ccmetrics v0.1.0 — 7 days, 12 sessions, 3 projects (filtered)
Filter: since 2026-03-15, model matches "opus"

[Same table structure, filtered data]
```

### 2. Daily Timeline (`ccmetrics daily`)

Answers: "What's my usage trend over time?"

```
$ ccmetrics daily

ccmetrics v0.1.0 — Daily breakdown (last 7 days)

┌────────────┬──────────┬──────────────┬─────────┐
│ Date       │ Requests │ In+Out Toks  │    Cost │
├────────────┼──────────┼──────────────┼─────────┤
│ 2026-03-22 │      847 │      412,300 │  $12.45 │
│ 2026-03-21 │    1,203 │      580,100 │  $18.20 │
│ 2026-03-20 │      956 │      445,800 │  $14.33 │
│ 2026-03-19 │      312 │      167,400 │   $5.10 │
│ 2026-03-18 │    1,445 │      723,500 │  $22.80 │
│ 2026-03-17 │      678 │      334,200 │  $10.55 │
│ 2026-03-16 │      445 │      210,600 │   $6.70 │
├────────────┼──────────┼──────────────┼─────────┤
│ Total      │    5,886 │    2,873,900 │  $90.13 │
└────────────┴──────────┴──────────────┴─────────┘

Avg: 841 requests/day, $12.88/day
```

**With lookback:**
```
$ ccmetrics daily --since 30d
```

### 3. Session List & Drill-Down (`ccmetrics session`)

Answers: "What happened in my sessions?" and "What did this specific session cost?"

**List mode** (no argument):
```
$ ccmetrics session

ccmetrics v0.1.0 — Recent sessions

┌──────────────┬────────────┬──────────┬───────────────────┬─────────┐
│ Session ID   │ Date       │ Requests │ Model             │    Cost │
├──────────────┼────────────┼──────────┼───────────────────┼─────────┤
│ a3f8c2d1...  │ 2026-03-22 │       42 │ claude-opus-4-6   │   $4.20 │
│ b7e1d4a9...  │ 2026-03-22 │       18 │ claude-sonnet-4-6 │   $1.35 │
│ c9f2b5e3...  │ 2026-03-21 │      156 │ claude-opus-4-6   │  $18.90 │
│ ...          │            │          │                   │         │
└──────────────┴────────────┴──────────┴───────────────────┴─────────┘

Showing 20 most recent. Use --since to see more.
```

**Drill-down mode** (with session ID, partial match):
```
$ ccmetrics session a3f8c2d1

Session a3f8c2d1... — 2026-03-22, project: cc-metrics

[Token Breakdown table — same format as default, scoped to this session]
[By Model table — if multiple models used in session]

42 requests over 2h 15m. 3 subagent spawns.
```

### 4. Explain (`ccmetrics explain`)

Already built. No changes needed.

---

## Composition Examples

Filters compose with every view:

```bash
# Last week's usage
ccmetrics --since 7d

# Opus usage across all time
ccmetrics --model opus

# Daily breakdown for one project
ccmetrics daily --project cc-metrics

# Sessions from today using Sonnet
ccmetrics session --since today --model sonnet

# JSON output for scripting
ccmetrics --since 30d --json

# Everything together
ccmetrics daily --since 30d --model opus --project cc-metrics --json
```

---

## Help Output

```
$ ccmetrics --help

Honest token metrics for Claude Code

Usage: ccmetrics [OPTIONS] [COMMAND]

Commands:
  daily    Daily breakdown — one row per day
  session  List sessions, or drill into one by ID
  explain  Walk through the methodology on your data

Options:
      --since <DATE>      Filter from date (2026-03-01, 7d, 30d, today)
      --until <DATE>      Filter until date
      --model <PATTERN>   Filter by model (fuzzy: opus, sonnet, haiku)
      --project <PATTERN> Filter by project name
      --json              Output as JSON
  -v, --verbose           Show dedup stats, file counts, modifiers
      --path <DIR>        Path to Claude projects [default: ~/.claude/projects]
  -h, --help              Print help
  -V, --version           Print version

Run 'ccmetrics' with no arguments for a full dashboard.
```

---

## Date Parsing

The `--since` and `--until` flags accept:

| Format | Example | Meaning |
|---|---|---|
| ISO date | `2026-03-01` | Specific date |
| Relative days | `7d` | 7 days ago |
| Relative weeks | `2w` | 14 days ago |
| Keyword | `today` | Start of today |

---

## JSON Schema (for `--json`)

Each view produces JSON. The default view schema extends the current output with `by_project`:

```json
{
  "version": "0.1.0",
  "filter": {
    "since": "2026-03-15T00:00:00Z",
    "until": null,
    "model": "opus",
    "project": null
  },
  "data_range": { ... },
  "dedup": { ... },
  "tokens": { ... },
  "cost": { ... },
  "split": { ... },
  "by_model": [ ... ],
  "by_project": [
    {
      "project": "cc-metrics",
      "sessions": 42,
      "requests": 1200,
      "input_tokens": 500000,
      "output_tokens": 200000,
      "cost": 52.00
    }
  ]
}
```

Daily view JSON:
```json
{
  "version": "0.1.0",
  "filter": { ... },
  "daily": [
    {
      "date": "2026-03-22",
      "requests": 847,
      "input_tokens": 200000,
      "output_tokens": 212300,
      "cost": 12.45
    }
  ]
}
```

---

## Open Design Questions

- [ ] Should `ccmetrics daily` default to 7 days or 30 days?
- [ ] Should session IDs be truncated in list view? How many chars?
- [ ] Should `--model` support multiple patterns? (`--model opus --model sonnet`)
- [ ] Should the By Project table show in default view always, or only when 2+ projects (like By Model)?
- [ ] Should filters show a "filter active" indicator in the output header?
- [ ] Color and visual hierarchy — deferred to UX design pass

---

## Competitive Comparison

| Feature | ccmetrics (proposed) | ccusage | tokscale |
|---|---|---|---|
| Subcommands | 4 | 6 | 10+ |
| Time filters | `--since`/`--until` | `--since`/`--until` | `--since`/`--until`/`--today`/`--week`/`--month` |
| Model filter | `--model` (fuzzy) | `--breakdown` | `--group-by model` |
| Project filter | `--project` | `--instances` | N/A |
| Default output | Full dashboard | Daily table | Interactive TUI |
| JSON output | All views | All views | `--json` flag |

---

*This document is a living design spec. Update as decisions are made.*
