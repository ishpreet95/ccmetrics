use std::io::BufRead;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::types::{Speed, Warning};

/// A raw parsed entry before deduplication.
/// Multiple RawEntries may share the same request_id (streaming chunks).
#[derive(Debug, Clone)]
pub struct RawEntry {
    pub request_id: Option<String>,
    pub message_id: Option<String>,
    pub session_id: String,
    pub model: String,
    pub is_sidechain: bool,
    pub timestamp: DateTime<Utc>,
    pub stop_reason: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_write_5m_tokens: u64,
    pub cache_write_1h_tokens: u64,
    pub speed: Speed,
    pub inference_geo: Option<String>,
    pub web_search_requests: u32,
    pub web_fetch_requests: u32,
    pub source_file: std::path::PathBuf,
    pub project_path: String,
    pub line_number: usize,
}

/// Parse result for a single file.
pub struct ParseResult {
    pub entries: Vec<RawEntry>,
    pub warnings: Vec<Warning>,
    pub raw_line_count: usize,
    pub skipped_lines: usize,
    pub synthetic_count: usize,
    pub assistant_lines: usize,
}

/// Parse all assistant entries from a JSONL file.
pub fn parse_jsonl_file(path: &Path, project_path: &str) -> ParseResult {
    let mut entries = Vec::new();
    let mut warnings = Vec::new();
    let mut raw_line_count = 0;
    let mut skipped_lines = 0;
    let mut synthetic_count = 0;
    let mut assistant_lines = 0;

    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => {
            warnings.push(Warning {
                file: path.to_path_buf(),
                line: None,
                message: format!("Failed to open: {e}"),
            });
            return ParseResult {
                entries,
                warnings,
                raw_line_count: 0,
                skipped_lines: 0,
                synthetic_count: 0,
                assistant_lines: 0,
            };
        }
    };

    let reader = std::io::BufReader::new(file);
    let is_subagent_file = crate::scanner::is_subagent_path(path);

    for (idx, line) in reader.lines().enumerate() {
        let line_number = idx + 1;
        raw_line_count += 1;

        let line = match line {
            Ok(l) => l,
            Err(e) => {
                skipped_lines += 1;
                warnings.push(Warning {
                    file: path.to_path_buf(),
                    line: Some(line_number),
                    message: format!("Read error: {e}"),
                });
                continue;
            }
        };

        if line.trim().is_empty() {
            continue;
        }

        let value: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => {
                skipped_lines += 1;
                continue;
            }
        };

        // Only process assistant messages
        if value.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }

        let message = match value.get("message") {
            Some(m) => m,
            None => continue,
        };

        // Skip synthetic messages
        let model = message.get("model").and_then(Value::as_str).unwrap_or("");
        if model == "<synthetic>" {
            synthetic_count += 1;
            continue;
        }
        if model.is_empty() {
            continue;
        }

        assistant_lines += 1;

        let usage = match message.get("usage") {
            Some(u) => u,
            None => continue,
        };

        // Extract fields
        let request_id = value
            .get("requestId")
            .and_then(Value::as_str)
            .map(String::from);
        let message_id = message.get("id").and_then(Value::as_str).map(String::from);
        let session_id = value
            .get("sessionId")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        // Use cwd from JSONL entry as project path (more accurate than directory decoding)
        let entry_project_path = value
            .get("cwd")
            .and_then(Value::as_str)
            .unwrap_or(project_path)
            .to_string();

        let stop_reason = message
            .get("stop_reason")
            .and_then(Value::as_str)
            .map(String::from);

        let is_sidechain = value
            .get("isSidechain")
            .and_then(Value::as_bool)
            .unwrap_or(is_subagent_file);

        let timestamp = match value.get("timestamp").and_then(Value::as_str) {
            Some(s) => match s.parse::<DateTime<Utc>>() {
                Ok(t) => t,
                Err(_) => {
                    // Use epoch as sentinel so it doesn't corrupt date range
                    DateTime::UNIX_EPOCH
                }
            },
            None => DateTime::UNIX_EPOCH,
        };

        // Token counts
        let input_tokens = usage
            .get("input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let output_tokens = usage
            .get("output_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let cache_read_input_tokens = usage
            .get("cache_read_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0);

        // Cache write tokens: prefer nested tiered fields, fall back to flat total
        let (cache_write_5m, cache_write_1h) = extract_cache_write_tokens(usage);

        // Speed
        let speed = match usage.get("speed").and_then(Value::as_str) {
            Some("fast") => Speed::Fast,
            _ => Speed::Standard,
        };

        // Inference geo
        let inference_geo = usage
            .get("inference_geo")
            .and_then(Value::as_str)
            .and_then(|s| {
                if s.is_empty() || s == "not_available" {
                    None
                } else {
                    Some(s.to_string())
                }
            });

        // Server tool use
        let server_tool_use = usage.get("server_tool_use");
        let web_search_requests = server_tool_use
            .and_then(|s| s.get("web_search_requests"))
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32;
        let web_fetch_requests = server_tool_use
            .and_then(|s| s.get("web_fetch_requests"))
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32;

        entries.push(RawEntry {
            request_id,
            message_id,
            session_id,
            model: model.to_string(),
            is_sidechain,
            timestamp,
            stop_reason,
            input_tokens,
            output_tokens,
            cache_read_input_tokens,
            cache_write_5m_tokens: cache_write_5m,
            cache_write_1h_tokens: cache_write_1h,
            speed,
            inference_geo,
            web_search_requests,
            web_fetch_requests,
            source_file: path.to_path_buf(),
            project_path: entry_project_path,
            line_number,
        });
    }

    ParseResult {
        entries,
        warnings,
        raw_line_count,
        skipped_lines,
        synthetic_count,
        assistant_lines,
    }
}

/// Extract cache write tokens from the usage object.
///
/// Prefers nested `cache_creation.ephemeral_5m_input_tokens` / `ephemeral_1h_input_tokens`.
/// Falls back to flat `cache_creation_input_tokens` (assigned to 1h tier for backward compat).
fn extract_cache_write_tokens(usage: &Value) -> (u64, u64) {
    if let Some(cache_creation) = usage.get("cache_creation") {
        let t5m = cache_creation
            .get("ephemeral_5m_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let t1h = cache_creation
            .get("ephemeral_1h_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if t5m > 0 || t1h > 0 {
            return (t5m, t1h);
        }
    }

    // Fallback: flat field, assign to 1h tier
    let flat_total = usage
        .get("cache_creation_input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    (0, flat_total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_cache_write_tokens_nested() {
        let usage: Value = serde_json::json!({
            "cache_creation_input_tokens": 49043,
            "cache_creation": {
                "ephemeral_5m_input_tokens": 43043,
                "ephemeral_1h_input_tokens": 6000
            }
        });
        let (t5m, t1h) = extract_cache_write_tokens(&usage);
        assert_eq!(t5m, 43043);
        assert_eq!(t1h, 6000);
    }

    #[test]
    fn test_extract_cache_write_tokens_flat_fallback() {
        let usage: Value = serde_json::json!({
            "cache_creation_input_tokens": 49043
        });
        let (t5m, t1h) = extract_cache_write_tokens(&usage);
        assert_eq!(t5m, 0);
        assert_eq!(t1h, 49043);
    }

    #[test]
    fn test_extract_cache_write_tokens_nested_zeros_fallback() {
        let usage: Value = serde_json::json!({
            "cache_creation_input_tokens": 49043,
            "cache_creation": {
                "ephemeral_5m_input_tokens": 0,
                "ephemeral_1h_input_tokens": 0
            }
        });
        let (t5m, t1h) = extract_cache_write_tokens(&usage);
        // Both nested are zero, fall back to flat
        assert_eq!(t5m, 0);
        assert_eq!(t1h, 49043);
    }
}
