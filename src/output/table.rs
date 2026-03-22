use comfy_table::{Attribute, Cell, CellAlignment, Color, ContentArrangement, Table};

use crate::filters::Filters;
use crate::types::Summary;

/// Format the summary as a terminal table.
pub fn render(summary: &Summary, filters: &Filters) -> String {
    let mut output = String::new();

    // Header
    if filters.is_active() {
        output.push_str(&format!(
            "ccmetrics v{} — {} days, {} sessions, {} projects (filtered: {})\n\n",
            summary.version,
            summary.days,
            summary.sessions,
            summary.projects,
            filters.describe()
        ));
    } else {
        output.push_str(&format!(
            "ccmetrics v{} — {} days, {} sessions, {} projects\n\n",
            summary.version, summary.days, summary.sessions, summary.projects
        ));
    }

    // Token breakdown table
    let mut token_table = Table::new();
    token_table.set_content_arrangement(ContentArrangement::Dynamic);
    token_table.load_preset(comfy_table::presets::UTF8_FULL_CONDENSED);

    token_table.set_header(vec![
        Cell::new("Token Breakdown").add_attribute(Attribute::Bold),
        Cell::new("Count").add_attribute(Attribute::Bold),
        Cell::new("%").add_attribute(Attribute::Bold),
        Cell::new("Cost").add_attribute(Attribute::Bold),
    ]);

    let total_tokens = summary.input_tokens
        + summary.output_tokens
        + summary.cache_read_tokens
        + summary.cache_write_5m_tokens
        + summary.cache_write_1h_tokens;

    let rows = [
        ("Input tokens", summary.input_tokens, summary.cost.input),
        ("Output tokens", summary.output_tokens, summary.cost.output),
        (
            "Cache read",
            summary.cache_read_tokens,
            summary.cost.cache_read,
        ),
        (
            "Cache write (5m)",
            summary.cache_write_5m_tokens,
            summary.cost.cache_write_5m,
        ),
        (
            "Cache write (1h)",
            summary.cache_write_1h_tokens,
            summary.cost.cache_write_1h,
        ),
    ];

    for (label, count, cost) in &rows {
        let pct = if total_tokens > 0 {
            *count as f64 / total_tokens as f64 * 100.0
        } else {
            0.0
        };

        token_table.add_row(vec![
            Cell::new(label),
            Cell::new(format_number(*count)).set_alignment(CellAlignment::Right),
            Cell::new(format!("{:.2}%", pct)).set_alignment(CellAlignment::Right),
            Cell::new(format_dollar(*cost)).set_alignment(CellAlignment::Right),
        ]);
    }

    // Total row
    token_table.add_row(vec![
        Cell::new("Total")
            .add_attribute(Attribute::Bold)
            .fg(Color::Cyan),
        Cell::new(format_number(total_tokens))
            .set_alignment(CellAlignment::Right)
            .add_attribute(Attribute::Bold)
            .fg(Color::Cyan),
        Cell::new("100.00%")
            .set_alignment(CellAlignment::Right)
            .add_attribute(Attribute::Bold)
            .fg(Color::Cyan),
        Cell::new(format_dollar(summary.cost.total()))
            .set_alignment(CellAlignment::Right)
            .add_attribute(Attribute::Bold)
            .fg(Color::Cyan),
    ]);

    output.push_str(&token_table.to_string());
    output.push('\n');

    // Main vs Subagent table
    let total_requests = summary.main_requests + summary.subagent_requests;
    let main_pct = if total_requests > 0 {
        summary.main_requests as f64 / total_requests as f64 * 100.0
    } else {
        0.0
    };
    let sub_pct = 100.0 - main_pct;

    let mut split_table = Table::new();
    split_table.set_content_arrangement(ContentArrangement::Dynamic);
    split_table.load_preset(comfy_table::presets::UTF8_FULL_CONDENSED);

    split_table.set_header(vec![
        Cell::new("Main vs Subagent").add_attribute(Attribute::Bold),
        Cell::new("Requests").add_attribute(Attribute::Bold),
        Cell::new("In+Out Tokens").add_attribute(Attribute::Bold),
        Cell::new("Cost").add_attribute(Attribute::Bold),
    ]);

    split_table.add_row(vec![
        Cell::new(format!("Main thread ({:.0}%)", main_pct)),
        Cell::new(format_number(summary.main_requests as u64)).set_alignment(CellAlignment::Right),
        Cell::new(format_number(summary.main_input_output_tokens))
            .set_alignment(CellAlignment::Right),
        Cell::new(format_dollar(summary.main_cost)).set_alignment(CellAlignment::Right),
    ]);

    split_table.add_row(vec![
        Cell::new(format!("Subagents ({:.0}%)", sub_pct)),
        Cell::new(format_number(summary.subagent_requests as u64))
            .set_alignment(CellAlignment::Right),
        Cell::new(format_number(summary.subagent_input_output_tokens))
            .set_alignment(CellAlignment::Right),
        Cell::new(format_dollar(summary.subagent_cost)).set_alignment(CellAlignment::Right),
    ]);

    output.push('\n');
    output.push_str(&split_table.to_string());
    output.push('\n');

    // By Model table (only shown when 2+ models are present)
    if summary.by_model.len() >= 2 {
        let mut model_table = Table::new();
        model_table.set_content_arrangement(ContentArrangement::Dynamic);
        model_table.load_preset(comfy_table::presets::UTF8_FULL_CONDENSED);

        model_table.set_header(vec![
            Cell::new("By Model").add_attribute(Attribute::Bold),
            Cell::new("Requests").add_attribute(Attribute::Bold),
            Cell::new("In+Out Tokens").add_attribute(Attribute::Bold),
            Cell::new("Cost").add_attribute(Attribute::Bold),
        ]);

        for m in &summary.by_model {
            model_table.add_row(vec![
                Cell::new(&m.model),
                Cell::new(format_number(m.requests as u64)).set_alignment(CellAlignment::Right),
                Cell::new(format_number(m.input_tokens + m.output_tokens))
                    .set_alignment(CellAlignment::Right),
                Cell::new(format_dollar(m.cost)).set_alignment(CellAlignment::Right),
            ]);
        }

        output.push('\n');
        output.push_str(&model_table.to_string());
        output.push('\n');
    }

    // Footer
    output.push_str(&format!(
        "\nDedup: {} assistant entries → {} unique requests ({:.2}x reduction)\n",
        format_number(summary.raw_lines as u64),
        format_number(summary.unique_requests as u64),
        summary.dedup_ratio
    ));
    output.push_str("Pricing: Anthropic rates as of 2026-03-22 (embedded, no network)\n");

    output
}

/// Format a number with thousand separators.
pub fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

/// Format a dollar amount with alignment padding.
fn format_dollar(amount: f64) -> String {
    if amount >= 0.01 {
        format!("${:.2}", amount)
    } else if amount > 0.0 {
        format!("${:.3}", amount)
    } else {
        "$0.00".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(999), "999");
        assert_eq!(format_number(1_000), "1,000");
        assert_eq!(format_number(1_140_000), "1,140,000");
        assert_eq!(format_number(2_730_000_000), "2,730,000,000");
    }

    #[test]
    fn test_format_dollar() {
        assert_eq!(format_dollar(5.70), "$5.70");
        assert_eq!(format_dollar(153.00), "$153.00");
        assert_eq!(format_dollar(0.50), "$0.50");
        assert_eq!(format_dollar(0.001), "$0.001");
        assert_eq!(format_dollar(0.0), "$0.00");
    }
}
