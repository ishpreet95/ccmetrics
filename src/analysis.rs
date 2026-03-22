use std::collections::{HashMap, HashSet};

use chrono::Utc;

use crate::pricing;
use crate::types::{CostBreakdown, ModelBreakdown, ParseStats, Summary, UsageEntry};

/// Per-model accumulator used during the analysis loop.
#[derive(Default)]
struct ModelAccum {
    requests: usize,
    input: u64,
    output: u64,
    cache_read: u64,
    cache_write_5m: u64,
    cache_write_1h: u64,
    cost: f64,
}

/// Analyze deduplicated entries and produce a summary.
pub fn analyze(entries: &[UsageEntry], stats: &ParseStats) -> Summary {
    let mut input_tokens: u64 = 0;
    let mut output_tokens: u64 = 0;
    let mut cache_read_tokens: u64 = 0;
    let mut cache_write_5m_tokens: u64 = 0;
    let mut cache_write_1h_tokens: u64 = 0;
    let mut total_cost = CostBreakdown::default();

    let mut main_requests: usize = 0;
    let mut main_io_tokens: u64 = 0;
    let mut main_cost: f64 = 0.0;

    let mut sub_requests: usize = 0;
    let mut sub_io_tokens: u64 = 0;
    let mut sub_cost: f64 = 0.0;

    let mut model_map: HashMap<String, ModelAccum> = HashMap::new();

    let mut sessions: HashSet<String> = HashSet::new();
    let mut projects: HashSet<String> = HashSet::new();
    let mut first_ts = None;
    let mut last_ts = None;

    for entry in entries {
        input_tokens += entry.input_tokens;
        output_tokens += entry.output_tokens;
        cache_read_tokens += entry.cache_read_input_tokens;
        cache_write_5m_tokens += entry.cache_write_5m_tokens;
        cache_write_1h_tokens += entry.cache_write_1h_tokens;

        let cost = pricing::calculate_cost(entry);

        if entry.is_sidechain {
            sub_requests += 1;
            sub_io_tokens += entry.input_tokens + entry.output_tokens;
            sub_cost += cost.total();
        } else {
            main_requests += 1;
            main_io_tokens += entry.input_tokens + entry.output_tokens;
            main_cost += cost.total();
        }

        // Accumulate per-model stats
        let ma = model_map.entry(entry.model.clone()).or_default();
        ma.requests += 1;
        ma.input += entry.input_tokens;
        ma.output += entry.output_tokens;
        ma.cache_read += entry.cache_read_input_tokens;
        ma.cache_write_5m += entry.cache_write_5m_tokens;
        ma.cache_write_1h += entry.cache_write_1h_tokens;
        ma.cost += cost.total();

        total_cost += cost;

        sessions.insert(entry.session_id.clone());
        projects.insert(entry.project_path.clone());

        // Skip sentinel timestamps (UNIX_EPOCH) from corrupting the date range
        let ts = entry.timestamp;
        if ts.timestamp() > 0 {
            first_ts = Some(first_ts.map_or(ts, |prev: chrono::DateTime<Utc>| prev.min(ts)));
            last_ts = Some(last_ts.map_or(ts, |prev: chrono::DateTime<Utc>| prev.max(ts)));
        }
    }

    let days = match (first_ts, last_ts) {
        (Some(first), Some(last)) => {
            let diff = last.signed_duration_since(first);
            (diff.num_days().max(0) + 1) as u64
        }
        _ => 0,
    };

    // Convert per-model map to sorted Vec<ModelBreakdown>
    let mut by_model: Vec<ModelBreakdown> = model_map
        .into_iter()
        .map(|(model, a)| ModelBreakdown {
            model,
            requests: a.requests,
            input_tokens: a.input,
            output_tokens: a.output,
            cache_read_tokens: a.cache_read,
            cache_write_5m_tokens: a.cache_write_5m,
            cache_write_1h_tokens: a.cache_write_1h,
            cost: a.cost,
        })
        .collect();
    by_model.sort_by(|a, b| b.cost.total_cmp(&a.cost));

    let unique_requests = entries.len();
    let dedup_ratio = if unique_requests > 0 {
        stats.assistant_lines as f64 / unique_requests as f64
    } else {
        0.0
    };

    Summary {
        version: env!("CARGO_PKG_VERSION").to_string(),
        generated_at: Utc::now(),
        first_session: first_ts,
        last_session: last_ts,
        days,
        sessions: sessions.len(),
        projects: projects.len(),
        raw_lines: stats.assistant_lines,
        unique_requests,
        skipped_lines: stats.skipped_lines,
        dedup_ratio,
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_write_5m_tokens,
        cache_write_1h_tokens,
        cost: total_cost,
        main_requests,
        main_input_output_tokens: main_io_tokens,
        main_cost,
        subagent_requests: sub_requests,
        subagent_input_output_tokens: sub_io_tokens,
        subagent_cost: sub_cost,
        by_model,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Speed;
    use chrono::{DateTime, TimeZone, Utc};
    use std::path::PathBuf;

    fn make_entry(model: &str, input: u64, output: u64, timestamp: DateTime<Utc>) -> UsageEntry {
        UsageEntry {
            request_id: "req_1".to_string(),
            session_id: "s1".to_string(),
            model: model.to_string(),
            is_sidechain: false,
            timestamp,
            input_tokens: input,
            output_tokens: output,
            cache_read_input_tokens: 0,
            cache_write_5m_tokens: 0,
            cache_write_1h_tokens: 0,
            speed: Speed::Standard,
            inference_geo: None,
            web_search_requests: 0,
            web_fetch_requests: 0,
            source_file: PathBuf::from("test.jsonl"),
            project_path: "/test".to_string(),
        }
    }

    #[test]
    fn test_epoch_timestamp_excluded_from_range() {
        // An entry with UNIX_EPOCH timestamp should NOT affect first_session/last_session.
        let epoch = DateTime::UNIX_EPOCH;
        let real_ts = Utc.with_ymd_and_hms(2026, 3, 20, 10, 0, 0).unwrap();

        let entries = vec![
            make_entry("claude-opus-4-6", 100, 50, epoch), // sentinel — must be excluded
            make_entry("claude-opus-4-6", 100, 50, real_ts), // real timestamp
        ];

        let stats = ParseStats {
            assistant_lines: 2,
            ..Default::default()
        };

        let summary = analyze(&entries, &stats);

        assert_eq!(
            summary.first_session,
            Some(real_ts),
            "UNIX_EPOCH should not be the first_session"
        );
        assert_eq!(
            summary.last_session,
            Some(real_ts),
            "UNIX_EPOCH should not be the last_session"
        );
        assert_eq!(summary.days, 1, "Single real day should produce days=1");
    }

    #[test]
    fn test_by_model_aggregation() {
        let ts = Utc.with_ymd_and_hms(2026, 3, 20, 10, 0, 0).unwrap();

        let entries = vec![
            make_entry("claude-opus-4-6", 1000, 500, ts),
            make_entry("claude-opus-4-6", 2000, 300, ts),
            make_entry("claude-sonnet-4-6", 500, 100, ts),
        ];

        let stats = ParseStats {
            assistant_lines: 3,
            ..Default::default()
        };

        let summary = analyze(&entries, &stats);

        // Should have 2 distinct models
        assert_eq!(summary.by_model.len(), 2);

        // Sorted by cost descending — Opus is more expensive
        assert_eq!(summary.by_model[0].model, "claude-opus-4-6");
        assert_eq!(summary.by_model[1].model, "claude-sonnet-4-6");

        // Opus: 2 requests, 3000 input, 800 output
        assert_eq!(summary.by_model[0].requests, 2);
        assert_eq!(summary.by_model[0].input_tokens, 3000);
        assert_eq!(summary.by_model[0].output_tokens, 800);

        // Sonnet: 1 request, 500 input, 100 output
        assert_eq!(summary.by_model[1].requests, 1);
        assert_eq!(summary.by_model[1].input_tokens, 500);
        assert_eq!(summary.by_model[1].output_tokens, 100);

        // Per-model requests should sum to total
        let model_requests: usize = summary.by_model.iter().map(|m| m.requests).sum();
        assert_eq!(model_requests, summary.unique_requests);

        // Per-model input tokens should sum to total
        let model_input: u64 = summary.by_model.iter().map(|m| m.input_tokens).sum();
        assert_eq!(model_input, summary.input_tokens);

        // Per-model costs should be positive for known models
        assert!(
            summary.by_model[0].cost > 0.0,
            "Opus cost should be positive"
        );
        assert!(
            summary.by_model[1].cost > 0.0,
            "Sonnet cost should be positive"
        );
    }

    #[test]
    fn test_by_model_single_model() {
        let ts = Utc.with_ymd_and_hms(2026, 3, 20, 10, 0, 0).unwrap();

        let entries = vec![
            make_entry("claude-opus-4-6", 100, 50, ts),
            make_entry("claude-opus-4-6", 200, 30, ts),
        ];

        let stats = ParseStats {
            assistant_lines: 2,
            ..Default::default()
        };

        let summary = analyze(&entries, &stats);

        // Single model should produce exactly 1 entry
        assert_eq!(summary.by_model.len(), 1);
        assert_eq!(summary.by_model[0].model, "claude-opus-4-6");
        assert_eq!(summary.by_model[0].requests, 2);
        assert_eq!(summary.by_model[0].input_tokens, 300);
        assert_eq!(summary.by_model[0].output_tokens, 80);
    }

    #[test]
    fn test_by_model_empty_entries() {
        let entries: Vec<UsageEntry> = vec![];
        let stats = ParseStats::default();

        let summary = analyze(&entries, &stats);

        assert!(
            summary.by_model.is_empty(),
            "No entries should produce empty by_model"
        );
    }

    #[test]
    fn test_by_model_unknown_model_zero_cost() {
        let ts = Utc.with_ymd_and_hms(2026, 3, 20, 10, 0, 0).unwrap();

        let entries = vec![make_entry("unknown-model-v99", 1_000_000, 500_000, ts)];

        let stats = ParseStats {
            assistant_lines: 1,
            ..Default::default()
        };

        let summary = analyze(&entries, &stats);

        assert_eq!(summary.by_model.len(), 1);
        assert_eq!(summary.by_model[0].model, "unknown-model-v99");
        assert_eq!(summary.by_model[0].requests, 1);
        assert_eq!(summary.by_model[0].input_tokens, 1_000_000);
        // Unknown model should have zero cost
        assert!(
            summary.by_model[0].cost.abs() < 0.001,
            "Unknown model should have zero cost, got {}",
            summary.by_model[0].cost
        );
    }

    #[test]
    fn test_by_model_cache_tokens_accumulated() {
        let ts = Utc.with_ymd_and_hms(2026, 3, 20, 10, 0, 0).unwrap();

        let mut e1 = make_entry("claude-opus-4-6", 100, 50, ts);
        e1.cache_read_input_tokens = 5000;
        e1.cache_write_5m_tokens = 200;
        e1.cache_write_1h_tokens = 300;

        let mut e2 = make_entry("claude-opus-4-6", 100, 50, ts);
        e2.cache_read_input_tokens = 3000;
        e2.cache_write_5m_tokens = 100;
        e2.cache_write_1h_tokens = 0;

        let entries = vec![e1, e2];
        let stats = ParseStats {
            assistant_lines: 2,
            ..Default::default()
        };

        let summary = analyze(&entries, &stats);

        assert_eq!(summary.by_model.len(), 1);
        assert_eq!(summary.by_model[0].cache_read_tokens, 8000);
        assert_eq!(summary.by_model[0].cache_write_5m_tokens, 300);
        assert_eq!(summary.by_model[0].cache_write_1h_tokens, 300);

        // Cache tokens per model should equal summary totals
        assert_eq!(
            summary.by_model[0].cache_read_tokens,
            summary.cache_read_tokens
        );
        assert_eq!(
            summary.by_model[0].cache_write_5m_tokens,
            summary.cache_write_5m_tokens
        );
        assert_eq!(
            summary.by_model[0].cache_write_1h_tokens,
            summary.cache_write_1h_tokens
        );
    }

    #[test]
    fn test_by_model_subagent_grouped_by_model_not_sidechain() {
        let ts = Utc.with_ymd_and_hms(2026, 3, 20, 10, 0, 0).unwrap();

        // Main thread request using Opus
        let main_entry = make_entry("claude-opus-4-6", 1000, 500, ts);

        // Subagent request also using Opus
        let mut sub_entry = make_entry("claude-opus-4-6", 800, 200, ts);
        sub_entry.is_sidechain = true;

        // Subagent using Haiku
        let mut sub_haiku = make_entry("claude-haiku-4-5", 500, 100, ts);
        sub_haiku.is_sidechain = true;

        let entries = vec![main_entry, sub_entry, sub_haiku];
        let stats = ParseStats {
            assistant_lines: 3,
            ..Default::default()
        };

        let summary = analyze(&entries, &stats);

        // by_model groups by model name, NOT by sidechain status
        // So Opus main + Opus subagent = 1 group, Haiku = 1 group
        assert_eq!(summary.by_model.len(), 2);

        // Opus should have 2 requests (main + subagent combined)
        let opus = summary
            .by_model
            .iter()
            .find(|m| m.model == "claude-opus-4-6")
            .unwrap();
        assert_eq!(opus.requests, 2);
        assert_eq!(opus.input_tokens, 1800); // 1000 + 800
        assert_eq!(opus.output_tokens, 700); // 500 + 200

        // Haiku should have 1 request
        let haiku = summary
            .by_model
            .iter()
            .find(|m| m.model == "claude-haiku-4-5")
            .unwrap();
        assert_eq!(haiku.requests, 1);

        // Meanwhile, main/sub split should correctly separate
        assert_eq!(summary.main_requests, 1);
        assert_eq!(summary.subagent_requests, 2);

        // All token sums across models should equal summary totals
        let total_input: u64 = summary.by_model.iter().map(|m| m.input_tokens).sum();
        let total_output: u64 = summary.by_model.iter().map(|m| m.output_tokens).sum();
        assert_eq!(total_input, summary.input_tokens);
        assert_eq!(total_output, summary.output_tokens);
    }
}
