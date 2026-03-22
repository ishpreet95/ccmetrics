# Critical Review: "Understanding Claude Code Token Metrics"

**Blog URL:** https://ishpreet95.me/blog/understanding-claude-code-token-metrics
**Review Date:** March 22, 2026
**Review Method:** Multi-provider AI review (Claude Opus 4.6 + OpenAI Codex GPT-5.4), 2 rounds, adversarial second pass
**Reviewer:** Ishandeep (via Claude Octopus multi-LLM pipeline)

---

## Executive Summary

Your blog identifies a **real, well-documented problem** — Claude Code's JSONL token accounting is messy, and existing tools disagree wildly on the same data. The core analysis is sound, the GitHub references are verified, and the investigative depth is rare for a technical blog. However, the adversarial review uncovered **several significant gaps** that undermine the blog's credibility and completeness. Addressing these would elevate it from a good post to a definitive reference.

**Overall Score: 6.4/10** (down from 8.3 after adversarial scrutiny)

---

## Scores by Dimension

| Dimension           | Score | Notes                                                                |
| ------------------- | ----- | -------------------------------------------------------------------- |
| Technical Accuracy  | 7/10  | Core mechanisms correct; cost math unverifiable; parser has bugs     |
| Reference Integrity | 8/10  | All 6 GitHub issues verified real; but omits key alternative sources |
| Writing Quality     | 7/10  | Strong narrative hook; clear structure; some tone issues             |
| Actionable Value    | 5/10  | Parser is useful but buggy; omits the canonical solution (Usage API) |
| Layout/UX           | 6/10  | Wall of text; no charts/diagrams; tables need visualization          |
| SEO                 | 6/10  | Good title; missing meta description, JSON-LD, TOC                   |
| Credibility         | 6/10  | Good methodology disclosure; but undisclosed conflict of interest    |

---

## P0 Issues (Must Fix Before Promoting)

### 1. You Omit Anthropic's Official Usage & Cost API

This is the single biggest gap. Anthropic provides a **server-side Usage and Cost API** (`/v1/organizations/usage_report/messages`) that gives authoritative token counts without any JSONL parsing. The blog's entire premise — "you must parse JSONL files correctly to know your costs" — is incomplete without mentioning this.

For organizations on API billing, the Usage API completely bypasses the JSONL mess. The blog should either:

- Add a section: "If you're on the API plan, use the Usage & Cost API instead"
- Or explain why JSONL parsing is still necessary despite the API existing (e.g., Max plan subscribers don't have API billing access)

**References:**

- https://platform.claude.com/docs/en/build-with-claude/usage-cost-api
- https://code.claude.com/docs/en/costs

### 2. Cost Figure (\$2,184) Is Unverifiable

The blog presents \$2,184 as the "real cost" but doesn't disclose the **model mix**. Independent calculation using pure Sonnet pricing yields ~\$1,360 — a \$824 gap that can only be explained by Opus usage. Without a per-model token breakdown, readers cannot verify or reproduce this number.

**Fix:** Add a table showing token counts broken down by model (e.g., "Sonnet: X input, Y output, Z cache read; Opus: ..."). This is critical for a blog whose thesis is "here's how to count correctly."

### 3. "ccost Abandoned Since June 2025" Is Factually Wrong

The blog claims ccost (carlosarraes/ccost) was "abandoned since June 2025 with minimal community adoption." This is **demonstrably incorrect**:

- Repository has 82 commits
- v0.2.0 release with breaking changes and major features
- Active development with LiteLLM pricing integration, dual caching, enhanced deduplication

This is the blog's most significant factual error, and it unfairly dismisses the one tool that implemented requestId deduplication early. Please either correct or remove this claim.

**Source:** https://github.com/carlosarraes/ccost

### 4. Undisclosed Conflict of Interest

The "What's Next" section teases an unreleased competing tool with "correct deduplication and disaggregated metrics." The preceding narrative systematically makes every existing tool look inadequate — ccusage undercounts, claudelytics overcounts, ccost is "abandoned," /stats lacks detail.

This reads as building a case for your own tool launch. Even if unintentional, you should:

- Add a brief disclosure: "Full disclosure: I'm working on a tool in this space"
- OR separate the tool announcement from the analysis post entirely

---

## P1 Issues (Should Fix)

### 5. Python Parser Has Bugs

The 20-line parser presented as "correct" has several issues:

**a) Bare `except` clause:**

