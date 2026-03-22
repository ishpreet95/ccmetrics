use chrono::{DateTime, Utc};
use std::path::PathBuf;

/// Speed modifier for pricing (6x for fast mode).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Speed {
    Standard,
    Fast,
}

/// A single deduplicated API request with its usage data.
#[derive(Debug, Clone)]
pub struct UsageEntry {
    pub request_id: String,
    pub session_id: String,
    pub model: String,
    pub is_sidechain: bool,
    pub timestamp: DateTime<Utc>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_write_5m_tokens: u64,
    pub cache_write_1h_tokens: u64,
    pub speed: Speed,
    pub inference_geo: Option<String>,
    pub web_search_requests: u32,
    #[allow(dead_code)] // Parsed for Phase 2 web fetch cost tracking
    pub web_fetch_requests: u32,
    #[allow(dead_code)] // Parsed for Phase 2 session drill-down
    pub source_file: PathBuf,
    pub project_path: String,
}

/// Cost breakdown for a single entry or aggregate.
#[derive(Debug, Clone, Default)]
pub struct CostBreakdown {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write_5m: f64,
    pub cache_write_1h: f64,
    pub web_search: f64,
}

impl CostBreakdown {
    pub fn total(&self) -> f64 {
        self.input
            + self.output
            + self.cache_read
            + self.cache_write_5m
            + self.cache_write_1h
            + self.web_search
    }
}

impl std::ops::AddAssign for CostBreakdown {
    fn add_assign(&mut self, rhs: Self) {
        self.input += rhs.input;
        self.output += rhs.output;
        self.cache_read += rhs.cache_read;
        self.cache_write_5m += rhs.cache_write_5m;
        self.cache_write_1h += rhs.cache_write_1h;
        self.web_search += rhs.web_search;
    }
}

/// Per-model token and cost breakdown for verifiability.
#[derive(Debug, Clone)]
pub struct ModelBreakdown {
    pub model: String,
    pub requests: usize,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_5m_tokens: u64,
    pub cache_write_1h_tokens: u64,
    pub cost: f64,
}

/// Aggregated summary of all usage.
#[derive(Debug)]
pub struct Summary {
    pub version: String,
    pub generated_at: DateTime<Utc>,
    pub first_session: Option<DateTime<Utc>>,
    pub last_session: Option<DateTime<Utc>>,
    pub days: u64,
    pub sessions: usize,
    pub projects: usize,
    pub raw_lines: usize,
    pub unique_requests: usize,
    pub skipped_lines: usize,
    pub dedup_ratio: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_5m_tokens: u64,
    pub cache_write_1h_tokens: u64,
    pub cost: CostBreakdown,
    pub main_requests: usize,
    pub main_input_output_tokens: u64,
    pub main_cost: f64,
    pub subagent_requests: usize,
    pub subagent_input_output_tokens: u64,
    pub subagent_cost: f64,
    pub by_model: Vec<ModelBreakdown>,
}

/// A non-fatal warning accumulated during processing.
#[derive(Debug)]
pub struct Warning {
    pub file: PathBuf,
    pub line: Option<usize>,
    pub message: String,
}

/// Stats about the scanning/parsing phase for verbose output.
#[derive(Debug, Default)]
pub struct ParseStats {
    pub total_files: usize,
    pub main_files: usize,
    pub subagent_files: usize,
    pub raw_lines: usize,
    pub assistant_lines: usize,
    pub skipped_lines: usize,
    pub no_id_entries: usize,
    pub unique_after_dedup: usize,
    pub synthetic_messages: usize,
    #[allow(dead_code)] // Tracked for Phase 3 verbose warnings
    pub unknown_models: usize,
}
