use std::collections::HashMap;

use crate::parser::RawEntry;
use crate::pricing;
use crate::types::{Summary, UsageEntry};

/// Data needed to render the explain output.
pub struct ExplainData {
    pub dedup_example: Option<DedupExample>,
    pub pricing_example: Option<PricingExample>,
    pub cache_tier: CacheTierExample,
    pub comparison: ComparisonData,
}

/// A real multi-chunk streaming request from the user's data.
pub struct DedupExample {
    pub request_id: String,
    pub chunks: Vec<ChunkInfo>,
    pub kept_index: usize,
}

pub struct ChunkInfo {
    pub stop_reason: Option<String>,
    pub output_tokens: u64,
}

/// Step-by-step pricing for one request.
pub struct PricingExample {
    pub model: String,
    pub speed: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_5m_tokens: u64,
    pub cache_write_1h_tokens: u64,
    pub input_rate: f64,
    pub output_rate: f64,
    pub cache_read_rate: f64,
    pub cache_write_5m_rate: f64,
    pub cache_write_1h_rate: f64,
    pub modifiers: Vec<String>,
    pub total_cost: f64,
}

/// Cache tier comparison from aggregate data.
pub struct CacheTierExample {
    pub total_5m_tokens: u64,
    pub total_1h_tokens: u64,
    pub cost_5m: f64,
    pub cost_1h: f64,
    pub single_rate_cost: f64,
}

/// What other tools would report.
pub struct ComparisonData {
    pub our_output_tokens: u64,
    pub our_cost: f64,
    pub first_seen_output_tokens: u64,
    pub no_dedup_output_tokens: u64,
}

/// Build explain data from raw entries, deduped entries, and summary.
pub fn build_explain(
    raw_entries: &[RawEntry],
    deduped: &[UsageEntry],
    summary: &Summary,
) -> ExplainData {
    // Step 1: Find the best multi-chunk request to use as example
    let dedup_example = find_best_dedup_example(raw_entries);

    // Step 2: Build pricing example from the deduped version of that request
    let pricing_example = dedup_example.as_ref().and_then(|ex| {
        deduped
            .iter()
            .find(|e| e.request_id.contains(&ex.request_id))
            .map(build_pricing_example)
    });

    // Step 3: Cache tier data from summary
    let cache_tier = CacheTierExample {
        total_5m_tokens: summary.cache_write_5m_tokens,
        total_1h_tokens: summary.cache_write_1h_tokens,
        cost_5m: summary.cost.cache_write_5m,
        cost_1h: summary.cost.cache_write_1h,
        // What single-rate pricing would produce (all at 1h rate)
        single_rate_cost: {
            let combined = summary.cache_write_5m_tokens + summary.cache_write_1h_tokens;
            // Use a representative 1h rate (weighted average across models would be ideal
            // but for the example, use the most common rate)
            combined as f64 * 10.0 / 1_000_000.0 // Opus 1h rate as representative
        },
    };

    // Step 4: Build comparison data
    let comparison = build_comparison(raw_entries, summary);

    ExplainData {
        dedup_example,
        pricing_example,
        cache_tier,
        comparison,
    }
}

/// Find a multi-chunk request with the most chunks (best illustration of dedup).
fn find_best_dedup_example(raw_entries: &[RawEntry]) -> Option<DedupExample> {
    let mut groups: HashMap<String, Vec<&RawEntry>> = HashMap::new();

    for entry in raw_entries {
        if let Some(ref rid) = entry.request_id {
            // Use session-scoped key to match the real dedup pipeline
            let key = format!("{}:{}", entry.session_id, rid);
            groups.entry(key).or_default().push(entry);
        }
    }

    // Find a group with 3-10 chunks that has at least one stop_reason set.
    // Without stop_reason, the example would show a truncated session
    // which doesn't illustrate the dedup value proposition.
    let best = groups
        .into_iter()
        .filter(|(_, chunks)| {
            chunks.len() >= 3 && chunks.iter().any(|e| e.stop_reason.is_some())
        })
        .min_by_key(|(_, chunks)| {
            let len = chunks.len();
            // Prefer 4-6 chunks (score 0-2), then 3 or 7-10 (score 3-7), then 11+ (score 100+)
            if (4..=6).contains(&len) {
                len.abs_diff(5) // 0-1
            } else if len <= 10 {
                len.abs_diff(5) + 3
            } else {
                100 + len // deprioritize very long groups
            }
        });

    best.map(|(session_key, mut chunks)| {
        // Extract the bare requestId for display (key is "session:requestId")
        let request_id = session_key
            .split_once(':')
            .map(|(_, rid)| rid.to_string())
            .unwrap_or(session_key);
        chunks.sort_by_key(|e| e.line_number);

        let kept_index = chunks
            .iter()
            .rposition(|e| e.stop_reason.is_some())
            .unwrap_or(chunks.len() - 1);

        let chunk_infos: Vec<ChunkInfo> = chunks
            .iter()
            .map(|e| ChunkInfo {
                stop_reason: e.stop_reason.clone(),
                output_tokens: e.output_tokens,
            })
            .collect();

        DedupExample {
            request_id,
            chunks: chunk_infos,
            kept_index,
        }
    })
}

