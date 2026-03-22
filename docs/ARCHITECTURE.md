# Architecture

## Language: Rust

### Why Rust
- Single binary, zero runtime dependencies (ccusage requires Node.js)
- Fast parallel parsing with rayon (claudelytics proves this works at scale)
- Both ccost and claudelytics chose Rust for the same reasons
- Good CLI ecosystem (clap for args, comfy-table for output)
- Cross-platform binaries (macOS, Linux, Windows)

### Why not alternatives
- **Python:** Fast to prototype but requires runtime. Could prototype here first, port later.
- **TypeScript/Bun:** Matches Claude Code's stack but requires runtime.
- **Go:** Viable but Rust has better precedent in this space.

## Directory Structure

```
cc-metrics/
  docs/                          # Project documentation
    VISION.md                    # Product vision
    ARCHITECTURE.md              # This file
    PRICING.md                   # Embedded pricing reference
  src/
    main.rs                      # CLI entry point (clap)
    lib.rs                       # Library root
    scanner/
      mod.rs                     # File discovery
      glob.rs                    # Recursive JSONL glob (~/.claude/projects/**/*.jsonl)
    parser/
      mod.rs                     # JSONL line parsing
      message.rs                 # Message type definitions and deserialization
      usage.rs                   # Usage object extraction
    dedup/
      mod.rs                     # Deduplication engine
      request_id.rs              # requestId-based dedup (primary)
      strategy.rs                # Dedup strategy trait (extensible)
    pricing/
      mod.rs                     # Cost calculation engine
      models.rs                  # Embedded per-model pricing table
      modifiers.rs               # Fast mode (6x), data residency (1.1x), long context (2x)
      cache.rs                   # 5m vs 1h cache write distinction
    analysis/
      mod.rs                     # Aggregation and breakdown logic
      summary.rs                 # Overall summary stats
      by_model.rs                # Per-model breakdown
      by_project.rs              # Per-project breakdown
      by_day.rs                  # Daily breakdown
      by_session.rs              # Per-session drill-down
      sidechain.rs               # Main thread vs subagent separation
    output/
      mod.rs                     # Output formatting
      table.rs                   # Pretty terminal tables
      json.rs                    # --json output
  tests/
    fixtures/                    # Sample JSONL files for testing
      simple_session.jsonl       # One response, no streaming chunks
      streaming_session.jsonl    # Multiple chunks per response
      subagent_session.jsonl     # Subagent with isSidechain: true
      mixed_models.jsonl         # Multiple models in one session
    integration/
      dedup_test.rs              # Verify dedup correctness
      pricing_test.rs            # Verify cost calculations
      scanner_test.rs            # Verify file discovery
  Cargo.toml
  README.md
```

## Data Flow

```
Scanner                    Parser                  Dedup                  Analysis              Output
  │                          │                       │                      │                     │
  ├─ glob **/*.jsonl  ──►    ├─ parse JSONL    ──►   ├─ group by       ──►  ├─ aggregate    ──►   ├─ table
  │  (recursive,             │  lines                │  requestId           │  by model,          │  (default)
  │   includes               ├─ filter to            ├─ keep final         │  project,           ├─ json
  │   subagents/)            │  type=assistant        │  chunk              │  day,               │  (--json)
  │                          ├─ extract              │  (stop_reason       │  session             │
  │                          │  usage object         │   != null)          ├─ separate            │
  │                          ├─ extract              │                     │  main vs             │
  │                          │  metadata             │                     │  subagent            │
  │                          │  (model, sidechain,   │                     ├─ calculate           │
  │                          │   speed, geo)         │                     │  costs               │
  │                          │                       │                     │  (per-model,         │
  │                          │                       │                     │   per-type)           │
```

## CLI Interface (planned)

```bash
# Default: overall summary
cc-metrics

# Time-scoped views
cc-metrics today
cc-metrics yesterday
cc-metrics daily                  # last 7 days
cc-metrics daily --days 30

# Breakdown views
cc-metrics model                  # per-model breakdown
cc-metrics project                # per-project breakdown
cc-metrics session [id]           # drill into a session

# Filters
cc-metrics --model opus           # filter to specific model
cc-metrics --since 2026-03-01     # date range
cc-metrics --until 2026-03-15
cc-metrics --main-only            # exclude subagent usage
cc-metrics --subagents-only       # only subagent usage

# Output
cc-metrics --json                 # machine-readable
cc-metrics --verbose              # include cache breakdown details

# Meta
cc-metrics --version
cc-metrics --help
cc-metrics verify                 # run self-check against stats-cache.json
```

## Key Design Decisions

### Embedded pricing table
Pricing is compiled into the binary from `docs/PRICING.md`, not fetched from an API. Avoids:
- Network dependency
- LiteLLM pricing inaccuracy (ccusage #4 showed 81.9% match rate)
- Privacy concerns (no outbound requests)

Updated with each release. Users can override with `--pricing-file` if needed.

### Dedup: requestId, last-seen-wins
- Primary key: `requestId`
- Fallback: `message.id` (1:1 equivalent in all observed data)
- Strategy: keep the entry with `stop_reason != null` (final chunk with real output_tokens)
- Entries without requestId or message.id: count once, flag in verbose output

### No database
Parse JSONL on every run. No SQLite, no cache files, no state.
- Simpler mental model
- No stale cache bugs
- 1,337 files parse in <1 second with rayon
- If performance becomes an issue, add optional caching later

### Cross-reference with stats-cache.json
The `verify` command compares our numbers against `/stats`'s stats-cache.json to validate:
- Our input + output total should approximate stats-cache's total (within margin from ongoing sessions)
- Discrepancies flagged with explanations

## Dependencies (minimal)

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rayon = "1"
chrono = { version = "0.4", features = ["serde"] }
glob = "0.3"
comfy-table = "7"

[dev-dependencies]
tempfile = "3"
```

## Performance Targets

| Metric | Target |
|---|---|
| Parse 1,337 JSONL files | < 1 second |
| Binary size | < 5 MB |
| Memory usage | < 50 MB peak |
| Zero network requests | Always (embedded pricing) |
