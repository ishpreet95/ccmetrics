use serde::Serialize;

use crate::types::Summary;

#[derive(Serialize)]
pub struct JsonOutput {
    pub version: String,
    pub generated_at: String,
    pub data_range: DataRange,
    pub dedup: DedupStats,
    pub tokens: TokenCounts,
    pub cost: CostOutput,
    pub split: SplitOutput,
    pub by_model: Vec<ModelOutput>,
}

#[derive(Serialize)]
pub struct DataRange {
    pub first_session: Option<String>,
    pub last_session: Option<String>,
    pub days: u64,
    pub sessions: usize,
    pub projects: usize,
}

#[derive(Serialize)]
pub struct DedupStats {
    pub raw_lines: usize,
    pub unique_requests: usize,
    pub skipped_lines: usize,
    pub ratio: f64,
}

#[derive(Serialize)]
pub struct TokenCounts {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write_5m: u64,
    pub cache_write_1h: u64,
}

#[derive(Serialize)]
pub struct CostOutput {
    pub total: f64,
    pub by_type: CostByType,
    pub currency: String,
    pub pricing_date: String,
    pub note: String,
}

#[derive(Serialize)]
pub struct CostByType {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write_5m: f64,
    pub cache_write_1h: f64,
    pub web_search: f64,
}

#[derive(Serialize)]
pub struct SplitOutput {
    pub main: SplitDetail,
    pub subagent: SplitDetail,
}

#[derive(Serialize)]
pub struct SplitDetail {
    pub requests: usize,
    pub input_output_tokens: u64,
    pub cost: f64,
}

#[derive(Serialize)]
pub struct ModelOutput {
    pub model: String,
    pub requests: usize,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_5m_tokens: u64,
    pub cache_write_1h_tokens: u64,
    pub cost: f64,
}

/// Render the summary as JSON.
pub fn render(summary: &Summary) -> serde_json::Result<String> {
    let output = JsonOutput {
        version: summary.version.clone(),
        generated_at: summary.generated_at.to_rfc3339(),
        data_range: DataRange {
            first_session: summary.first_session.map(|t| t.to_rfc3339()),
            last_session: summary.last_session.map(|t| t.to_rfc3339()),
            days: summary.days,
            sessions: summary.sessions,
            projects: summary.projects,
        },
        dedup: DedupStats {
            raw_lines: summary.raw_lines,
            unique_requests: summary.unique_requests,
            skipped_lines: summary.skipped_lines,
            ratio: round2(summary.dedup_ratio),
        },
        tokens: TokenCounts {
            input: summary.input_tokens,
            output: summary.output_tokens,
            cache_read: summary.cache_read_tokens,
            cache_write_5m: summary.cache_write_5m_tokens,
            cache_write_1h: summary.cache_write_1h_tokens,
        },
        cost: CostOutput {
            // Total is the sum of rounded components, then rounded again to avoid
            // IEEE 754 excess digits (e.g., 0.18000000000000002)
            total: round2(
                round2(summary.cost.input)
                    + round2(summary.cost.output)
                    + round2(summary.cost.cache_read)
                    + round2(summary.cost.cache_write_5m)
                    + round2(summary.cost.cache_write_1h)
                    + round2(summary.cost.web_search),
            ),
            by_type: CostByType {
                input: round2(summary.cost.input),
                output: round2(summary.cost.output),
                cache_read: round2(summary.cost.cache_read),
                cache_write_5m: round2(summary.cost.cache_write_5m),
                cache_write_1h: round2(summary.cost.cache_write_1h),
                web_search: round2(summary.cost.web_search),
            },
            currency: "USD".to_string(),
            pricing_date: "2026-03-22".to_string(),
            note: "API-equivalent cost at published Anthropic rates".to_string(),
        },
        split: SplitOutput {
            main: SplitDetail {
                requests: summary.main_requests,
                input_output_tokens: summary.main_input_output_tokens,
                cost: round2(summary.main_cost),
            },
            subagent: SplitDetail {
                requests: summary.subagent_requests,
                input_output_tokens: summary.subagent_input_output_tokens,
                cost: round2(summary.subagent_cost),
            },
        },
        by_model: summary
            .by_model
            .iter()
            .map(|m| ModelOutput {
                model: m.model.clone(),
                requests: m.requests,
                input_tokens: m.input_tokens,
                output_tokens: m.output_tokens,
                cache_read_tokens: m.cache_read_tokens,
                cache_write_5m_tokens: m.cache_write_5m_tokens,
                cache_write_1h_tokens: m.cache_write_1h_tokens,
                cost: round2(m.cost),
            })
            .collect(),
    };

    serde_json::to_string_pretty(&output)
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
