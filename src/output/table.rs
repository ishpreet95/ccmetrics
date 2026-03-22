use crate::filters::Filters;
use crate::pricing;
use crate::types::Summary;

use super::style::{self, InsightLevel};

/// Format the summary as a 4-section dashboard.
pub fn render(summary: &Summary, filters: &Filters) -> String {
    let color = style::stdout_color();
    let width = style::terminal_width();
    let bw = style::bar_width(width);
    let bars = style::show_bars(width);

    let mut out = String::new();

    out.push_str(&render_header(summary, filters, color));
    out.push_str("\n\n");
    out.push_str(&render_work_section(summary, width, color));
    out.push_str("\n\n");
    out.push_str(&render_cost_section(summary, width, bw, bars, color));
    out.push_str("\n\n");
    out.push_str(&render_cache_section(summary, width, bw, bars, color));
    out.push_str("\n\n");
    out.push_str(&render_where_section(summary, width, bw, bars, color));
    out.push_str("\n\n");
    out.push_str(&render_footer(summary, color));
    out.push('\n');

    out
}

// ─── HEADER ─────────────────────────────

fn render_header(summary: &Summary, filters: &Filters, color: bool) -> String {
    let version = style::hero(&format!("ccmetrics v{}", summary.version), color);
    let total_requests = summary.main_requests + summary.subagent_requests;
    let stats = style::dim(
        &format!(
            "◇ {} days  ◇ {} sessions  ◇ {} projects  ◇ {} requests",
            summary.days, summary.sessions, summary.projects, total_requests,
        ),
        color,
    );

    let mut header = format!("  {version}    {stats}");

    if filters.is_active() {
        header.push_str(&format!(
            "\n  {}",
            style::dim(&format!("filtered: {}", filters.describe()), color)
        ));
    }

    header
}

// ─── SECTION RULE ───────────────────────

fn render_section_rule(title: &str, width: usize, color: bool) -> String {
    let rule_width = width.saturating_sub(2); // account for 2-char left indent
    let prefix = "───";
    let prefix_visible = 3; // "───" is 3 visible chars (each '─' is 3 bytes in UTF-8)
    let title_part = format!(" {} ", title);
    let title_visible = title_part.chars().count();
    let remaining = rule_width.saturating_sub(prefix_visible + title_visible);
    let suffix = "─".repeat(remaining);
    let rule = format!("{prefix}{title_part}{suffix}");

    format!(
        "  {}",
        if color {
            format!(
                "{}{prefix} {}{}{title}{} {suffix}{}",
                style::DIM,
                style::BOLD,
                style::ACCENT,
                style::DIM,
                style::RESET,
            )
        } else {
            rule
        }
    )
}

// ─── COST SECTION ───────────────────────

fn render_cost_section(summary: &Summary, width: usize, bw: usize, bars: bool, color: bool) -> String {
    let mut out = String::new();
    out.push_str(&render_section_rule("¤ COST", width, color));
    out.push('\n');

    // Hero: total cost (chip style, number only)
    let total_cost = summary.cost.total();
    let hero = style::chip(&format_dollar(total_cost), color);
    let subtitle = style::accent("API-equivalent estimate", color);
    out.push_str(&format!("\n  {hero}    {subtitle}\n"));

    // Cost rows: Output, Input, Cache read, Cache write
    // Cache read saves money (90% discount), cache write costs more than input (1.25-2x).
    // Showing them separately tells the real cost story.
    let cache_read_cost = summary.cost.cache_read;
    let cache_write_cost = summary.cost.cache_write_5m + summary.cost.cache_write_1h;
    let web_cost = summary.cost.web_search;

    let mut rows: Vec<(&str, f64)> = vec![
        ("Output", summary.cost.output),
        ("Input", summary.cost.input),
        ("Cache read", cache_read_cost),
    ];
    if cache_write_cost > 0.0 {
        // Compute as remainder to avoid rounding mismatch with hero total
        let remainder =
            total_cost - summary.cost.output - summary.cost.input - cache_read_cost - web_cost;
        rows.push(("Cache write", remainder));
    }
    if web_cost > 0.0 {
        rows.push(("Web search", web_cost));
    }

    out.push('\n');
    for (label, cost) in &rows {
        let pct = if total_cost > 0.0 {
            cost / total_cost * 100.0
        } else {
            0.0
        };
        let cost_str = style::value(&format!("{:>10}", format_dollar(*cost)), color);
        let bar_str = if bars {
            format!("    {}", style::render_bar(cost / total_cost, bw, color))
        } else {
            String::new()
        };
        let label_styled = style::dim(label, color);
        let label_pad = 12usize.saturating_sub(label.len());
        let pct_str = style::secondary(&format!("{:>5.1}%", pct), color);
        out.push_str(&format!(
            "  {label_styled}{} {cost_str}{bar_str}    {pct_str}\n",
            " ".repeat(label_pad),
        ));
    }

    // Insight: what drives cost
    let output_pct = if total_cost > 0.0 {
        summary.cost.output / total_cost * 100.0
    } else {
        0.0
    };
    if output_pct > 80.0 {
        out.push('\n');
        out.push_str(&style::render_insight(
            InsightLevel::Good,
            &format!(
                "Output tokens drive {:.0}% of your cost — the words Claude writes back",
                output_pct
            ),
            color,
        ));
        out.push('\n');
    }

    out
}

