use std::collections::{HashMap, HashSet};

use chrono::Utc;

use crate::pricing;
use crate::types::{
    CostBreakdown, DayBreakdown, ModelBreakdown, ParseStats, ProjectBreakdown, SessionBreakdown,
    Summary, UsageEntry,
};

/// Per-project accumulator used during the analysis loop.
#[derive(Default)]
struct ProjectAccum {
    sessions: HashSet<String>,
    requests: usize,
    input: u64,
    output: u64,
    cost: f64,
}

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
    let mut project_map: HashMap<String, ProjectAccum> = HashMap::new();

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

        // Accumulate per-project stats
        let pa = project_map.entry(entry.project_path.clone()).or_default();
        pa.sessions.insert(entry.session_id.clone());
        pa.requests += 1;
        pa.input += entry.input_tokens;
        pa.output += entry.output_tokens;
        pa.cost += cost.total();

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
    by_model.sort_by(|a, b| b.cost.total_cmp(&a.cost).then(a.model.cmp(&b.model)));

    // Convert per-project map to sorted Vec<ProjectBreakdown>
    let mut by_project: Vec<ProjectBreakdown> = project_map
        .into_iter()
        .map(|(project, a)| ProjectBreakdown {
            project,
            sessions: a.sessions.len(),
            requests: a.requests,
            input_tokens: a.input,
            output_tokens: a.output,
            cost: a.cost,
        })
        .collect();
    by_project.sort_by(|a, b| b.cost.total_cmp(&a.cost).then(a.project.cmp(&b.project)));

    let unique_requests = entries.len();
    // Use pre-filter unique count for dedup ratio so it's not inflated by filtering
    let dedup_base = if stats.unique_after_dedup > 0 {
        stats.unique_after_dedup
    } else {
        unique_requests
    };
    let dedup_ratio = if dedup_base > 0 {
        stats.assistant_lines as f64 / dedup_base as f64
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
        by_project,
    }
}

/// Per-day accumulator used during the daily analysis.
#[derive(Default)]
struct DayAccum {
    requests: usize,
    input: u64,
    output: u64,
    cost: f64,
}

/// Group deduplicated entries by day and produce daily breakdowns.
pub fn analyze_daily(entries: &[UsageEntry]) -> Vec<DayBreakdown> {
    let mut day_map: HashMap<String, DayAccum> = HashMap::new();

    for entry in entries {
        // Skip sentinel timestamps (UNIX_EPOCH) — same guard as analyze()
        if entry.timestamp.timestamp() <= 0 {
            continue;
        }
        let date_str = entry.timestamp.format("%Y-%m-%d").to_string();
        let cost = pricing::calculate_cost(entry).total();
        let acc = day_map.entry(date_str).or_default();
        acc.requests += 1;
        acc.input += entry.input_tokens;
        acc.output += entry.output_tokens;
        acc.cost += cost;
    }

    let mut days: Vec<DayBreakdown> = day_map
        .into_iter()
        .map(|(date, a)| DayBreakdown {
            date,
            requests: a.requests,
            input_tokens: a.input,
            output_tokens: a.output,
            cost: a.cost,
        })
        .collect();

    // Sort by ISO date descending (most recent first)
    days.sort_by(|a, b| b.date.cmp(&a.date));
    days
}

/// Per-session accumulator.
#[derive(Default)]
struct SessionAccum {
    project: String,
    requests: usize,
    subagent_spawns: usize,
    input: u64,
    output: u64,
    cache_read: u64,
    cache_write_5m: u64,
    cache_write_1h: u64,
    cost: f64,
    model_counts: HashMap<String, usize>,
    first_ts: Option<chrono::DateTime<Utc>>,
    last_ts: Option<chrono::DateTime<Utc>>,
}