fn build_pricing_example(entry: &UsageEntry) -> PricingExample {
    let rates = pricing::lookup_rates(&entry.model);
    let (input_rate, output_rate, cr_rate, c5m_rate, c1h_rate) = match rates {
        Some(r) => (r.input, r.output, r.cache_read, r.cache_write_5m, r.cache_write_1h),
        None => (0.0, 0.0, 0.0, 0.0, 0.0),
    };

    let mut modifiers = Vec::new();
    if entry.speed == crate::types::Speed::Fast {
        modifiers.push("fast mode (6x)".to_string());
    }
    if entry.inference_geo.as_deref() == Some("us") {
        modifiers.push("data residency (1.1x)".to_string());
    }
    if pricing::is_long_context_eligible(&entry.model) && entry.input_tokens > 200_000 {
        modifiers.push("long context (2x input, 1.5x output)".to_string());
    }

    let cost = pricing::calculate_cost(entry);

    PricingExample {
        model: entry.model.clone(),
        speed: match entry.speed {
            crate::types::Speed::Standard => "standard".to_string(),
            crate::types::Speed::Fast => "fast".to_string(),
        },
        input_tokens: entry.input_tokens,
        output_tokens: entry.output_tokens,
        cache_read_tokens: entry.cache_read_input_tokens,
        cache_write_5m_tokens: entry.cache_write_5m_tokens,
        cache_write_1h_tokens: entry.cache_write_1h_tokens,
        input_rate,
        output_rate,
        cache_read_rate: cr_rate,
        cache_write_5m_rate: c5m_rate,
        cache_write_1h_rate: c1h_rate,
        modifiers,
        total_cost: cost.total(),
    }
}

