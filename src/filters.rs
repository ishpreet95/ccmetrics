use chrono::{DateTime, Duration, NaiveDate, Utc};

use crate::types::UsageEntry;

/// Parsed filter criteria from CLI flags.
#[derive(Debug, Default)]
pub struct Filters {
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub model: Option<String>,
    pub project: Option<String>,
}

impl Filters {
    /// Returns true if any filter is active.
    pub fn is_active(&self) -> bool {
        self.since.is_some()
            || self.until.is_some()
            || self.model.is_some()
            || self.project.is_some()
    }

    /// Returns a human-readable description of active filters.
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if let Some(since) = &self.since {
            parts.push(format!("since {}", since.format("%Y-%m-%d")));
        }
        if let Some(until) = &self.until {
            parts.push(format!("until {}", until.format("%Y-%m-%d")));
        }
        if let Some(model) = &self.model {
            parts.push(format!("model: {model}"));
        }
        if let Some(project) = &self.project {
            parts.push(format!("project: {project}"));
        }
        parts.join(", ")
    }

    /// Apply all active filters to a list of entries.
    pub fn apply(&self, entries: Vec<UsageEntry>) -> Vec<UsageEntry> {
        entries
            .into_iter()
            .filter(|e| {
                if let Some(since) = &self.since {
                    if e.timestamp < *since {
                        return false;
                    }
                }
                if let Some(until) = &self.until {
                    // Half-open interval: until is start-of-next-day, so use strict <
                    if e.timestamp >= *until {
                        return false;
                    }
                }
                if let Some(model) = &self.model {
                    if !e.model.to_lowercase().contains(&model.to_lowercase()) {
                        return false;
                    }
                }
                if let Some(project) = &self.project {
                    if !e
                        .project_path
                        .to_lowercase()
                        .contains(&project.to_lowercase())
                    {
                        return false;
                    }
                }
                true
            })
            .collect()
    }
}