// ─── YOUR WORK SECTION ──────────────────

fn render_work_section(summary: &Summary, width: usize, color: bool) -> String {
    let mut out = String::new();
    out.push_str(&render_section_rule("⊡ YOUR WORK", width, color));
    out.push('\n');

    let io_tokens = summary.input_tokens.saturating_add(summary.output_tokens);
    let total_requests = summary.main_requests + summary.subagent_requests;

    // Hero: in+out total (chip style, number only)
    let hero = style::chip(&format_abbreviated(io_tokens), color);
    let companion = style::accent(
        &format!("tokens · {} requests", format_number(total_requests as u64)),
        color,
    );
    out.push_str(&format!("\n  {hero}  {companion}\n"));

    // 2 rows: Output, Input with descriptions
    let work_rows: Vec<(&str, u64, &str)> = vec![
        ("Output", summary.output_tokens, "what Claude wrote back"),
        (
            "Input",
            summary.input_tokens,
            "what you sent (prompts, tool results, context)",
        ),
    ];
    out.push('\n');
    for (label, tokens, desc) in &work_rows {
        let label_styled = style::dim(label, color);
        let label_pad = 14usize.saturating_sub(label.len());
        let token_str = format_abbreviated(*tokens);
        out.push_str(&format!(
            "  {label_styled}{}{}   {}\n",
            " ".repeat(label_pad),
            style::value(&format!("{:>8}", token_str), color),
            style::description(desc, color),
        ));
    }

    // Insights — dedup first (the differentiator), then total tokens
    out.push('\n');
    if summary.dedup_ratio > 1.5 {
        out.push_str(&style::render_insight(
            InsightLevel::Note,
            &format!(
                "{:.2}x dedup reduction — raw logs overcount by nearly {:.0}x",
                summary.dedup_ratio,
                summary.dedup_ratio.round(),
            ),
            color,
        ));
        out.push('\n');
    }
    out.push_str(&style::render_insight(
        InsightLevel::Good,
        "Deduplicated in+out — what Claude's UI calls \"total tokens\"",
        color,
    ));
    out.push('\n');

    out
}

// ─── CACHE SECTION ──────────────────────