```python
except:
    continue
```

This catches `KeyboardInterrupt`, `SystemExit`, and `MemoryError`. Should be:

```python
except (json.JSONDecodeError, ValueError):
    continue
```

**b) Contradicts its own cited evidence:**
Issue #22686 (which you cite) reports that the final chunk with accurate output tokens is **never saved to JSONL** on some systems — only intermediate chunks with `output_tokens: 1` are recorded. If this is the case, your parser's "keep the entry where `stop_reason` is set" logic would find nothing and fall back to first-seen with `output_tokens: 1`.

The parser works for data where the final chunk IS written, but the blog should add a caveat acknowledging the #22686 edge case.

**c) Missing edge cases:**

- No explicit file encoding parameter (`open(f)` without `encoding='utf-8'`)
- Lines without `requestId` are silently dropped — no quantification of how many
- `glob.glob` follows symlinks by default — could cause issues
- All usage dicts held in memory — potential issue for very heavy users

### 6. Missing Alternative Tools

The blog covers ccusage, claudelytics, ccost, and /stats but omits several significant alternatives:

| Tool                                      | Why It Matters                                                                                                                         |
| ----------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| **cccost** (badlogic/cccost)              | Intercepts `fetch()` at the Node.js level — architecturally superior to JSONL parsing because it avoids streaming duplication entirely |
| **tokscale** (junhoyeo/tokscale)          | Multi-platform tracker (Claude Code, Codex, Gemini, Cursor) with real-time pricing                                                     |
| **par-cc-usage**                          | Python tool with cross-session deduplication using message + request IDs                                                               |
| **claude-code-usage-analyzer** (aarora79) | Uses ccusage + LiteLLM for per-model breakdowns                                                                                        |
| **goccc** (backstabslash/goccc)           | Go CLI with proper cache tier pricing                                                                                                  |

The omission of **cccost** is particularly notable — it solves the problem at the interception layer rather than trying to reconstruct accuracy from broken JSONL logs.

### 7. Add Methodology Section

The verification statement is buried in a footnote. For a research-oriented piece, methodology should be prominent:

**Suggested callout box at top:**

> **Methodology:** All analysis performed on local `~/.claude/` data from a single user over 77 active days (Jan 5 – Mar 22, 2026), 298 sessions, 10 projects. Models used: [list]. Tool versions: ccusage v18.0.10, claudelytics v0.5.2, ccost v0.2.0. Pricing verified against Anthropic's published rates on March 22, 2026.

Also add: "These ratios (95.8% cache, 87% subagent files, 2.85x inflation) reflect one power user's workflow with aggressive subagent usage. Your numbers will vary."

### 8. Add Visual Elements

The blog is a wall of text. These specific visualizations would dramatically improve it:

**a) Token breakdown donut chart:**

- Cache read: 95.8% (this IS the story — make it visual)
- Cache write: 4.2%
- Output: 0.21%
- Input: 0.04%

**b) Four-method comparison bar chart (log scale):**

- jq: 2.36M
- requestId dedup: 7.26M
- /stats: 9.4M
- claudelytics: 8.2B
  The 3,500x gap between tools is impossible to grasp in a table.

**c) Cost waterfall chart:**
\$2,184 (real) → +\$10,303 phantom costs → \$12,487 (naive) → +\$49,946 → \$62,433 (worst)
Show where the phantom costs come from.

**d) Streaming duplication flow diagram:**
1 API request → message_start → content_block (thinking) → content_block (text) → content_block (tool_use) → message_delta (final) = N JSONL lines

### 9. n=1 Sample Size Caveat

The ratios presented as findings are from one power user:

- 95.8% cache read — would be lower for users with shorter sessions
- 87% subagent files — heavily dependent on whether user has agents configured
- 2.85x inflation — depends on response complexity (more content blocks = more inflation)

Move this caveat from a buried footnote to a prominent callout near the data. Something like:

> "Your numbers will differ. A user running simple Q&A sessions without subagents would see much lower cache ratios and minimal subagent files. The mechanisms are universal; the ratios are personal."

---

## P2 Issues (Nice to Have)

### 10. Fix Author Byline

The byline appears to show "claude-code" as the author tag instead of your actual name. This makes it look AI-generated, which is ironic for a credibility-focused research piece.

### 11. Add SEO Fundamentals

