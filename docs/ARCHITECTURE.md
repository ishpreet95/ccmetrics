# Architecture

## Language: Rust

### Why Rust
- Single binary, zero runtime dependencies
- Fast parallel parsing with rayon
- Good CLI ecosystem (clap for args, comfy-table for tables)
- Cross-platform binaries (macOS, Linux, Windows)

## Directory Structure

```
ccmetrics/
  docs/
    ARCHITECTURE.md              # This file
    PRD.md                       # Product requirements (v1.3)
    PRICING.md                   # Embedded pricing reference
    dashboard.png                # Dashboard screenshot
  src/
    main.rs                      # CLI entry point (clap), subcommand dispatch
    types.rs                     # Core types: Summary, CostBreakdown, ModelBreakdown, etc.
    scanner.rs                   # File discovery (~/.claude/projects/**/*.jsonl)
    parser.rs                    # JSONL line parsing, usage/metadata extraction
    dedup.rs                     # Streaming chunk dedup (requestId, last-seen-wins)
    filters.rs                   # Date/model/project filters with date parsing
    analysis.rs                  # Aggregation: by-model, by-project, daily, session
    pricing.rs                   # Embedded per-model pricing table with modifiers
    pipeline.rs                  # Streaming pipeline orchestrator (scan → parse → dedup → filter → calculate)
    explain.rs                   # Methodology walkthrough data collection
    output/
      mod.rs                     # Output module root
      table.rs                   # Boreal Command dashboard (4-section layout)
      style.rs                   # ANSI styling: chip, hero, accent, dim, bars, insights
      json.rs                    # --json output (stable machine-readable contract)
      daily.rs                   # Daily breakdown table (comfy-table)
      session.rs                 # Session list + detail views (comfy-table)
      explain.rs                 # Explain mode terminal rendering
  tests/
    fixtures/
      simple_session.jsonl       # One response, no streaming
      streaming_session.jsonl    # Multiple chunks per response
      subagent_session.jsonl     # Subagent with isSidechain: true
      synthetic_and_edge.jsonl   # Synthetic messages, malformed lines
    integration_test.rs          # 28 integration tests
  Cargo.toml
  README.md
```

## Data Flow

```
Scanner          Parser            Dedup             Filters           Analysis          Output
  │                │                 │                 │                 │                 │
  ├─ glob        ► ├─ parse JSONL  ► ├─ group by    ► ├─ date range  ► ├─ aggregate    ► ├─ table
  │  **/*.jsonl    │  lines          │  requestId     │  --since/      │  by model,      │  (dashboard)
  │  (recursive,   ├─ filter to      ├─ keep final   │  --until       │  project,       ├─ json
  │   includes     │  assistant      │  chunk         ├─ model match  │  day, session   │  (--json)
  │   subagents/)  ├─ extract        │  (stop_reason  ├─ project      ├─ separate       ├─ daily
  │                │  usage +        │   != null)     │  match        │  main vs        ├─ session
  │                │  metadata       │                │               │  subagent       ├─ explain
  │                │                 │                │               ├─ calculate      │
  │                │                 │                │               │  costs          │
  │                │                 │                │               │  (per-model)    │
```

## Pipeline

The streaming pipeline (`pipeline.rs`) orchestrates the data flow with real-time progress output to stderr:

```
✔ Found 1420 files (176 main + 1244 subagent)     (85ms)
✔ 101412 assistant entries, 2 skipped, 74 excluded (5569ms)
✔ 37152 unique requests (2.7x reduction)           (248ms)
✔ $2879.75 total (5 models, 5 token types)         (84ms)
```

*Example from 54 days of usage on Apple M4 Max, 36GB RAM. Timings will vary by machine and dataset size.*

Each step reports timing and key stats. Suppressed with `--quiet`.

## Dashboard Design: Boreal Command

The default output (`output/table.rs` + `output/style.rs`) renders a 4-section terminal dashboard:

```
⊡ YOUR WORK  →  Deduplicated token totals, input/output breakdown
¤ COST        →  API-equivalent cost estimate, proportional bars
↻ CACHE       →  Cache efficiency, savings, read/write volume
⊕ WHERE IT GOES → Main/subagent split, by-model, by-project
```

### Visual elements
- **Chip heroes** — accent background + dark text for key numbers (`style::chip`)
- **Section rules** — full terminal width, dim `───` with accent title + semantic icon
- **Proportional bars** — `████░░░░` showing relative proportions
- **Insights** — colored severity dots (green/yellow/red) with contextual messages
- **Right-aligned columns** — consistent 2 decimal places for vertical scanning
- **Unicode symbols** — `◇ ◆ ▪` for subheader differentiation

### Color palette (ANSI 256)
| Role | Code | Color |
|------|------|-------|
| Accent (heroes, bars) | 151 | Sage-mint `#afd7af` |
| Values (data) | 254 | Near-white `#e4e4e4` |
| Labels, rules | DIM | Dimmed default |
| Descriptions | DIM+ITALIC | Subtle italic |
| Chip background | 48;5;151 | Sage-mint bg |
| Chip text | 38;5;16 | True black |

## CLI Interface

```bash
ccmetrics                        # Boreal Command dashboard
ccmetrics daily                  # Daily breakdown (comfy-table)
ccmetrics session                # List 20 most recent sessions
ccmetrics session <id>           # Drill into a session by ID (prefix match)
ccmetrics explain                # Methodology walkthrough on your data

# Filters (work with all subcommands)
ccmetrics --since 7d             # Last 7 days (also: 2w, 30d, today, ISO dates)
ccmetrics --until 2026-03-15     # Up to a date
ccmetrics --model opus           # Filter by model (substring, case-insensitive)
ccmetrics --project myapp        # Filter by project name

# Output
ccmetrics --json                 # Machine-readable JSON
ccmetrics --verbose              # Detailed stats
ccmetrics --quiet                # Suppress pipeline progress
```

## Key Design Decisions

### Embedded pricing table
Pricing is compiled into the binary from `docs/PRICING.md`. No network dependency, no LiteLLM inaccuracy, no privacy concerns. Updated with each release.

### Dedup: requestId, last-seen-wins
- Primary key: `requestId`
- Strategy: keep the entry with `stop_reason != null` (final chunk with real output_tokens)
- Entries without requestId: count once, flag in verbose output

### No database
Parse JSONL on every run. No SQLite, no cache files, no state. Simpler mental model, no stale cache bugs.

### NO_COLOR support
All styling functions accept a `color: bool` parameter. When `NO_COLOR` is set or stdout is not a terminal, all ANSI codes are suppressed. Chip heroes fall back to `[ value ]` bracket notation.

## Dependencies

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
comfy-table = "7"
glob = "0.3"

[dev-dependencies]
tempfile = "3"
```

## Performance

Benchmarked on Apple M4 Max, 36GB RAM, 54 days of Claude Code usage (1,420 JSONL files, 101K entries):

| Metric | Measured |
|---|---|
| Full pipeline (1,420 files) | ~6 seconds |
| 110 tests | < 1 second |
| Binary size (release) | ~4 MB |
| Zero network requests | Always |

*Timings scale with dataset size. A few days of usage processes in under a second.*
