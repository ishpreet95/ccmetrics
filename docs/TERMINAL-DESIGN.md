# Terminal Output Design: ccmetrics

**Status:** Draft — decisions captured, ready for implementation spec
**Date:** 2026-03-22
**Research:** BM25 design intelligence + cross-tool UX analysis (htop, infracost, starship, k9s, duf, stripe CLI)

---

## Design Philosophy

1. **Show the work as it happens** — stream each processing step, building trust through transparency
2. **Numbers first** — hero metrics prominent, differentiated by color/spacing
3. **Context, not just data** — tell the user what's good, what's notable
4. **Concern-based grouping** — organize by user question, not data schema
5. **Progressive disclosure** — glanceable summary → detailed tables → verbose debug
6. **Honest about estimates** — flag what's exact vs approximated
7. **Restraint on visuals** — bars only where proportions matter, no decoration for decoration's sake

---

## Research Findings Applied

| Pattern | Source | How we use it |
|---------|--------|---------------|
| Hero numbers at top | Analytics dashboards, btop | Total cost + total tokens prominent |
| "Symbol WHAT, Color WHETHER" | Starship prompt | Indicators like `●` green/yellow |
| Concern-based sections | gh-dash, duf | Group by "cost", "tokens", "activity" |
| Right-align numbers, left-align labels | Infracost, every financial tool | All numeric columns right-aligned |
| Threshold-based coloring | duf, k9s | Cache efficiency green if >90% |
| Faint structural characters | Infracost tree lines | Dim borders, bold data |
| Be honest about unknowns | Infracost | "API-equivalent estimate" label |
| NO_COLOR fallback | clig.dev standard | Text prefixes when color unavailable |

---

## Design Decisions (locked)

Based on user input, 2026-03-22:

| Decision | Choice | Rationale |
|---|---|---|
| Layout style | **Option A** — concern-based sections | Clear separation, hero numbers prominent |
| Hero numbers | Differentiated by **color + spacing**, not box | Terminal boxes are fragile across widths |
| Token format | **Abbreviated** (2.86B, 6.1M) as default | Exact in `--verbose` and `--json` |
| Bars | **Only where proportions matter** | Cost breakdown yes, model split no |
| Streaming pipeline | **Yes — show processing steps** | Core differentiator, builds trust |
| Subcommands | Use to **separate structurally different views** | Keeps each view clean and focused |

---

## Streaming Pipeline Output

**The signature feature.** Before showing results, ccmetrics streams each processing step as it runs — like watching an AI stream its response, but for data processing. This is transparency as UX.

### Why this matters

ccmetrics' entire value proposition is "we get the math right." Showing the pipeline steps proves there's no magic — just careful parsing of your local data. Every step explains what happened and why.

### Full experience (default mode)

```
  ⠋ Scanning ~/.claude/projects/ ...
  ✓ Found 1,337 files (170 main + 1,167 subagent)

  ⠋ Parsing JSONL entries ...
  ✓ 87,684 assistant entries (42 skipped, 65 synthetic excluded)

  ⠋ Deduplicating by requestId ...
  ✓ 30,746 unique requests (2.85x reduction — streaming chunks removed)

  ⠋ Calculating costs (3 models, 5 token types) ...
  ✓ $2,184.03 total across 10 projects

  ────────────────────────────────────────────────────────────────

  [Dashboard output appears here]
```

### How it works technically

Each step corresponds to a real code phase in `main.rs`:

```
Step 1: scanner::scan_jsonl_files()     → file count, main/sub split
Step 2: parser::parse_jsonl_file()      → entry count, skipped, synthetic
Step 3: dedup::deduplicate()            → unique count, reduction ratio
Step 4: analysis::analyze()             → cost total, model count
```