/// Parse a date string into a DateTime<Utc>.
///
/// Supported formats:
/// - ISO date: `2026-03-01` → start of that day (00:00:00 UTC)
/// - Relative days: `7d` → 7 days ago from now
/// - Relative weeks: `2w` → 14 days ago from now
/// - Keyword: `today` → start of today (00:00:00 UTC)
pub fn parse_date(input: &str) -> Result<DateTime<Utc>, String> {
    let input = input.trim();

    // Keyword: today
    if input.eq_ignore_ascii_case("today") {
        let today = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
        return Ok(DateTime::from_naive_utc_and_offset(today, Utc));
    }

    // Relative: Nd (days)
    if let Some(n) = input.strip_suffix('d') {
        let days: i64 = n
            .parse()
            .map_err(|_| format!("Invalid number in relative date: '{input}'"))?;
        if days < 0 {
            return Err(format!("Relative days must be positive: '{input}'"));
        }
        let dt = Utc::now() - Duration::days(days);
        let start_of_day = dt.date_naive().and_hms_opt(0, 0, 0).unwrap();
        return Ok(DateTime::from_naive_utc_and_offset(start_of_day, Utc));
    }

    // Relative: Nw (weeks)
    if let Some(n) = input.strip_suffix('w') {
        let weeks: i64 = n
            .parse()
            .map_err(|_| format!("Invalid number in relative date: '{input}'"))?;
        if weeks < 0 {
            return Err(format!("Relative weeks must be positive: '{input}'"));
        }
        let dt = Utc::now() - Duration::weeks(weeks);
        let start_of_day = dt.date_naive().and_hms_opt(0, 0, 0).unwrap();
        return Ok(DateTime::from_naive_utc_and_offset(start_of_day, Utc));
    }

    // ISO date: YYYY-MM-DD
    if let Ok(date) = NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        let dt = date.and_hms_opt(0, 0, 0).unwrap();
        return Ok(DateTime::from_naive_utc_and_offset(dt, Utc));
    }

    Err(format!(
        "Unrecognized date format: '{input}'. Expected: YYYY-MM-DD, 7d, 2w, or today"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Speed;
    use chrono::TimeZone;
    use std::path::PathBuf;

    fn make_entry(model: &str, project: &str, timestamp: DateTime<Utc>) -> UsageEntry {
        UsageEntry {
            request_id: "req_1".to_string(),
            session_id: "s1".to_string(),
            model: model.to_string(),
            is_sidechain: false,
            timestamp,
            input_tokens: 100,
            output_tokens: 50,
            cache_read_input_tokens: 0,
            cache_write_5m_tokens: 0,
            cache_write_1h_tokens: 0,
            speed: Speed::Standard,
            inference_geo: None,
            web_search_requests: 0,
            web_fetch_requests: 0,
            source_file: PathBuf::from("test.jsonl"),
            project_path: project.to_string(),
        }
    }

    // --- parse_date tests ---

    #[test]
    fn test_parse_iso_date() {
        let dt = parse_date("2026-03-15").unwrap();
        assert_eq!(dt, Utc.with_ymd_and_hms(2026, 3, 15, 0, 0, 0).unwrap());
    }

    #[test]
    fn test_parse_today() {
        let dt = parse_date("today").unwrap();
        let now = Utc::now();
        assert_eq!(dt.date_naive(), now.date_naive());
        // Should be start of day
        assert_eq!(dt.time(), chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
    }

    #[test]
    fn test_parse_today_case_insensitive() {
        assert!(parse_date("Today").is_ok());
        assert!(parse_date("TODAY").is_ok());
    }

    #[test]
    fn test_parse_relative_days() {
        let dt = parse_date("7d").unwrap();
        let expected = (Utc::now() - Duration::days(7)).date_naive();
        assert_eq!(dt.date_naive(), expected);
    }

    #[test]
    fn test_parse_relative_weeks() {
        let dt = parse_date("2w").unwrap();
        let expected = (Utc::now() - Duration::weeks(2)).date_naive();
        assert_eq!(dt.date_naive(), expected);
    }

    #[test]
    fn test_parse_zero_days() {
        // 0d should be start of today
        let dt = parse_date("0d").unwrap();
        assert_eq!(dt.date_naive(), Utc::now().date_naive());
    }

    #[test]
    fn test_parse_invalid_format() {
        assert!(parse_date("foobar").is_err());
        assert!(parse_date("").is_err());
        assert!(parse_date("2026/03/15").is_err());
        assert!(parse_date("-7d").is_err());
    }

    #[test]
    fn test_parse_invalid_relative() {
        assert!(parse_date("abcd").is_err());
        assert!(parse_date("xd").is_err());
    }

    // --- filter tests ---

    #[test]
    fn test_filter_by_date_since() {
        let t1 = Utc.with_ymd_and_hms(2026, 3, 10, 12, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2026, 3, 20, 12, 0, 0).unwrap();

        let entries = vec![
            make_entry("opus", "/project", t1),
            make_entry("opus", "/project", t2),
        ];

        let filters = Filters {
            since: Some(Utc.with_ymd_and_hms(2026, 3, 15, 0, 0, 0).unwrap()),
            ..Default::default()
        };

        let result = filters.apply(entries);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].timestamp, t2);
    }

    #[test]
    fn test_filter_by_date_until() {
        let t1 = Utc.with_ymd_and_hms(2026, 3, 10, 12, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2026, 3, 20, 12, 0, 0).unwrap();

        let entries = vec![
            make_entry("opus", "/project", t1),
            make_entry("opus", "/project", t2),
        ];

        let filters = Filters {
            until: Some(Utc.with_ymd_and_hms(2026, 3, 15, 0, 0, 0).unwrap()),
            ..Default::default()
        };

        let result = filters.apply(entries);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].timestamp, t1);
    }

    #[test]
    fn test_filter_by_date_range() {
        let t1 = Utc.with_ymd_and_hms(2026, 3, 5, 12, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2026, 3, 15, 12, 0, 0).unwrap();
        let t3 = Utc.with_ymd_and_hms(2026, 3, 25, 12, 0, 0).unwrap();

        let entries = vec![
            make_entry("opus", "/project", t1),
            make_entry("opus", "/project", t2),
            make_entry("opus", "/project", t3),
        ];

        let filters = Filters {
            since: Some(Utc.with_ymd_and_hms(2026, 3, 10, 0, 0, 0).unwrap()),
            until: Some(Utc.with_ymd_and_hms(2026, 3, 20, 0, 0, 0).unwrap()),
            ..Default::default()
        };

        let result = filters.apply(entries);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].timestamp, t2);
    }

    #[test]
    fn test_filter_by_model_substring() {
        let ts = Utc::now();
        let entries = vec![
            make_entry("claude-opus-4-6", "/project", ts),
            make_entry("claude-sonnet-4-6", "/project", ts),
            make_entry("claude-haiku-4-5", "/project", ts),
        ];

        let filters = Filters {
            model: Some("opus".to_string()),
            ..Default::default()
        };

        let result = filters.apply(entries);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].model, "claude-opus-4-6");
    }

    #[test]
    fn test_filter_by_model_case_insensitive() {
        let ts = Utc::now();
        let entries = vec![
            make_entry("claude-opus-4-6", "/project", ts),
            make_entry("claude-sonnet-4-6", "/project", ts),
        ];

        let filters = Filters {
            model: Some("OPUS".to_string()),
            ..Default::default()
        };

        let result = filters.apply(entries);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_filter_by_model_no_match() {
        let ts = Utc::now();
        let entries = vec![make_entry("claude-opus-4-6", "/project", ts)];

        let filters = Filters {
            model: Some("gemini".to_string()),
            ..Default::default()
        };

        let result = filters.apply(entries);
        assert!(result.is_empty());
    }

    #[test]
    fn test_filter_by_project_substring() {
        let ts = Utc::now();
        let entries = vec![
            make_entry("opus", "/home/user/my-saas-app", ts),
            make_entry("opus", "/home/user/cc-metrics", ts),
            make_entry("opus", "/home/user/dotfiles", ts),
        ];

        let filters = Filters {
            project: Some("saas".to_string()),
            ..Default::default()
        };

        let result = filters.apply(entries);
        assert_eq!(result.len(), 1);
        assert!(result[0].project_path.contains("saas"));
    }

    #[test]
    fn test_filter_by_project_case_insensitive() {
        let ts = Utc::now();
        let entries = vec![make_entry("opus", "/home/user/MyProject", ts)];

        let filters = Filters {
            project: Some("myproject".to_string()),
            ..Default::default()
        };

        let result = filters.apply(entries);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_filters_compose() {
        let t1 = Utc.with_ymd_and_hms(2026, 3, 10, 12, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2026, 3, 20, 12, 0, 0).unwrap();

        let entries = vec![
            make_entry("claude-opus-4-6", "/project-a", t1), // old opus
            make_entry("claude-opus-4-6", "/project-a", t2), // recent opus ← match
            make_entry("claude-sonnet-4-6", "/project-a", t2), // recent sonnet
            make_entry("claude-opus-4-6", "/project-b", t2), // recent opus, wrong project
        ];

        let filters = Filters {
            since: Some(Utc.with_ymd_and_hms(2026, 3, 15, 0, 0, 0).unwrap()),
            model: Some("opus".to_string()),
            project: Some("project-a".to_string()),
            ..Default::default()
        };

        let result = filters.apply(entries);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].model, "claude-opus-4-6");
        assert_eq!(result[0].timestamp, t2);
        assert!(result[0].project_path.contains("project-a"));
    }

    #[test]
    fn test_no_filters_passes_all() {
        let ts = Utc::now();
        let entries = vec![make_entry("opus", "/a", ts), make_entry("sonnet", "/b", ts)];

        let filters = Filters::default();
        let result = filters.apply(entries);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_filters_describe() {
        let filters = Filters {
            since: Some(Utc.with_ymd_and_hms(2026, 3, 15, 0, 0, 0).unwrap()),
            model: Some("opus".to_string()),
            ..Default::default()
        };

        let desc = filters.describe();
        assert!(desc.contains("since 2026-03-15"));
        assert!(desc.contains("model: opus"));
    }

    #[test]
    fn test_filters_is_active() {
        assert!(!Filters::default().is_active());
        assert!(Filters {
            model: Some("opus".to_string()),
            ..Default::default()
        }
        .is_active());
    }
}