- **Meta description** (missing): Suggested: "Four tools measured the same Claude Code usage data and returned four wildly different numbers. Here's what each one actually counts, and why they all disagree."
- **JSON-LD Article schema** for rich search snippets
- **Canonical URL** to prevent duplicate content issues
- **Table of contents** with anchor links (9-minute article with 7+ sections needs one)

### 12. Add Code Copy Button

For a developer audience, code blocks should have a copy-to-clipboard button. The 20-line Python parser is the blog's most shareable artifact.

### 13. Add Tool Comparison Matrix

A single reference table comparing what each tool gets right/wrong would be the most bookmarked element:

| Feature            | ccusage            | claudelytics    | /stats   | ccost               | cccost           |
| ------------------ | ------------------ | --------------- | -------- | ------------------- | ---------------- |
| Deduplication      | First-seen (buggy) | None            | N/A      | requestId (correct) | N/A (intercepts) |
| Cache separation   | Partial            | None            | No       | Yes                 | Yes              |
| Subagent scanning  | No (#313)          | Yes (recursive) | Unknown  | Yes                 | Yes              |
| Cost calculation   | Underestimates     | Massively over  | N/A      | Correct             | Correct          |
| Active maintenance | Yes (v18)          | Yes             | Built-in | Yes (v0.2)          | Yes              |

### 14. Add Call-to-Action

After 9 minutes of reading, give the reader something to do:

- "Try running the parser on your own data"
- "Star/watch the repo for the upcoming tool"
- "Discuss on [HN/Twitter/GitHub Discussions]"

### 15. Soften Tone on Specific Tools

The characterization of claudelytics ("Zero deduplication... sums all four token types") reads somewhat dismissive. Consider: "claudelytics prioritizes simplicity over granularity, summing all token types without deduplication — useful for relative trend analysis, but misleading for absolute cost estimation."

### 16. Verify Appendix Link

The blog references a "full technical appendix" at `/blog/claude-code-token-metrics-appendix`. Verify this URL resolves. Dead links undermine the credibility you've worked hard to build.

### 17. Mobile Table Handling

Wide tables (especially the token breakdown) may overflow on narrow screens. Ensure tables either scroll horizontally or reflow to a stacked layout on mobile.

---

## What You Got Right (Keep These)

These are genuine strengths that set this blog apart:

1. **The opening hook** — four contradictory numbers from the same data is immediately compelling
2. **Source-code verification** — tracing each tool's counting logic is rare and credible
3. **Real GitHub issue citations** — all 6 verified as real and accurately described
4. **The Python parser** — despite bugs, providing runnable code is a strong credibility move
5. **The timeline section** — explaining _why_ the ecosystem broke is more valuable than just _that_ it broke
6. **Cache pricing verification** — confirming 5-min (1.25x) and 1-hour (2x) tiers against official docs
7. **The "cost illusion" framing** — showing \$2,184 vs \$62,433 from the same data is a powerful illustration

---

## Suggested Revision Priority

1. Add Usage & Cost API mention (30 min)
2. Fix ccost "abandoned" claim (5 min)
3. Add conflict of interest disclosure (5 min)
4. Add per-model token breakdown (15 min)
5. Fix Python parser bugs (15 min)
6. Add methodology callout box (10 min)
7. Create 2-3 charts (1-2 hours)
8. Add tool comparison matrix (30 min)
9. Add meta description + JSON-LD (15 min)
10. Verify appendix link + add TOC (10 min)

**Estimated total effort: ~3-4 hours for a significantly stronger post.**

---

## Review Methodology

This review was conducted using:

- **Round 1:** Claude Opus 4.6 with 4 parallel research agents (GitHub verification, JSONL format validation, statistical plausibility, UX review)
- **Round 2:** Multi-provider adversarial review — Codex CLI (GPT-5.4) + Claude Opus 4.6 second-pass
- **Direct verification:** WebFetch on each GitHub issue URL, Anthropic pricing page, ccost/ccusage/claudelytics repositories
- **Independent cost calculation:** Using verified Anthropic pricing to reconstruct the blog's claimed figures

Notable finding: Codex (GPT-5.4) incorrectly claimed the GitHub issues were fake (looked up wrong repositories), while Claude correctly verified them. This cross-provider disagreement validated the multi-model approach and is itself evidence that single-model review is insufficient.

---

_Generated via Claude Octopus multi-LLM review pipeline. All claims independently verified against primary sources._
