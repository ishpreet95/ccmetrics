use std::collections::HashSet;

use chrono::Utc;

use crate::pricing;
use crate::types::{CostBreakdown, ParseStats, Summary, UsageEntry};

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
            make_entry("claude-opus-4-6", 100, 50, epoch),    // sentinel — must be excluded
            make_entry("claude-opus-4-6", 100, 50, real_ts),  // real timestamp
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
}
