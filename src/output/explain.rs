use crate::explain::ExplainData;
use crate::output::table::format_number;

/// Render the explain walkthrough as terminal output.
pub fn render(data: &ExplainData, version: &str) -> String {
    let mut out = String::new();

    out.push_str(&format!("ccmetrics v{version} — Methodology Walkthrough\n"));

    // Step 1: Dedup
    out.push_str(
        "\n━━━ STEP 1: Streaming Chunk Deduplication ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n",
    );
    out.push_str(
        "One API request → multiple JSONL lines. Here's a real example from YOUR data:\n\n",
    );

    if let Some(ref ex) = data.dedup_example {
        out.push_str(&format!("  requestId: {}\n", ex.request_id));
        out.push_str(&format!(
            "  {} JSONL lines found (1 API request):\n\n",
            ex.chunks.len()
        ));

        out.push_str("  Line   stop_reason    output_tokens\n");
        out.push_str("  ────   ───────────    ─────────────\n");

        for (i, chunk) in ex.chunks.iter().enumerate() {
            let sr = chunk.stop_reason.as_deref().unwrap_or("null");
            let marker = if i == ex.kept_index {
                " ← REAL VALUE ✓"
            } else {
                " ← placeholder"
            };
            out.push_str(&format!(
                "  {:<6} {:<14} {:>5}{}\n",
                i + 1,
                sr,
                chunk.output_tokens,
                marker
            ));
        }

        let kept = &ex.chunks[ex.kept_index];
        let first = &ex.chunks[0];
        let sum: u64 = ex.chunks.iter().map(|c| c.output_tokens).sum();

        out.push_str(&format!(
            "\n  ccmetrics keeps:  line {} (stop_reason = \"{}\") → {} output tokens\n",
            ex.kept_index + 1,
            kept.stop_reason.as_deref().unwrap_or("?"),
            kept.output_tokens
        ));
        out.push_str(&format!(
            "  ccusage keeps:    line 1 (first-seen-wins)          → {} output tokens\n",
            first.output_tokens
        ));
        out.push_str(&format!(
            "  claudelytics:     all {} lines (no dedup)            → {} output tokens\n",
            ex.chunks.len(),
            sum
        ));

        if kept.output_tokens > 0 && first.output_tokens < kept.output_tokens {
            let undercount_pct =
                (1.0 - first.output_tokens as f64 / kept.output_tokens as f64) * 100.0;
            let overcount_pct = (sum as f64 / kept.output_tokens as f64 - 1.0) * 100.0;
            out.push_str(&format!(
                "\n  Impact: ccusage undercounts by {:.1}%. claudelytics overcounts by {:.1}%.\n",
                undercount_pct, overcount_pct
            ));
        }
    } else {
        out.push_str("  No multi-chunk requests found in your data.\n");
        out.push_str("  (All your requests had a single JSONL line — no dedup needed.)\n");
    }

    // Step 2: Pricing
    out.push_str(
        "\n━━━ STEP 2: Pricing Calculation ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n",
    );

    if let Some(ref px) = data.pricing_example {
        out.push_str(&format!(
            "For this request (model: {}, speed: {}):\n\n",
            px.model, px.speed
        ));

        out.push_str("  Token Type            Count        Rate ($/M)    Cost\n");
        out.push_str("  ──────────            ─────        ──────────    ────\n");

        let rows = [
            ("Input", px.input_tokens, px.input_rate),
            ("Output", px.output_tokens, px.output_rate),
            ("Cache read", px.cache_read_tokens, px.cache_read_rate),
            (
                "Cache write (5m)",
                px.cache_write_5m_tokens,
                px.cache_write_5m_rate,
            ),
            (
                "Cache write (1h)",
                px.cache_write_1h_tokens,
                px.cache_write_1h_rate,
            ),
        ];

        for (label, count, rate) in &rows {
            if *count > 0 {
                let cost = *count as f64 * rate / 1_000_000.0;
                out.push_str(&format!(
                    "  {:<22} {:>8}     ${:<10.2}    ${:.6}\n",
                    label,
                    format_number(*count),
                    rate,
                    cost
                ));
            }
        }

        if px.modifiers.is_empty() {
            out.push_str("\n  Modifiers: none (standard speed, no geo, <200k input)\n");
        } else {
            out.push_str(&format!("\n  Modifiers: {}\n", px.modifiers.join(", ")));
        }

        out.push_str(&format!("  Request total: ${:.6}\n", px.total_cost));
    }

    // Step 3: Cache tiers
    out.push_str(
        "\n━━━ STEP 3: Cache Tier Distinction ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n",
    );

    let ct = &data.cache_tier;
    if ct.total_5m_tokens > 0 && ct.total_1h_tokens > 0 {
        out.push_str("Your data uses TWO cache tiers with DIFFERENT pricing:\n\n");
        out.push_str("  Main thread:   1h cache writes (ephemeral_1h) — higher rate\n");
        out.push_str("  Subagents:     5m cache writes (ephemeral_5m) — lower rate\n\n");
        out.push_str(&format!(
            "  {} tokens at 1h rate = ${:.2}\n",
            format_number(ct.total_1h_tokens),
            ct.cost_1h
        ));
        out.push_str(&format!(
            "  {} tokens at 5m rate = ${:.2}\n",
            format_number(ct.total_5m_tokens),
            ct.cost_5m
        ));
        let actual = ct.cost_5m + ct.cost_1h;
        if ct.single_rate_cost > actual {
            let overcount_pct = (ct.single_rate_cost / actual - 1.0) * 100.0;
            out.push_str(&format!(
                "  vs single-rate: {} tokens at 1h rate = ${:.2} ({:.0}% overcounted)\n",
                format_number(ct.total_5m_tokens + ct.total_1h_tokens),
                ct.single_rate_cost,
                overcount_pct
            ));
        }
        out.push_str("\n  Other tools lump both tiers together → wrong cost.\n");
    } else if ct.total_1h_tokens > 0 {
        out.push_str("Your data only uses 1h cache tier.\n");
    } else if ct.total_5m_tokens > 0 {
        out.push_str("Your data only uses 5m cache tier.\n");
    } else {
        out.push_str("No cache write data found.\n");
    }

    // Step 4: Aggregate comparison
    out.push_str("\n━━━ STEP 4: Your Aggregate Numbers ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n");

    let cmp = &data.comparison;
    out.push_str("  Tool           Output Tokens    Total Cost    Methodology\n");
    out.push_str("  ────           ─────────────    ──────────    ───────────\n");
    out.push_str(&format!(
        "  ccmetrics      {:>13}    ${:>9.2}    Final chunk, 5-type split, modifiers\n",
        format_number(cmp.our_output_tokens),
        cmp.our_cost
    ));
    out.push_str(&format!(
        "  ccusage*       {:>13}    (varies)      First-seen-wins (placeholder tokens)\n",
        format_number(cmp.first_seen_output_tokens),
    ));
    out.push_str(&format!(
        "  claudelytics*  {:>13}    (varies)      No dedup (all chunks counted)\n",
        format_number(cmp.no_dedup_output_tokens),
    ));
    out.push_str("\n  * Estimated based on methodology, not actual tool output.\n");

    out
}