The spinner (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`) animates while each step runs. When it completes, the spinner is replaced with `✓` and the result summary appears. Each step prints in real-time as it finishes — no buffering.

### Behavior variants

| Context | Pipeline behavior |
|---|---|
| Default (`ccmetrics`) | Full streaming pipeline + dashboard |
| With `--json` | Pipeline suppressed (clean JSON only) |
| Piped (`ccmetrics \| jq`) | Pipeline suppressed (detect `!is_terminal()`) |
| With `--quiet` or `-q` | Pipeline suppressed, dashboard only |
| With `--verbose` | Pipeline + dashboard + extra detail |
| Subcommands (`daily`, `session`) | Same pipeline, then subcommand view |

### Color in pipeline

```
  ⠋  ← cyan (spinner)
  ✓  ← green (success)
  ✗  ← red (if a step had warnings, e.g., many skipped lines)

  Step text: default color
  Numbers in results: bold (1,337 files, 87,684 entries, etc.)
  Reduction note: dim ("streaming chunks removed")
```

### NO_COLOR fallback

```
  ... Scanning ~/.claude/projects/
  [ok] Found 1,337 files (170 main + 1,167 subagent)

  ... Parsing JSONL entries
  [ok] 87,684 assistant entries (42 skipped, 65 synthetic excluded)

  ... Deduplicating by requestId
  [ok] 30,746 unique requests (2.85x reduction)

  ... Calculating costs (3 models, 5 token types)
  [ok] $2,184.03 total across 10 projects
```

### Why this is a differentiator

No other Claude Code usage tool does this. They silently process and dump a table. ccmetrics shows you:
- What data it found (you can verify the file count)
- What it filtered (skipped lines, synthetic messages)
- How much dedup mattered (2.85x — the core insight)
- What it calculated (model count, total cost)

This turns "trust me, here are numbers" into "watch me derive these numbers from your data."

---

## Proposed Layout: Default Dashboard

### Option A: Hero Numbers + Concern Sections

```
  ccmetrics v0.1.0                           77 days · 298 sessions · 10 projects

  ┌─ COST ────────────────────────────────────────────────────────────────────┐
  │                                                                          │
  │   $2,184.03                        API-equivalent estimate               │
  │                                                                          │
  │   Input        $5.70    ░░          Cache read     $1.37    ░             │
  │   Output     $153.00    ████████    Cache 5m       $0.37    ░             │
  │   Web search   $0.10    ░           Cache 1h       $0.59    ░             │
  │                                                                          │
  │   ● 93% of cost is output tokens                                        │
  │   ● Cache saves you ~$10,303 vs uncached pricing                        │
  └──────────────────────────────────────────────────────────────────────────┘

  ┌─ TOKENS ──────────────────────────────────────────────────────────────────┐
  │                                                                          │
  │   2.86B total                      30,746 unique API requests            │
  │                                                                          │
  │   Cache read   2,730,000,000  ██████████████████████████████████  95.8%  │
  │   Cache write    118,880,000  █░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░   4.2%  │
  │   Output           6,120,000  ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░   0.2%  │
  │   Input            1,140,000  ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░   0.04% │
  │                                                                          │
  │   ● 95.8% cache efficiency — your context is being reused well          │
  │   ● 2.85x dedup reduction (87,684 raw → 30,746 unique)                 │
  └──────────────────────────────────────────────────────────────────────────┘

  ┌─ WHERE IT GOES ───────────────────────────────────────────────────────────┐
  │                                                                          │
  │   By thread                     By model                                │
  │   Main      47%   $1,026.49     sonnet-4-6    $1,402.00    64%          │
  │   Subagent  53%   $1,157.54     opus-4-6        $648.00    30%          │
  │                                 haiku-4-5       $134.03     6%          │
  │                                                                          │
  │   By project                                                            │
  │   my-saas-app      $980.00   ████████████████░░░░  45%                  │
  │   cc-metrics       $652.00   ██████████░░░░░░░░░░  30%                  │
  │   dotfiles         $552.03   ████████░░░░░░░░░░░░  25%                  │
  └──────────────────────────────────────────────────────────────────────────┘

  Methodology: run 'ccmetrics explain' to see how these numbers are calculated
```

### Option B: Compact Hero Strip + Clean Tables

```
  ccmetrics v0.1.0 — 77 days, 298 sessions

  $2,184      2.86B tokens      30,746 requests      95.8% cache
  ─────────── ──────────────── ─────────────────── ──────────────
  total cost   total volume     unique (deduped)     efficiency ●

  COST BREAKDOWN                                        % of total
  ─────────────────────────────────────────────────────────────────
  Output tokens            $153.00  ██████████████████████   93.1%
  Input tokens               $5.70  ██░░░░░░░░░░░░░░░░░░░    3.5%
  Cache read                 $1.37  ░░░░░░░░░░░░░░░░░░░░░    0.8%
  Cache write (1h)           $0.59  ░░░░░░░░░░░░░░░░░░░░░    0.4%
  Cache write (5m)           $0.37  ░░░░░░░░░░░░░░░░░░░░░    0.2%
  Web search                 $0.10  ░░░░░░░░░░░░░░░░░░░░░    0.1%

  MODEL SPLIT                    Requests    In+Out      Cost
  ─────────────────────────────────────────────────────────────────
  claude-sonnet-4-6              25,100    5.8M    $1,402.00
  claude-opus-4-6                 4,200    1.2M      $648.00
  claude-haiku-4-5                1,446    260K      $134.03

  THREAD SPLIT                   Requests    In+Out      Cost
  ─────────────────────────────────────────────────────────────────
  Main thread        47%         14,450    3.4M    $1,026.49
  Subagents          53%         16,296    3.8M    $1,157.54

  ──────────────────────────────────────────────────────────────────
  ● cache efficiency 95.8% — excellent, context is being reused
  ● dedup: 87,684 raw lines → 30,746 unique requests (2.85x)
  ● pricing: Anthropic rates 2026-03-22 (embedded, no network)
```

### Option C: Minimal Hero + Insight Callouts

```
  ╭──────────────────────────────────────────────────╮
  │  $2,184.03 total cost    2.86B tokens processed  │
  │  77 days · 298 sessions · 10 projects            │
  ╰──────────────────────────────────────────────────╯

  Cost                              Tokens
  ────                              ──────
  Output     $153.00  (93%)         Cache read   2.73B  (95.8%)
  Input        $5.70  ( 3%)         Cache write  119M   ( 4.2%)
  Cache read   $1.37  ( 1%)         Output       6.1M   ( 0.2%)
  Cache 1h     $0.59                Input        1.1M   ( 0.04%)
  Cache 5m     $0.37
  Web search   $0.10

  ▸ 95.8% of tokens are cache reads — your setup is efficient
  ▸ Output tokens drive 93% of cost despite being 0.2% of volume
  ▸ Subagents account for 53% of requests (automated background work)

  Models                    Requests    Cost
  ──────                    ────────    ────
  sonnet-4-6                 25,100    $1,402
  opus-4-6                    4,200      $648
  haiku-4-5                   1,446      $134

  Projects                  Sessions    Cost
  ────────                  ────────    ────
  my-saas-app                   180      $980
  cc-metrics                     42      $652
  dotfiles                       76      $552

  dedup: 87,684 → 30,746 (2.85x) · run 'ccmetrics explain' for methodology
```

---

## Contextual Insights

The key differentiator from plain tables. After each section, surface **1-2 insights** that tell the user what the numbers mean:

| Data Point | Insight | Why it matters |
|---|---|---|
| Cache read 95.8% | "Your setup is efficient — context is being reused" | Users worry about waste |
| Output = 93% of cost | "Output tokens drive most of your cost despite being 0.2% of volume" | Non-obvious cost driver |
| Subagent 53% | "53% of requests are automated subagent work, not your direct conversations" | Sets expectations |
| Cache savings | "Cache saves ~$10,303 vs uncached pricing" | Shows value of caching |
| Dedup ratio | "2.85x reduction — raw logs overcount by nearly 3x" | Builds trust in methodology |

### Threshold-Based Context (future)

When we have date comparison data:
```
  ▸ Cost is up 23% vs last week ($18.40/day → $22.60/day)
  ▸ Output tokens increased — more complex responses this week
```

---

## Color System

### Semantic Colors (ANSI 256)

| Color | Meaning | When |
|---|---|---|
| **Bold white** | Hero numbers, totals | `$2,184.03`, `2.86B tokens` |
| **Cyan** | Section headers | `COST BREAKDOWN`, `MODEL SPLIT` |
| **Green** | Good / healthy | Cache efficiency >90%, cost down |
| **Yellow** | Notable / attention | Cache efficiency 70-90%, cost up |
| **Red** | Concerning | Cache efficiency <70% (rare) |
| **Dim/faint** | Structural, secondary | Borders, dedup footer, units |
| **Default** | Data values | Token counts, percentages |

### NO_COLOR Fallback

When `NO_COLOR` is set or output is piped:
- Replace `●` indicators with text: `[good]`, `[note]`, `[warn]`
- Replace `█░` bars with percentage numbers only
- Keep right-alignment and structure
- Drop all ANSI codes

```
  $2,184.03 total cost    2.86B tokens processed
  77 days, 298 sessions, 10 projects

  Cost Breakdown                                     % of total
  Output tokens            $153.00                       93.1%
  Input tokens               $5.70                        3.5%
  ...

  [good] cache efficiency 95.8%
  [note] output tokens drive 93% of cost
```

---

## Inline Visualizations

### Proportional Bars

Using block characters to show relative proportions:

```
█ = filled (primary color)
░ = empty (dim)

Output     $153.00  ██████████████████████░░░░░░░░  93%
Input        $5.70  █░░░░░░░░░░░░░░░░░░░░░░░░░░░░   3%
Cache read   $1.37  ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░   1%
```

Bar width adapts to terminal width. Minimum: 10 chars. Maximum: 30 chars.

### Sparklines (for daily view)

```
  Last 7 days: ▁▃▅██▃▂  $90.13 total
```

Using braille or block-eighth characters: `▁▂▃▄▅▆▇█`

---

## Number Formatting Rules

| Type | Format | Example |
|---|---|---|
| Currency | `$` prefix, 2 decimal places, comma-separated | `$2,184.03` |
| Large tokens | Abbreviated with suffix | `2.86B`, `6.1M`, `260K` |
| Exact tokens | Comma-separated (in verbose/JSON) | `2,856,140,000` |
| Percentages | 1 decimal place, `%` suffix | `95.8%` |
| Counts | Comma-separated | `30,746` |
| Dates | ISO short | `2026-03-22` |
| Duration | Human-readable | `77 days`, `2h 15m` |

### Abbreviation Thresholds

```
>= 1,000,000,000 → B (2.86B)
>= 1,000,000     → M (6.1M)
>= 1,000         → K (1.4K)
< 1,000          → exact (847)
```

---

## Daily View Design

```
  ccmetrics v0.1.0 — Daily (last 7 days)

  Date          Requests    In+Out      Cost     ▁▂▃▄▅▆▇█
  ──────────    ────────    ──────    ───────    ────────
  2026-03-22         847     412K     $12.45     ▃
  2026-03-21       1,203     580K     $18.20     █
  2026-03-20         956     446K     $14.33     ▅
  2026-03-19         312     167K      $5.10     ▁
  2026-03-18       1,445     724K     $22.80     █
  2026-03-17         678     334K     $10.55     ▃
  2026-03-16         445     211K      $6.70     ▂
  ──────────    ────────    ──────    ───────
  Total            5,886    2.87M     $90.13

  Avg: 841 requests/day · $12.88/day
  ▸ Busiest: 2026-03-18 (1,445 requests, $22.80)
```

---

## Session List Design

```
  ccmetrics v0.1.0 — Sessions (20 most recent)

  Session       Date          Requests    Model               Cost
  ───────       ────          ────────    ─────               ────
  a3f8c2d1…     2026-03-22         42    opus-4-6           $4.20
  b7e1d4a9…     2026-03-22         18    sonnet-4-6         $1.35
  c9f2b5e3…     2026-03-21        156    opus-4-6          $18.90
  d1a4c7f2…     2026-03-21         67    sonnet-4-6         $3.45
  ...

  Use 'ccmetrics session a3f8c2d1' for full breakdown
```

---

## Design Decisions: Resolved

- [x] Layout: **Option A** — concern-based sections with separation
- [x] Hero numbers: **Color + spacing** (not boxed)
- [x] Tokens: **Abbreviated** (2.86B) as default, exact in verbose/JSON
- [x] Bars: **Only where proportions matter** — cost breakdown yes, not everywhere
- [x] Streaming pipeline: **Yes** — core differentiator
- [x] Subcommands: **Use for structurally different views** (daily, session)

## Open Design Questions

- [ ] Insight callout marker: `▸` or `●` or `→`?
- [ ] Max number of projects/models before "and N more" truncation?
- [ ] Cache savings insight: show by default or only in verbose?
- [ ] Sparklines in daily view: worth the complexity?
- [ ] Pipeline spinner speed: 80ms or 120ms per frame?
- [ ] Should `--quiet` suppress pipeline only, or also trim dashboard?

---

## Implementation Notes

- Use `comfy-table` for table structure (already in deps)
- Use ANSI escape codes for color (no extra dependency needed)
- Check `NO_COLOR` env var and `!stdout.is_terminal()` for fallback
- Bars can be computed with: `bar_len = (value / max_value * bar_width).round()`
- Number abbreviation: simple function mapping to B/M/K suffixes
- Insights are computed from the Summary struct — no new data needed

---

*This document captures the design exploration. Decisions to be made with user input before implementation.*