fn render_cache_section(summary: &Summary, width: usize, bw: usize, bars: bool, color: bool) -> String {
    let mut out = String::new();
    out.push_str(&render_section_rule("↻ CACHE", width, color));
    out.push('\n');

    let total_cache = summary
        .cache_read_tokens
        .saturating_add(summary.cache_write_5m_tokens)
        .saturating_add(summary.cache_write_1h_tokens);
    let io_tokens = summary.input_tokens.saturating_add(summary.output_tokens);
    let total_volume = io_tokens.saturating_add(total_cache);

    // Cache efficiency = cache_read / (cache_read + input)
    let cacheable = summary
        .cache_read_tokens
        .saturating_add(summary.input_tokens);
    let efficiency = if cacheable > 0 {
        summary.cache_read_tokens as f64 / cacheable as f64 * 100.0
    } else {
        0.0
    };

    // Compute cache savings using per-model rates
    let savings = compute_cache_savings(summary);

    // Hero: efficiency + savings
    // Cap display at 99.9% unless truly 100% — rounding 99.96% to "100.0%" is misleading
    let eff_display = if efficiency >= 99.95 && summary.input_tokens > 0 {
        99.9
    } else {
        (efficiency * 10.0).round() / 10.0
    };
    let eff_str = style::chip(&format!("{:.1}%", eff_display), color);
    let companion = style::accent(
        &format!("efficiency · saved you ~{}", format_dollar(savings)),
        color,
    );
    out.push_str(&format!("\n  {eff_str}  {companion}\n"));

    // One-line cache explanation (full methodology in `ccmetrics explain`)
    out.push('\n');
    out.push_str(&format!(
        "  {}\n",
        style::description(
            "Context is cached server-side — you pay full price once, then 90% off.",
            color,
        )
    ));

    // Cache rows
    let cache_rows: Vec<(&str, u64, &str)> = vec![
        ("Read", summary.cache_read_tokens, "context replayed"),
        (
            "Write (5m)",
            summary.cache_write_5m_tokens,
            "new context stored",
        ),
        (
            "Write (1h)",
            summary.cache_write_1h_tokens,
            "extended cache",
        ),
    ];

    out.push('\n');
    for (label, count, desc) in &cache_rows {
        let fraction = if total_volume > 0 {
            *count as f64 / total_volume as f64
        } else {
            0.0
        };
        let bar_str = if bars {
            format!("    {}", style::render_bar(fraction, bw, color),)
        } else {
            String::new()
        };
        let label_styled = style::dim(label, color);
        let label_pad = 14usize.saturating_sub(label.len());
        let count_str = format_abbreviated(*count);
        out.push_str(&format!(
            "  {label_styled}{}{}{bar_str}    {}\n",
            " ".repeat(label_pad),
            style::value(&format!("{:>8}", count_str), color),
            style::description(desc, color),
        ));
    }

    // Insight: total without caching
    let without_cache = savings + summary.cost.total();
    out.push('\n');
    let level = if efficiency >= 90.0 {
        InsightLevel::Good
    } else if efficiency >= 70.0 {
        InsightLevel::Note
    } else {
        InsightLevel::Warn
    };
    out.push_str(&style::render_insight(
        level,
        &format!(
            "Without caching, this would have cost {}",
            format_dollar(without_cache)
        ),
        color,
    ));
    out.push('\n');

    out
}

/// Compute cache savings using per-model input rates from the pricing table.
/// savings = cache_read_tokens * (input_rate - cache_read_rate) per model
fn compute_cache_savings(summary: &Summary) -> f64 {
    let mut total_savings = 0.0;

    for m in &summary.by_model {
        if let Some(rates) = pricing::lookup_rates(&m.model) {
            // Savings per token = input rate - cache read rate
            let savings_per_m = rates.input - rates.cache_read;
            total_savings += m.cache_read_tokens as f64 * savings_per_m / 1_000_000.0;
        }
    }

    total_savings
}

// ─── WHERE IT GOES SECTION ──────────────

