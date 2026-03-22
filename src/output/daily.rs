use comfy_table::{Attribute, Cell, CellAlignment, Color, ContentArrangement, Table};
use serde::Serialize;

use crate::filters::Filters;
use crate::types::DayBreakdown;

use super::round2;
use super::table::{format_abbreviated, format_dollar, format_number};

/// Render the daily view as a terminal table.
pub fn render(days: &[DayBreakdown], version: &str, filters: &Filters) -> String {
    let mut output = String::new();

    // Header
    if filters.is_active() {
        output.push_str(&format!(
            "ccmetrics v{} — Daily breakdown (filtered: {})\n\n",
            version,
            filters.describe()
        ));
    } else {
        output.push_str(&format!("ccmetrics v{} — Daily breakdown\n\n", version));
    }

    if days.is_empty() {
        output.push_str("No data for the selected period.\n");
        return output;
    }

    let mut table = Table::new();
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.load_preset(comfy_table::presets::UTF8_FULL_CONDENSED);

    table.set_header(vec![
        Cell::new("Date").add_attribute(Attribute::Bold),
        Cell::new("Requests").add_attribute(Attribute::Bold),
        Cell::new("In+Out Tokens").add_attribute(Attribute::Bold),
        Cell::new("Cost").add_attribute(Attribute::Bold),
    ]);

    let mut total_requests: usize = 0;
    let mut total_io_tokens: u64 = 0;
    let mut total_cost: f64 = 0.0;

    for day in days {
        let io_tokens = day.input_tokens.saturating_add(day.output_tokens);
        total_requests += day.requests;
        total_io_tokens += io_tokens;
        total_cost += day.cost;

        table.add_row(vec![
            Cell::new(&day.date),
            Cell::new(format_number(day.requests as u64)).set_alignment(CellAlignment::Right),
            Cell::new(format_abbreviated(io_tokens)).set_alignment(CellAlignment::Right),
            Cell::new(format_dollar(day.cost)).set_alignment(CellAlignment::Right),
        ]);
    }

    // Total row
    table.add_row(vec![
        Cell::new("Total")
            .add_attribute(Attribute::Bold)
            .fg(Color::DarkGreen),
        Cell::new(format_number(total_requests as u64))
            .set_alignment(CellAlignment::Right)
            .add_attribute(Attribute::Bold)
            .fg(Color::DarkGreen),
        Cell::new(format_abbreviated(total_io_tokens))
            .set_alignment(CellAlignment::Right)
            .add_attribute(Attribute::Bold)
            .fg(Color::DarkGreen),
        Cell::new(format_dollar(total_cost))
            .set_alignment(CellAlignment::Right)
            .add_attribute(Attribute::Bold)
            .fg(Color::DarkGreen),
    ]);

    output.push_str(&table.to_string());
    output.push('\n');

    // Average line
    let num_days = days.len();
    let avg_requests = total_requests as f64 / num_days as f64;
    let avg_cost = total_cost / num_days as f64;
    output.push_str(&format!(
        "\nAvg: {:.0} requests/day, ${:.2}/day\n",
        avg_requests, avg_cost
    ));

    output
}

#[derive(Serialize)]
struct DailyJsonOutput {
    version: String,
    generated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    filter: Option<super::json::FilterInfo>,
    daily: Vec<DailyJsonEntry>,
}

#[derive(Serialize)]
struct DailyJsonEntry {
    date: String,
    requests: usize,
    input_tokens: u64,
    output_tokens: u64,
    cost: f64,
}

/// Render the daily view as JSON.
pub fn render_json(
    days: &[DayBreakdown],
    version: &str,
    filters: &Filters,
) -> serde_json::Result<String> {
    let filter_info = if filters.is_active() {
        Some(super::json::FilterInfo {
            since: filters.since.map(|dt| dt.to_rfc3339()),
            until: filters.until.map(|dt| dt.to_rfc3339()),
            model: filters.model.clone(),
            project: filters.project.clone(),
        })
    } else {
        None
    };

    let output = DailyJsonOutput {
        version: version.to_string(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        filter: filter_info,
        daily: days
            .iter()
            .map(|d| DailyJsonEntry {
                date: d.date.clone(),
                requests: d.requests,
                input_tokens: d.input_tokens,
                output_tokens: d.output_tokens,
                cost: round2(d.cost),
            })
            .collect(),
    };

    serde_json::to_string_pretty(&output)
}
