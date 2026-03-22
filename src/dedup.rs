use std::collections::HashMap;

use crate::parser::RawEntry;
use crate::types::UsageEntry;

/// Deduplicates raw entries by requestId, keeping the final chunk
/// (the one with `stop_reason` set).
///
/// Strategy:
/// 1. Group entries by their dedup key (requestId, falling back to message_id)
/// 2. For each group, prefer the entry with `stop_reason` set (final chunk)
/// 3. If no entry has stop_reason set (truncated session), keep the last by line order
/// 4. Entries with neither requestId nor message_id are kept as-is (counted once)
pub fn deduplicate(raw_entries: Vec<RawEntry>) -> (Vec<UsageEntry>, usize) {
    let mut groups: HashMap<String, Vec<RawEntry>> = HashMap::new();
    let mut no_id_entries: Vec<RawEntry> = Vec::new();
    let mut no_id_count = 0;

    for entry in raw_entries {
        // Key includes session_id to prevent cross-session collisions
        // (requestIds are unique per-session but could theoretically repeat across sessions)
        let id_part = if let Some(ref rid) = entry.request_id {
            rid.clone()
        } else if let Some(ref mid) = entry.message_id {
            mid.clone()
        } else {
            no_id_count += 1;
            no_id_entries.push(entry);
            continue;
        };
        let key = format!("{}:{}", entry.session_id, id_part);

        groups.entry(key).or_default().push(entry);
    }

    let mut deduped: Vec<UsageEntry> = Vec::with_capacity(groups.len() + no_id_entries.len());

    for (_key, group) in groups {
        let selected = select_final_chunk(group);
        deduped.push(raw_to_usage(selected));
    }

    // Add no-ID entries as-is
    for entry in no_id_entries {
        deduped.push(raw_to_usage(entry));
    }

    // Sort by timestamp for consistent output
    deduped.sort_by_key(|e| e.timestamp);

    (deduped, no_id_count)
}

/// Select the "final" chunk from a group of entries sharing the same requestId.
///
/// Prefers the entry with `stop_reason` set (the real final chunk with accurate token counts).
/// If none has stop_reason set (truncated session), takes the last by line number.
fn select_final_chunk(mut group: Vec<RawEntry>) -> RawEntry {
    // Sort by line number for deterministic order within a file.
    // NOTE: line_number is per-file; if a requestId somehow appears across multiple
    // files (which should not happen for Claude Code sessions), ordering is best-effort.
    group.sort_by_key(|e| e.line_number);

    // Find entry with stop_reason set
    if let Some(pos) = group.iter().rposition(|e| e.stop_reason.is_some()) {
        return group.swap_remove(pos);
    }

    // Fallback: last entry by line number
    group.pop().expect("group should not be empty")
}

fn raw_to_usage(raw: RawEntry) -> UsageEntry {
    UsageEntry {
        request_id: raw
            .request_id
            .or(raw.message_id)
            .unwrap_or_else(|| format!("unknown-{}:{}", raw.source_file.display(), raw.line_number)),
        session_id: raw.session_id,
        model: raw.model,
        is_sidechain: raw.is_sidechain,
        timestamp: raw.timestamp,
        input_tokens: raw.input_tokens,
        output_tokens: raw.output_tokens,
        cache_read_input_tokens: raw.cache_read_input_tokens,
        cache_write_5m_tokens: raw.cache_write_5m_tokens,
        cache_write_1h_tokens: raw.cache_write_1h_tokens,
        speed: raw.speed,
        inference_geo: raw.inference_geo,
        web_search_requests: raw.web_search_requests,
        web_fetch_requests: raw.web_fetch_requests,
        source_file: raw.source_file,
        project_path: raw.project_path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Speed;
    use chrono::Utc;
    use std::path::PathBuf;

    fn make_raw(request_id: &str, stop_reason: Option<&str>, output_tokens: u64, line: usize) -> RawEntry {
        RawEntry {
            request_id: Some(request_id.to_string()),
            message_id: Some("msg_123".to_string()),
            session_id: "session_1".to_string(),
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

    #[test]
    fn test_dedup_keeps_final_chunk() {
        let entries = vec![
            make_raw("req_1", None, 10, 1),      // intermediate, placeholder
            make_raw("req_1", None, 10, 2),      // intermediate, placeholder
            make_raw("req_1", Some("end_turn"), 365, 3), // final, real tokens
        ];

        let (deduped, _) = deduplicate(entries);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].output_tokens, 365);
    }

    #[test]
    fn test_dedup_no_stop_reason_keeps_last() {
        let entries = vec![
            make_raw("req_1", None, 8, 1),
            make_raw("req_1", None, 11, 2),
        ];

        let (deduped, _) = deduplicate(entries);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].output_tokens, 11);
    }

    #[test]
    fn test_dedup_separate_requests() {
        let entries = vec![
            make_raw("req_1", Some("end_turn"), 100, 1),
            make_raw("req_2", Some("tool_use"), 200, 2),
        ];

        let (deduped, _) = deduplicate(entries);
        assert_eq!(deduped.len(), 2);
    }
}