fn render_where_section(summary: &Summary, width: usize, bw: usize, bars: bool, color: bool) -> String {
    let mut out = String::new();
    out.push_str(&render_section_rule("⊕ WHERE IT GOES", width, color));
    out.push('\n');

    let total_cost = summary.cost.total();

    // Hero: main vs subagent cost split
    let model_count = summary.by_model.len();
    if summary.subagent_cost > 0.0 && total_cost > 0.0 {
        let main_pct_hero = (summary.main_cost / total_cost * 100.0).round() as u32;
        let sub_pct_hero = 100u32.saturating_sub(main_pct_hero);
        let hero_str = style::hero(
            &format!("{}% main · {}% subagent", main_pct_hero, sub_pct_hero),
            color,
        );
        let companion = style::dim(
            &format!("across {} model{}", model_count, if model_count == 1 { "" } else { "s" }),
            color,
        );
        out.push_str(&format!("\n  {hero_str}    {companion}\n"));
    } else {
        let hero_str = style::hero("100% main thread", color);
        let companion = style::dim("no subagent usage", color);
        out.push_str(&format!("\n  {hero_str}    {companion}\n"));
    }

    // Build left column: By thread (cost-based percentages)
    let main_pct = if total_cost > 0.0 {
        summary.main_cost / total_cost * 100.0
    } else {
        0.0
    };
    let sub_pct = if total_cost > 0.0 {
        summary.subagent_cost / total_cost * 100.0
    } else {
        0.0
    };

    let thread_rows: Vec<(&str, f64, f64)> = vec![
        ("Main", main_pct, summary.main_cost),
        ("Subagent", sub_pct, summary.subagent_cost),
    ];

    let mut left: Vec<String> = Vec::new();
    left.push(format!("  {} {}", style::accent("◇", color), style::subheader("By thread", color)));
    for (label, pct, cost) in &thread_rows {
        let label_styled = style::dim(label, color);
        let label_pad = 12usize.saturating_sub(label.len());
        left.push(format!(
            "  {label_styled}{} {}    {}",
            " ".repeat(label_pad),
            style::secondary(&format!("{:>3.0}%", pct), color),
            style::value(&format!("{:>10}", format_dollar(*cost)), color),
        ));
    }

    // Build right column: By model (top 5)
    let mut right: Vec<String> = Vec::new();
    right.push(format!("{} {}", style::accent("◆", color), style::subheader("By model", color)));
    let model_limit = summary.by_model.len().min(5);
    for m in &summary.by_model[..model_limit] {
        let model_pct = if total_cost > 0.0 {
            m.cost / total_cost * 100.0
        } else {
            0.0
        };
        let name: String = if m.model.len() > 20 {
            m.model.chars().take(20).collect()
        } else {
            m.model.clone()
        };
        let name_styled = style::dim(&name, color);
        let name_pad = 20usize.saturating_sub(name.len());
        right.push(format!(
            "{name_styled}{}  {}    {}",
            " ".repeat(name_pad),
            style::value(&format!("{:>10}", format_dollar(m.cost)), color),
            style::secondary(&format!("{:>5}", format!("{:.0}%", model_pct)), color),
        ));
    }

    // Merge columns side by side
    let merged = merge_columns(&left, &right, 4);
    out.push('\n');
    for line in &merged {
        out.push_str(line);
        out.push('\n');
    }

    // By Project (if 2+ projects)
    if summary.by_project.len() >= 2 {
        let project_header = format!("{} {}", style::accent("▪", color), style::subheader("By project", color));
        out.push_str(&format!("\n  {project_header}\n"));
        let max_cost = summary
            .by_project
            .iter()
            .map(|p| p.cost)
            .fold(0.0_f64, f64::max);
        let project_limit = summary.by_project.len().min(5);
        for p in &summary.by_project[..project_limit] {
            let pct = if total_cost > 0.0 {
                p.cost / total_cost * 100.0
            } else {
                0.0
            };
            let name = short_project(&p.project);
            let bar_str = if bars {
                let fraction = if max_cost > 0.0 {
                    p.cost / max_cost
                } else {
                    0.0
                };
                format!("    {}", style::render_bar(fraction, bw.min(20), color),)
            } else {
                String::new()
            };
            let name_styled = style::dim(name, color);
            let name_pad = 18usize.saturating_sub(name.len());
            out.push_str(&format!(
                "  {name_styled}{} {}{}    {}\n",
                " ".repeat(name_pad),
                style::value(&format!("{:>10}", format_dollar(p.cost)), color),
                bar_str,
                style::secondary(&format!("{:>5}", format!("{:.0}%", pct)), color),
            ));
        }
    }

    out
}

/// Merge two column vectors side-by-side with a gap.
fn merge_columns(left: &[String], right: &[String], gap: usize) -> Vec<String> {
    let max_left_width = left.iter().map(|s| visible_len(s)).max().unwrap_or(0);
    let rows = left.len().max(right.len());
    let mut result = Vec::with_capacity(rows);

    for i in 0..rows {
        let l = left.get(i).map(|s| s.as_str()).unwrap_or("");
        let r = right.get(i).map(|s| s.as_str()).unwrap_or("");
        let l_vis = visible_len(l);
        let padding = max_left_width.saturating_sub(l_vis) + gap;
        result.push(format!("{l}{}{r}", " ".repeat(padding)));
    }

    result
}

