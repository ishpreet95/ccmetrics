use comfy_table::{Attribute, Cell, CellAlignment, ContentArrangement, Table};
use serde::Serialize;

use crate::filters::Filters;
use crate::types::SessionBreakdown;

use super::round2;
use super::table::{format_dollar, format_number};

const DEFAULT_LIST_LIMIT: usize = 20;
const SESSION_ID_DISPLAY_LEN: usize = 8;

/// Truncate a session ID for display, safe for multi-byte UTF-8.
fn truncate_id(id: &str) -> String {
    let char_count = id.chars().count();
    if char_count > SESSION_ID_DISPLAY_LEN {
        let truncated: String = id.chars().take(SESSION_ID_DISPLAY_LEN).collect();
        format!("{}...", truncated)
    } else {
        id.to_string()
    }
}

/// Extract the last path component for display.
fn short_project(project: &str) -> &str {
    std::path::Path::new(project)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(project)
}

/// Format a duration in minutes as "Xh Ym" or "Ym".
fn format_duration(minutes: u64) -> String {
    if minutes >= 60 {
        format!("{}h {}m", minutes / 60, minutes % 60)
    } else {
        format!("{}m", minutes)
    }
}

/// Render session list as a terminal table.
pub fn render_list(sessions: &[SessionBreakdown], version: &str, filters: &Filters) -> String {
    let mut output = String::new();

    if filters.is_active() {
        output.push_str(&format!(
            "ccmetrics v{} — Recent sessions (filtered: {})\n\n",
            version,
            filters.describe()
        ));
    } else {
        output.push_str(&format!("ccmetrics v{} — Recent sessions\n\n", version));
    }

    if sessions.is_empty() {
        output.push_str("No sessions found.\n");
        return output;
    }

    let display = &sessions[..sessions.len().min(DEFAULT_LIST_LIMIT)];

    let mut table = Table::new();
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.load_preset(comfy_table::presets::UTF8_FULL_CONDENSED);

    table.set_header(vec![
        Cell::new("Session ID").add_attribute(Attribute::Bold),
        Cell::new("Date").add_attribute(Attribute::Bold),
        Cell::new("Project").add_attribute(Attribute::Bold),
        Cell::new("Requests").add_attribute(Attribute::Bold),
        Cell::new("Model").add_attribute(Attribute::Bold),
        Cell::new("Cost").add_attribute(Attribute::Bold),
    ]);

    for s in display {
        table.add_row(vec![
            Cell::new(truncate_id(&s.session_id)),
            Cell::new(&s.date),
            Cell::new(short_project(&s.project)),
            Cell::new(format_number(s.requests as u64)).set_alignment(CellAlignment::Right),
            Cell::new(&s.primary_model),
            Cell::new(format_dollar(s.cost)).set_alignment(CellAlignment::Right),
        ]);
    }

    output.push_str(&table.to_string());
    output.push('\n');

    if sessions.len() > DEFAULT_LIST_LIMIT {
        output.push_str(&format!(
            "\nShowing {} of {}. Use --since to narrow.\n",
            DEFAULT_LIST_LIMIT,
            sessions.len()
        ));
    }

    output
}

/// Render session detail view.
pub fn render_detail(session: &SessionBreakdown) -> String {
    let mut output = String::new();

    output.push_str(&format!(
        "Session {} — {}, project: {}\n\n",
        truncate_id(&session.session_id),
        session.date,
        short_project(&session.project),
    ));

    // Token breakdown for this session — all from SessionBreakdown
    let mut table = Table::new();
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.load_preset(comfy_table::presets::UTF8_FULL_CONDENSED);

    table.set_header(vec![
        Cell::new("Token Breakdown").add_attribute(Attribute::Bold),
        Cell::new("Count").add_attribute(Attribute::Bold),
    ]);

    let rows = [
        ("Input tokens", session.input_tokens),
        ("Output tokens", session.output_tokens),
        ("Cache read", session.cache_read_tokens),
        ("Cache write (5m)", session.cache_write_5m_tokens),
        ("Cache write (1h)", session.cache_write_1h_tokens),
    ];

    for (label, count) in &rows {
        table.add_row(vec![
            Cell::new(label),
            Cell::new(format_number(*count)).set_alignment(CellAlignment::Right),
        ]);
    }

    output.push_str(&table.to_string());
    output.push('\n');

    // Summary line with duration
    let duration_part = session
        .duration_minutes
        .filter(|&m| m > 0)
        .map(|m| format!(" over {}", format_duration(m)))
        .unwrap_or_default();
    let sub_part = if session.subagent_spawns > 0 {
        format!(". {} subagent spawns", session.subagent_spawns)
    } else {
        String::new()
    };
    output.push_str(&format!(
        "\n{} requests{}, ${:.2} total{}\n",
        session.requests, duration_part, session.cost, sub_part
    ));

    output
}

// --- JSON rendering ---

#[derive(Serialize)]
struct SessionListJsonOutput {
    version: String,
    generated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    filter: Option<super::json::FilterInfo>,
    sessions: Vec<SessionJsonEntry>,
}

#[derive(Serialize)]
struct SessionJsonEntry {
    session_id: String,
    date: String,
    project: String,
    requests: usize,
    subagent_spawns: usize,
    primary_model: String,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_write_5m_tokens: u64,
    cache_write_1h_tokens: u64,
    cost: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_minutes: Option<u64>,
}

fn build_filter_info(filters: &Filters) -> Option<super::json::FilterInfo> {
    if filters.is_active() {
        Some(super::json::FilterInfo {
            since: filters.since.map(|dt| dt.to_rfc3339()),
            until: filters.until.map(|dt| dt.to_rfc3339()),
            model: filters.model.clone(),
            project: filters.project.clone(),
        })
    } else {
        None
    }
}

fn session_to_json(s: &SessionBreakdown) -> SessionJsonEntry {
    SessionJsonEntry {
        session_id: s.session_id.clone(),
        date: s.date.clone(),
        project: s.project.clone(),
        requests: s.requests,
        subagent_spawns: s.subagent_spawns,
        primary_model: s.primary_model.clone(),
        input_tokens: s.input_tokens,
        output_tokens: s.output_tokens,
        cache_read_tokens: s.cache_read_tokens,
        cache_write_5m_tokens: s.cache_write_5m_tokens,
        cache_write_1h_tokens: s.cache_write_1h_tokens,
        cost: round2(s.cost),
        duration_minutes: s.duration_minutes,
    }
}

/// Render session list as JSON.
pub fn render_list_json(
    sessions: &[SessionBreakdown],
    version: &str,
    filters: &Filters,
) -> serde_json::Result<String> {
    let output = SessionListJsonOutput {
        version: version.to_string(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        filter: build_filter_info(filters),
        sessions: sessions.iter().map(session_to_json).collect(),
    };

    serde_json::to_string_pretty(&output)
}

/// Render a single session detail as JSON.
pub fn render_detail_json(
    session: &SessionBreakdown,
    version: &str,
    filters: &Filters,
) -> serde_json::Result<String> {
    #[derive(Serialize)]
    struct SessionDetailJsonOutput {
        version: String,
        generated_at: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        filter: Option<super::json::FilterInfo>,
        session: SessionJsonEntry,
    }

    let output = SessionDetailJsonOutput {
        version: version.to_string(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        filter: build_filter_info(filters),
        session: session_to_json(session),
    };

    serde_json::to_string_pretty(&output)
}
