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

    let total_tokens = summary
        .input_tokens
        .saturating_add(summary.output_tokens)
        .saturating_add(summary.cache_read_tokens)
        .saturating_add(summary.cache_write_5m_tokens)
        .saturating_add(summary.cache_write_1h_tokens);

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
            Cell::new(format_abbreviated(*count)).set_alignment(CellAlignment::Right),
            Cell::new(format!("{:.2}%", pct)).set_alignment(CellAlignment::Right),
            Cell::new(format_dollar(*cost)).set_alignment(CellAlignment::Right),
        ]);
    }

    // Total row
    token_table.add_row(vec![
        Cell::new("Total")
            .add_attribute(Attribute::Bold)
            .fg(Color::Cyan),
        Cell::new(format_abbreviated(total_tokens))
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
        Cell::new(format_abbreviated(summary.main_input_output_tokens))
            .set_alignment(CellAlignment::Right),
        Cell::new(format_dollar(summary.main_cost)).set_alignment(CellAlignment::Right),
    ]);

    split_table.add_row(vec![
        Cell::new(format!("Subagents ({:.0}%)", sub_pct)),
        Cell::new(format_number(summary.subagent_requests as u64))
            .set_alignment(CellAlignment::Right),
        Cell::new(format_abbreviated(summary.subagent_input_output_tokens))
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
                Cell::new(format_abbreviated(
                    m.input_tokens.saturating_add(m.output_tokens),
                ))
                .set_alignment(CellAlignment::Right),
                Cell::new(format_dollar(m.cost)).set_alignment(CellAlignment::Right),
            ]);
        }

        output.push('\n');
        output.push_str(&model_table.to_string());
        output.push('\n');
    }

    // By Project table (only shown when 2+ projects are present)
    if summary.by_project.len() >= 2 {
        let mut project_table = Table::new();
        project_table.set_content_arrangement(ContentArrangement::Dynamic);
        project_table.load_preset(comfy_table::presets::UTF8_FULL_CONDENSED);

        project_table.set_header(vec![
            Cell::new("By Project").add_attribute(Attribute::Bold),
            Cell::new("Sessions").add_attribute(Attribute::Bold),
            Cell::new("Requests").add_attribute(Attribute::Bold),
            Cell::new("In+Out Tokens").add_attribute(Attribute::Bold),
            Cell::new("Cost").add_attribute(Attribute::Bold),
        ]);

        for p in &summary.by_project {
            project_table.add_row(vec![
                Cell::new(&p.project),
                Cell::new(format_number(p.sessions as u64)).set_alignment(CellAlignment::Right),
                Cell::new(format_number(p.requests as u64)).set_alignment(CellAlignment::Right),
                Cell::new(format_abbreviated(
                    p.input_tokens.saturating_add(p.output_tokens),
                ))
                .set_alignment(CellAlignment::Right),
                Cell::new(format_dollar(p.cost)).set_alignment(CellAlignment::Right),
            ]);
        }

        output.push('\n');
        output.push_str(&project_table.to_string());
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

/// Format a number with thousand separators (exact).
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

/// Format a number in abbreviated form: 2.86B, 6.1M, 260K.
///
/// Uses 2-3 significant figures, trims trailing zeros.
/// Numbers below 1,000 are shown as-is.
pub fn format_abbreviated(n: u64) -> String {
    if n < 1_000 {
        return n.to_string();
    }

    let (value, suffix) = if n >= 1_000_000_000 {
        (n as f64 / 1_000_000_000.0, "B")
    } else if n >= 1_000_000 {
        (n as f64 / 1_000_000.0, "M")
    } else {
        (n as f64 / 1_000.0, "K")
    };

    // If rounding will push us to 1000+, promote to the next tier
    if value >= 999.5 {
        return match suffix {
            "K" => "1M".to_string(),
            "M" => "1B".to_string(),
            // B tier: no higher suffix, show as-is
            _ => format!("{:.0}B", value),
        };
    }

    // Precision: target 2-3 significant figures
    let formatted = if value >= 100.0 {
        format!("{:.0}", value)
    } else if value >= 10.0 {
        format!("{:.1}", value)
    } else {
        format!("{:.2}", value)
    };

    // Trim trailing zeros after decimal point
    let trimmed = if formatted.contains('.') {
        let t = formatted.trim_end_matches('0').trim_end_matches('.');
        t.to_string()
    } else {
        formatted
    };

    format!("{}{}", trimmed, suffix)
}

/// Format a dollar amount with alignment padding.
pub fn format_dollar(amount: f64) -> String {
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
    fn test_format_abbreviated() {
        // Below 1K: exact
        assert_eq!(format_abbreviated(0), "0");
        assert_eq!(format_abbreviated(1), "1");
        assert_eq!(format_abbreviated(999), "999");

        // Thousands
        assert_eq!(format_abbreviated(1_000), "1K");
        assert_eq!(format_abbreviated(1_140), "1.14K");
        assert_eq!(format_abbreviated(1_500), "1.5K");
        assert_eq!(format_abbreviated(10_500), "10.5K");
        assert_eq!(format_abbreviated(260_000), "260K");

        // Millions
        assert_eq!(format_abbreviated(1_000_000), "1M");
        assert_eq!(format_abbreviated(1_140_000), "1.14M");
        assert_eq!(format_abbreviated(6_100_000), "6.1M");
        assert_eq!(format_abbreviated(153_000_000), "153M");

        // Billions
        assert_eq!(format_abbreviated(1_000_000_000), "1B");
        assert_eq!(format_abbreviated(2_730_000_000), "2.73B");
        assert_eq!(format_abbreviated(2_860_000_000), "2.86B");
    }

    #[test]
    fn test_format_abbreviated_tier_promotion() {
        // Rounding at K→M boundary
        assert_eq!(format_abbreviated(999_500), "1M");
        assert_eq!(format_abbreviated(999_999), "1M");

        // Rounding at M→B boundary
        assert_eq!(format_abbreviated(999_500_000), "1B");
        assert_eq!(format_abbreviated(999_999_999), "1B");

        // Just below rounding threshold stays in current tier
        assert_eq!(format_abbreviated(999_499), "999K");
        assert_eq!(format_abbreviated(999_499_999), "999M");

        // Very large values (trillions) — no T tier, stays as B
        assert_eq!(format_abbreviated(1_000_000_000_000), "1000B");

        // u64::MAX doesn't panic
        let _ = format_abbreviated(u64::MAX);
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