/// Calculate what other tools would report for comparison.
///
/// Entries without either `request_id` or `message_id` are skipped entirely
/// in the first-seen simulation (they have no grouping key).
fn build_comparison(raw_entries: &[RawEntry], summary: &Summary) -> ComparisonData {
    // First-seen-wins: for each requestId, take the FIRST entry's output_tokens
    let mut first_seen: HashMap<String, u64> = HashMap::new();
    let mut no_dedup_total: u64 = 0;

    for entry in raw_entries {
        if let Some(key) = entry.request_id.as_deref().or(entry.message_id.as_deref()) {
            first_seen.entry(key.to_string()).or_insert(entry.output_tokens);
        }
        no_dedup_total += entry.output_tokens;
    }

    let first_seen_total: u64 = first_seen.values().sum();

    ComparisonData {
        our_output_tokens: summary.output_tokens,
        our_cost: summary.cost.total(),
        first_seen_output_tokens: first_seen_total,
        no_dedup_output_tokens: no_dedup_total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Speed;
    use chrono::Utc;
    use std::path::PathBuf;

    fn make_raw(
        request_id: &str,
        session_id: &str,
        stop_reason: Option<&str>,
        output_tokens: u64,
        line: usize,
    ) -> RawEntry {
        RawEntry {
            request_id: Some(request_id.to_string()),
            message_id: Some(format!("msg_{}", request_id)),
            session_id: session_id.to_string(),
            model: "claude-opus-4-6".to_string(),
            is_sidechain: false,
            timestamp: Utc::now(),
            stop_reason: stop_reason.map(String::from),
            input_tokens: 100,
            output_tokens,
            cache_read_input_tokens: 0,
            cache_write_5m_tokens: 0,
            cache_write_1h_tokens: 0,
            speed: Speed::Standard,
            inference_geo: None,
            web_search_requests: 0,
            web_fetch_requests: 0,
            source_file: PathBuf::from("test.jsonl"),
            project_path: "/test".to_string(),
            line_number: line,
        }
    }

    fn make_summary(output_tokens: u64, cost_total: f64) -> Summary {
        use crate::types::CostBreakdown;
        Summary {
            version: "0.1.0".to_string(),
            generated_at: Utc::now(),
            first_session: None,
            last_session: None,
            days: 0,
            sessions: 0,
            projects: 0,
            raw_lines: 0,
            unique_requests: 0,
            skipped_lines: 0,
            dedup_ratio: 0.0,
            input_tokens: 0,
            output_tokens,
            cache_read_tokens: 0,
            cache_write_5m_tokens: 0,
            cache_write_1h_tokens: 0,
            cost: CostBreakdown {
                input: cost_total / 2.0,
                output: cost_total / 2.0,
                cache_read: 0.0,
                cache_write_5m: 0.0,
                cache_write_1h: 0.0,
                web_search: 0.0,
            },
            main_requests: 0,
            main_input_output_tokens: 0,
            main_cost: 0.0,
            subagent_requests: 0,
            subagent_input_output_tokens: 0,
            subagent_cost: 0.0,
        }
    }

    #[test]
    fn test_find_dedup_example_picks_right_size() {
        // Create groups of 2, 5, and 20 chunks. The algorithm prefers 4-6 chunks,
        // so it should pick the 5-chunk group.
        let mut entries = Vec::new();

        // 2-chunk group (below minimum of 3, will be filtered out)
        for i in 0..2 {
            entries.push(make_raw("req_2chunk", "s1", if i == 1 { Some("end_turn") } else { None }, 10, i + 1));
        }

        // 5-chunk group (ideal range 4-6)
        for i in 0..5 {
            entries.push(make_raw("req_5chunk", "s1", if i == 4 { Some("end_turn") } else { None }, 50, 10 + i));
        }

        // 20-chunk group (deprioritized as >10)
        for i in 0..20 {
            entries.push(make_raw("req_20chunk", "s1", if i == 19 { Some("end_turn") } else { None }, 100, 30 + i));
        }

        let result = find_best_dedup_example(&entries);
        assert!(result.is_some(), "Should find a dedup example");
        let example = result.unwrap();
        assert_eq!(example.request_id, "req_5chunk");
        assert_eq!(example.chunks.len(), 5);
    }

    #[test]
    fn test_find_dedup_example_requires_stop_reason() {
        // Groups without any stop_reason should be skipped
        let mut entries = Vec::new();
        for i in 0..5 {
            entries.push(make_raw("req_no_stop", "s1", None, 10, i + 1));
        }

        let result = find_best_dedup_example(&entries);
        assert!(result.is_none(), "Should skip groups without any stop_reason");
    }

    #[test]
    fn test_find_dedup_example_none_when_no_multi_chunk() {
        // All single-chunk entries should return None
        let entries = vec![
            make_raw("req_a", "s1", Some("end_turn"), 100, 1),
            make_raw("req_b", "s1", Some("end_turn"), 200, 2),
            make_raw("req_c", "s1", Some("tool_use"), 300, 3),
        ];

        let result = find_best_dedup_example(&entries);
        assert!(result.is_none(), "Single-chunk entries should not produce a dedup example");
    }

    #[test]
    fn test_build_comparison_first_seen_wins() {
        // Create entries where the same requestId appears multiple times.
        // First-seen simulation should pick the first entry's output_tokens.
        let entries = vec![
            make_raw("req_1", "s1", None, 10, 1),       // first seen for req_1
            make_raw("req_1", "s1", None, 50, 2),       // duplicate, ignored by first-seen
            make_raw("req_1", "s1", Some("end_turn"), 365, 3), // duplicate, ignored by first-seen
            make_raw("req_2", "s1", Some("end_turn"), 200, 4), // first seen for req_2
        ];

        let summary = make_summary(565, 1.0);
        let comparison = build_comparison(&entries, &summary);

        // First-seen: req_1 -> 10, req_2 -> 200 => total 210
        assert_eq!(comparison.first_seen_output_tokens, 210);
        // No dedup: 10 + 50 + 365 + 200 = 625
        assert_eq!(comparison.no_dedup_output_tokens, 625);
    }

    #[test]
    fn test_build_comparison_no_id_entries_excluded() {
        // Entries without requestId or message_id should not inflate first-seen count
        let mut entry_no_id = make_raw("req_1", "s1", Some("end_turn"), 100, 1);
        entry_no_id.request_id = None;
        entry_no_id.message_id = None;

        let entry_with_id = make_raw("req_2", "s1", Some("end_turn"), 200, 2);

        let entries = vec![entry_no_id, entry_with_id];
        let summary = make_summary(300, 1.0);
        let comparison = build_comparison(&entries, &summary);

        // Only req_2 counted in first-seen (entry_no_id has no key)
        assert_eq!(comparison.first_seen_output_tokens, 200);
        // No-dedup counts all output_tokens regardless of ID
        assert_eq!(comparison.no_dedup_output_tokens, 300);
    }
}