/// Calculate the visible length of a string (ignoring ANSI escape codes).
fn visible_len(s: &str) -> usize {
    let mut len = 0;
    let mut in_escape = false;
    for c in s.chars() {
        if in_escape {
            if c.is_ascii_alphabetic() {
                in_escape = false;
            }
        } else if c == '\x1b' {
            in_escape = true;
        } else {
            len += 1;
        }
    }
    len
}

/// Extract the last path component for display.
fn short_project(project: &str) -> &str {
    std::path::Path::new(project)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(project)
}

// ─── FOOTER ─────────────────────────────

fn render_footer(summary: &Summary, color: bool) -> String {
    let dedup = style::dim(
        &format!(
            "{:.2}x dedup · {} raw → {} unique",
            summary.dedup_ratio,
            format_number(summary.raw_lines as u64),
            format_number(summary.unique_requests as u64),
        ),
        color,
    );
    // Build footer with accent command embedded in dim text
    let pricing = if color {
        format!(
            "{}run '{}ccmetrics explain{}' for full methodology · Anthropic rates as of v{}{}",
            style::DIM,
            style::ACCENT,
            style::DIM,
            summary.version,
            style::RESET,
        )
    } else {
        format!(
            "run 'ccmetrics explain' for full methodology · Anthropic rates as of v{}",
            summary.version,
        )
    };
    format!("  {dedup}\n  {pricing}")
}

// ─── FORMATTING HELPERS ─────────────────

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

    // If rounding to 2dp will push us to 1000+, promote to the next tier
    if value >= 999.995 {
        return match suffix {
            "K" => "1.00M".to_string(),
            "M" => "1.00B".to_string(),
            // B tier: no higher suffix, show as-is
            _ => format!("{:.2}B", value),
        };
    }

    // Consistent 2 decimal places for vertical scanning
    format!("{:.2}{}", value, suffix)
}

/// Format a dollar amount.
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

        // Thousands — consistent 2 decimal places
        assert_eq!(format_abbreviated(1_000), "1.00K");
        assert_eq!(format_abbreviated(1_140), "1.14K");
        assert_eq!(format_abbreviated(1_500), "1.50K");
        assert_eq!(format_abbreviated(10_500), "10.50K");
        assert_eq!(format_abbreviated(260_000), "260.00K");

        // Millions
        assert_eq!(format_abbreviated(1_000_000), "1.00M");
        assert_eq!(format_abbreviated(1_140_000), "1.14M");
        assert_eq!(format_abbreviated(6_100_000), "6.10M");
        assert_eq!(format_abbreviated(153_000_000), "153.00M");

        // Billions
        assert_eq!(format_abbreviated(1_000_000_000), "1.00B");
        assert_eq!(format_abbreviated(2_730_000_000), "2.73B");
        assert_eq!(format_abbreviated(2_860_000_000), "2.86B");
    }

    #[test]
    fn test_format_abbreviated_tier_promotion() {
        // Rounding at K→M boundary (999.995+ promotes)
        assert_eq!(format_abbreviated(999_500), "999.50K");
        assert_eq!(format_abbreviated(999_999), "1.00M");

        // Rounding at M→B boundary
        assert_eq!(format_abbreviated(999_500_000), "999.50M");
        assert_eq!(format_abbreviated(999_999_999), "1.00B");

        // Just below rounding threshold stays in current tier
        assert_eq!(format_abbreviated(999_499), "999.50K");
        assert_eq!(format_abbreviated(999_499_999), "999.50M");

        // Very large values (trillions) — no T tier, stays as B
        assert_eq!(format_abbreviated(1_000_000_000_000), "1000.00B");

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

    #[test]
    fn test_visible_len() {
        assert_eq!(visible_len("hello"), 5);
        assert_eq!(visible_len("\x1b[1mhello\x1b[0m"), 5);
        assert_eq!(visible_len("\x1b[38;5;144mtest\x1b[0m"), 4);
        assert_eq!(visible_len(""), 0);
    }

    #[test]
    fn test_merge_columns() {
        let left = vec!["abc".to_string(), "de".to_string()];
        let right = vec!["xyz".to_string()];
        let merged = merge_columns(&left, &right, 2);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], "abc  xyz");
        assert_eq!(merged[1], "de   ");
    }

    #[test]
    fn test_short_project() {
        assert_eq!(short_project("/home/user/my-project"), "my-project");
        assert_eq!(short_project("standalone"), "standalone");
    }
}