/// Group deduplicated entries by session and produce session breakdowns.
pub fn analyze_sessions(entries: &[UsageEntry]) -> Vec<SessionBreakdown> {
    let mut session_map: HashMap<String, SessionAccum> = HashMap::new();

    for entry in entries {
        let acc = session_map
            .entry(entry.session_id.clone())
            .or_insert_with(|| SessionAccum {
                project: entry.project_path.clone(),
                ..Default::default()
            });
        acc.requests += 1;
        if entry.is_sidechain {
            acc.subagent_spawns += 1;
        }
        acc.input += entry.input_tokens;
        acc.output += entry.output_tokens;
        acc.cache_read += entry.cache_read_input_tokens;
        acc.cache_write_5m += entry.cache_write_5m_tokens;
        acc.cache_write_1h += entry.cache_write_1h_tokens;
        acc.cost += pricing::calculate_cost(entry).total();
        *acc.model_counts.entry(entry.model.clone()).or_default() += 1;

        let ts = entry.timestamp;
        if ts.timestamp() > 0 {
            acc.first_ts = Some(acc.first_ts.map_or(ts, |prev| prev.min(ts)));
            acc.last_ts = Some(acc.last_ts.map_or(ts, |prev| prev.max(ts)));
        }
    }

    let mut sessions: Vec<SessionBreakdown> = session_map
        .into_iter()
        .map(|(session_id, a)| {
            // Primary model = the one with the most requests
            let primary_model = a
                .model_counts
                .into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(model, _)| model)
                .unwrap_or_default();

            let date = a
                .first_ts
                .map(|ts| ts.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "unknown".to_string());

            let duration_minutes = match (a.first_ts, a.last_ts) {
                (Some(f), Some(l)) => Some(l.signed_duration_since(f).num_minutes().max(0) as u64),
                _ => None,
            };

            SessionBreakdown {
                session_id,
                date,
                project: a.project,
                requests: a.requests,
                subagent_spawns: a.subagent_spawns,
                primary_model,
                input_tokens: a.input,
                output_tokens: a.output,
                cache_read_tokens: a.cache_read,
                cache_write_5m_tokens: a.cache_write_5m,
                cache_write_1h_tokens: a.cache_write_1h,
                cost: a.cost,
                duration_minutes,
            }
        })
        .collect();

    // Sort by date descending, then cost descending, then session_id for stability
    sessions.sort_by(|a, b| {
        b.date
            .cmp(&a.date)
            .then(b.cost.total_cmp(&a.cost))
            .then(a.session_id.cmp(&b.session_id))
    });
    sessions
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

    fn make_entry_full(
        model: &str,
        input: u64,
        output: u64,
        timestamp: DateTime<Utc>,
        session_id: &str,
        project: &str,
    ) -> UsageEntry {
        let mut e = make_entry(model, input, output, timestamp);
        e.session_id = session_id.to_string();
        e.project_path = project.to_string();
        e
    }

    // --- by_project tests ---

    #[test]
    fn test_by_project_aggregation() {
        let ts = Utc.with_ymd_and_hms(2026, 3, 20, 10, 0, 0).unwrap();

        let entries = vec![
            make_entry_full("claude-opus-4-6", 1000, 500, ts, "s1", "/proj-a"),
            make_entry_full("claude-opus-4-6", 2000, 300, ts, "s1", "/proj-a"),
            make_entry_full("claude-opus-4-6", 500, 100, ts, "s2", "/proj-b"),
        ];

        let stats = ParseStats {
            assistant_lines: 3,
            ..Default::default()
        };

        let summary = analyze(&entries, &stats);

        assert_eq!(summary.by_project.len(), 2);
        // Sorted by cost descending — proj-a has more tokens
        assert_eq!(summary.by_project[0].project, "/proj-a");
        assert_eq!(summary.by_project[0].requests, 2);
        assert_eq!(summary.by_project[0].sessions, 1); // both s1
        assert_eq!(summary.by_project[0].input_tokens, 3000);
        assert_eq!(summary.by_project[0].output_tokens, 800);

        assert_eq!(summary.by_project[1].project, "/proj-b");
        assert_eq!(summary.by_project[1].requests, 1);
        assert_eq!(summary.by_project[1].sessions, 1); // s2
    }

    #[test]
    fn test_by_project_multiple_sessions_counted() {
        let ts = Utc.with_ymd_and_hms(2026, 3, 20, 10, 0, 0).unwrap();

        let entries = vec![
            make_entry_full("claude-opus-4-6", 100, 50, ts, "s1", "/proj-a"),
            make_entry_full("claude-opus-4-6", 100, 50, ts, "s2", "/proj-a"),
            make_entry_full("claude-opus-4-6", 100, 50, ts, "s3", "/proj-a"),
        ];

        let stats = ParseStats {
            assistant_lines: 3,
            ..Default::default()
        };

        let summary = analyze(&entries, &stats);

        assert_eq!(summary.by_project.len(), 1);
        assert_eq!(summary.by_project[0].sessions, 3);
        assert_eq!(summary.by_project[0].requests, 3);
    }

    // --- analyze_daily tests ---

    #[test]
    fn test_daily_groups_by_date() {
        let day1 = Utc.with_ymd_and_hms(2026, 3, 20, 10, 0, 0).unwrap();
        let day1_later = Utc.with_ymd_and_hms(2026, 3, 20, 15, 0, 0).unwrap();
        let day2 = Utc.with_ymd_and_hms(2026, 3, 21, 10, 0, 0).unwrap();

        let entries = vec![
            make_entry("claude-opus-4-6", 1000, 500, day1),
            make_entry("claude-opus-4-6", 2000, 300, day1_later),
            make_entry("claude-opus-4-6", 500, 100, day2),
        ];

        let days = analyze_daily(&entries);

        assert_eq!(days.len(), 2);
        // Sorted by date descending
        assert_eq!(days[0].date, "2026-03-21");
        assert_eq!(days[0].requests, 1);
        assert_eq!(days[0].input_tokens, 500);

        assert_eq!(days[1].date, "2026-03-20");
        assert_eq!(days[1].requests, 2);
        assert_eq!(days[1].input_tokens, 3000);
    }

    #[test]
    fn test_daily_empty() {
        let days = analyze_daily(&[]);
        assert!(days.is_empty());
    }

    // --- analyze_sessions tests ---

    #[test]
    fn test_sessions_grouped_by_id() {
        let ts = Utc.with_ymd_and_hms(2026, 3, 20, 10, 0, 0).unwrap();

        let entries = vec![
            make_entry_full("claude-opus-4-6", 1000, 500, ts, "session-aaa", "/proj-a"),
            make_entry_full("claude-opus-4-6", 2000, 300, ts, "session-aaa", "/proj-a"),
            make_entry_full("claude-sonnet-4-6", 500, 100, ts, "session-bbb", "/proj-b"),
        ];

        let sessions = analyze_sessions(&entries);

        assert_eq!(sessions.len(), 2);
        // Both sessions are same date, sorted by cost descending
        let aaa = sessions
            .iter()
            .find(|s| s.session_id == "session-aaa")
            .unwrap();
        assert_eq!(aaa.requests, 2);
        assert_eq!(aaa.input_tokens, 3000);
        assert_eq!(aaa.output_tokens, 800);
        assert_eq!(aaa.primary_model, "claude-opus-4-6");

        let bbb = sessions
            .iter()
            .find(|s| s.session_id == "session-bbb")
            .unwrap();
        assert_eq!(bbb.requests, 1);
        assert_eq!(bbb.project, "/proj-b");
    }

    #[test]
    fn test_sessions_subagent_counted() {
        let ts = Utc.with_ymd_and_hms(2026, 3, 20, 10, 0, 0).unwrap();

        let mut main = make_entry_full("claude-opus-4-6", 1000, 500, ts, "s1", "/proj");
        let mut sub = make_entry_full("claude-haiku-4-5", 200, 50, ts, "s1", "/proj");
        sub.is_sidechain = true;
        main.request_id = "req_main".to_string();
        sub.request_id = "req_sub".to_string();

        let sessions = analyze_sessions(&[main, sub]);

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].requests, 2);
        assert_eq!(sessions[0].subagent_spawns, 1);
        // Primary model should be opus (1 req) or haiku (1 req) — tie breaks by model name in HashMap
        // But both have 1 request, so it's non-deterministic. Just check it's one of them.
        assert!(
            sessions[0].primary_model == "claude-opus-4-6"
                || sessions[0].primary_model == "claude-haiku-4-5"
        );
    }

    #[test]
    fn test_sessions_empty() {
        let sessions = analyze_sessions(&[]);
        assert!(sessions.is_empty());
    }
}
