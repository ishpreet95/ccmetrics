use crate::types::{CostBreakdown, Speed, UsageEntry};

/// Per-model pricing rates in dollars per million tokens.
#[derive(Debug, Clone, Copy)]
pub struct ModelRates {
    pub model_id: &'static str,
    pub input: f64,
    pub output: f64,
    pub cache_write_5m: f64,
    pub cache_write_1h: f64,
    pub cache_read: f64,
}

/// Embedded pricing table — source of truth: docs/PRICING.md
/// Last verified: 2026-03-22
pub const PRICING_TABLE: &[ModelRates] = &[
    ModelRates { model_id: "claude-opus-4-6",              input: 5.00,  output: 25.00, cache_write_5m: 6.25,  cache_write_1h: 10.00, cache_read: 0.50 },
    ModelRates { model_id: "claude-opus-4-5",              input: 5.00,  output: 25.00, cache_write_5m: 6.25,  cache_write_1h: 10.00, cache_read: 0.50 },
    ModelRates { model_id: "claude-opus-4-5-20251101",     input: 5.00,  output: 25.00, cache_write_5m: 6.25,  cache_write_1h: 10.00, cache_read: 0.50 },
    ModelRates { model_id: "claude-opus-4-1",              input: 15.00, output: 75.00, cache_write_5m: 18.75, cache_write_1h: 30.00, cache_read: 1.50 },
    ModelRates { model_id: "claude-opus-4",                input: 15.00, output: 75.00, cache_write_5m: 18.75, cache_write_1h: 30.00, cache_read: 1.50 },
    ModelRates { model_id: "claude-sonnet-4-6",            input: 3.00,  output: 15.00, cache_write_5m: 3.75,  cache_write_1h: 6.00,  cache_read: 0.30 },
    ModelRates { model_id: "claude-sonnet-4-5",            input: 3.00,  output: 15.00, cache_write_5m: 3.75,  cache_write_1h: 6.00,  cache_read: 0.30 },
    ModelRates { model_id: "claude-sonnet-4-5-20250929",   input: 3.00,  output: 15.00, cache_write_5m: 3.75,  cache_write_1h: 6.00,  cache_read: 0.30 },
    ModelRates { model_id: "claude-sonnet-4",              input: 3.00,  output: 15.00, cache_write_5m: 3.75,  cache_write_1h: 6.00,  cache_read: 0.30 },
    ModelRates { model_id: "claude-haiku-4-5",             input: 1.00,  output: 5.00,  cache_write_5m: 1.25,  cache_write_1h: 2.00,  cache_read: 0.10 },
    ModelRates { model_id: "claude-haiku-4-5-20251001",    input: 1.00,  output: 5.00,  cache_write_5m: 1.25,  cache_write_1h: 2.00,  cache_read: 0.10 },
    ModelRates { model_id: "claude-haiku-3-5",             input: 0.80,  output: 4.00,  cache_write_5m: 1.00,  cache_write_1h: 1.60,  cache_read: 0.08 },
];

/// Web search cost: $10 per 1,000 searches.
const WEB_SEARCH_COST_PER_REQUEST: f64 = 10.0 / 1000.0;

/// Look up pricing rates for a model.
///
/// Tries exact match first, then prefix match (e.g., "claude-opus-4-6" matches
/// "claude-opus-4-6[1m]" if Claude Code appends context suffixes).
pub fn lookup_rates(model_id: &str) -> Option<&'static ModelRates> {
    // Exact match
    if let Some(rates) = PRICING_TABLE.iter().find(|r| r.model_id == model_id) {
        return Some(rates);
    }
    // Prefix match: model_id starts with a known model
    PRICING_TABLE
        .iter()
        .filter(|r| model_id.starts_with(r.model_id))
        .max_by_key(|r| r.model_id.len())
}

/// Calculate the cost breakdown for a single usage entry.
pub fn calculate_cost(entry: &UsageEntry) -> CostBreakdown {
    let rates = match lookup_rates(&entry.model) {
        Some(r) => r,
        None => {
            // Unknown model — $0 pricing
            return CostBreakdown::default();
        }
    };

    let per_m = 1_000_000.0;

    // Base costs
    let mut input_cost = entry.input_tokens as f64 * rates.input / per_m;
    let mut output_cost = entry.output_tokens as f64 * rates.output / per_m;
    let mut cache_read_cost = entry.cache_read_input_tokens as f64 * rates.cache_read / per_m;
    let mut cache_5m_cost = entry.cache_write_5m_tokens as f64 * rates.cache_write_5m / per_m;
    let mut cache_1h_cost = entry.cache_write_1h_tokens as f64 * rates.cache_write_1h / per_m;

    // Long context modifier: Sonnet 4/4.5 only, >200k input tokens
    // Does NOT apply to Opus or Haiku
    if is_long_context_eligible(&entry.model) && entry.input_tokens > 200_000 {
        input_cost *= 2.0;
        output_cost *= 1.5;
    }

    // Fast mode modifier (6x all token types)
    // Currently only available for Opus 4.6 but we apply it when the field says "fast"
    // since Claude Code sets the field based on what was actually billed
    if entry.speed == Speed::Fast {
        let fast_mult = 6.0;
        input_cost *= fast_mult;
        output_cost *= fast_mult;
        cache_read_cost *= fast_mult;
        cache_5m_cost *= fast_mult;
        cache_1h_cost *= fast_mult;
    }

    // Data residency modifier (1.1x all token types)
    // Only applies when inference_geo is explicitly "us"
    if entry.inference_geo.as_deref() == Some("us") {
        let geo_mult = 1.1;
        input_cost *= geo_mult;
        output_cost *= geo_mult;
        cache_read_cost *= geo_mult;
        cache_5m_cost *= geo_mult;
        cache_1h_cost *= geo_mult;
    }

    // Web search costs — flat per-request rate, NOT scaled by speed/geo modifiers
    // (Anthropic bills web search independently of token pricing modifiers)
    let web_search_cost = entry.web_search_requests as f64 * WEB_SEARCH_COST_PER_REQUEST;

    CostBreakdown {
        input: input_cost,
        output: output_cost,
        cache_read: cache_read_cost,
        cache_write_5m: cache_5m_cost,
        cache_write_1h: cache_1h_cost,
        web_search: web_search_cost,
    }
}

/// Long-context pricing applies to Sonnet 4 and 4.5 only (not Opus, not Haiku).
pub(crate) fn is_long_context_eligible(model: &str) -> bool {
    model.contains("sonnet-4") || model.contains("sonnet-4-5")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Speed;
    use chrono::Utc;
    use std::path::PathBuf;

    fn make_entry(model: &str, input: u64, output: u64) -> UsageEntry {
        UsageEntry {
            request_id: "req_1".to_string(),
            session_id: "s1".to_string(),
            model: model.to_string(),
            is_sidechain: false,
            timestamp: Utc::now(),
            input_tokens: input,
            output_tokens: output,
            cache_read_input_tokens: 0,
            cache_write_5m_tokens: 0,
            cache_write_1h_tokens: 0,
            speed: Speed::Standard,
            inference_geo: None,
            web_search_requests: 0,
            web_fetch_requests: 0,
            source_file: PathBuf::from("test.jsonl"),
            project_path: "/test".to_string(),
        }
    }

    #[test]
    fn test_opus_pricing() {
        let entry = make_entry("claude-opus-4-6", 1_000_000, 1_000_000);
        let cost = calculate_cost(&entry);
        assert!((cost.input - 5.0).abs() < 0.001);
        assert!((cost.output - 25.0).abs() < 0.001);
    }

    #[test]
    fn test_haiku_pricing() {
        let entry = make_entry("claude-haiku-4-5-20251001", 1_000_000, 1_000_000);
        let cost = calculate_cost(&entry);
        assert!((cost.input - 1.0).abs() < 0.001);
        assert!((cost.output - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_fast_mode_6x() {
        let mut entry = make_entry("claude-opus-4-6", 1_000_000, 0);
        entry.speed = Speed::Fast;
        let cost = calculate_cost(&entry);
        assert!((cost.input - 30.0).abs() < 0.001); // 5.0 * 6.0
    }

    #[test]
    fn test_data_residency_1_1x() {
        let mut entry = make_entry("claude-opus-4-6", 1_000_000, 0);
        entry.inference_geo = Some("us".to_string());
        let cost = calculate_cost(&entry);
        assert!((cost.input - 5.5).abs() < 0.001); // 5.0 * 1.1
    }

    #[test]
    fn test_unknown_model_zero_cost() {
        let entry = make_entry("unknown-model-v9", 1_000_000, 1_000_000);
        let cost = calculate_cost(&entry);
        assert!((cost.total()).abs() < 0.001);
    }

    #[test]
    fn test_cache_read_pricing() {
        let mut entry = make_entry("claude-opus-4-6", 0, 0);
        entry.cache_read_input_tokens = 1_000_000;
        let cost = calculate_cost(&entry);
        assert!((cost.cache_read - 0.50).abs() < 0.001);
    }

    #[test]
    fn test_web_search_cost() {
        let mut entry = make_entry("claude-opus-4-6", 0, 0);
        entry.web_search_requests = 10;
        let cost = calculate_cost(&entry);
        assert!((cost.web_search - 0.10).abs() < 0.001);
    }

    #[test]
    fn test_modifier_stacking() {
        // Fast + data residency = 6.0 * 1.1 = 6.6x
        let mut entry = make_entry("claude-opus-4-6", 1_000_000, 0);
        entry.speed = Speed::Fast;
        entry.inference_geo = Some("us".to_string());
        let cost = calculate_cost(&entry);
        assert!((cost.input - 33.0).abs() < 0.001); // 5.0 * 6.0 * 1.1
    }

    #[test]
    fn test_sonnet_long_context_fast_stacking() {
        // Sonnet + >200k input + fast mode = long_context then fast
        let mut entry = make_entry("claude-sonnet-4-6", 200_001, 100_000);
        entry.speed = Speed::Fast;
        let cost = calculate_cost(&entry);
        // input: (200001/1e6 * 3.0) * 2.0 * 6.0
        let expected_input = 200_001.0 / 1_000_000.0 * 3.0 * 2.0 * 6.0;
        // output: (100000/1e6 * 15.0) * 1.5 * 6.0
        let expected_output = 100_000.0 / 1_000_000.0 * 15.0 * 1.5 * 6.0;
        assert!((cost.input - expected_input).abs() < 0.001);
        assert!((cost.output - expected_output).abs() < 0.001);
    }

    #[test]
    fn test_lookup_prefix_match() {
        // Model ID with suffix should still match
        let rates = lookup_rates("claude-opus-4-6");
        assert!(rates.is_some());
        assert_eq!(rates.unwrap().input, 5.0);
    }

    #[test]
    fn test_long_context_only_sonnet() {
        // Opus and Haiku should NOT get long-context modifier even with >200k input
        assert!(
            !is_long_context_eligible("claude-opus-4-6"),
            "Opus should not be long-context eligible"
        );
        assert!(
            !is_long_context_eligible("claude-opus-4-5"),
            "Opus 4.5 should not be long-context eligible"
        );
        assert!(
            !is_long_context_eligible("claude-haiku-4-5"),
            "Haiku should not be long-context eligible"
        );
        assert!(
            !is_long_context_eligible("claude-haiku-4-5-20251001"),
            "Haiku dated should not be long-context eligible"
        );

        // Verify Opus >200k input does NOT get 2x multiplier
        let mut opus_entry = make_entry("claude-opus-4-6", 300_000, 100_000);
        opus_entry.speed = Speed::Standard;
        let cost = calculate_cost(&opus_entry);
        // Without long-context: 300000/1e6 * 5.0 = 1.5
        let expected_input = 300_000.0 / 1_000_000.0 * 5.0;
        assert!(
            (cost.input - expected_input).abs() < 0.001,
            "Opus input cost should NOT have long-context 2x: got {} expected {}",
            cost.input,
            expected_input
        );

        // Verify Haiku >200k input does NOT get 2x multiplier
        let haiku_entry = make_entry("claude-haiku-4-5", 300_000, 100_000);
        let cost = calculate_cost(&haiku_entry);
        let expected_input = 300_000.0 / 1_000_000.0 * 1.0;
        assert!(
            (cost.input - expected_input).abs() < 0.001,
            "Haiku input cost should NOT have long-context 2x: got {} expected {}",
            cost.input,
            expected_input
        );

        // Verify Sonnet DOES get 2x
        assert!(
            is_long_context_eligible("claude-sonnet-4-6"),
            "Sonnet 4-6 should be long-context eligible"
        );
        assert!(
            is_long_context_eligible("claude-sonnet-4-5"),
            "Sonnet 4-5 should be long-context eligible"
        );
    }

    #[test]
    fn test_data_residency_only_us() {
        // inference_geo = "eu" should NOT trigger the 1.1x modifier
        let mut entry = make_entry("claude-opus-4-6", 1_000_000, 0);
        entry.inference_geo = Some("eu".to_string());
        let cost = calculate_cost(&entry);
        // Without geo modifier: 1M / 1e6 * 5.0 = 5.0
        assert!(
            (cost.input - 5.0).abs() < 0.001,
            "EU geo should NOT trigger data residency modifier: got {} expected 5.0",
            cost.input
        );
    }

    #[test]
    fn test_data_residency_none_no_modifier() {
        // inference_geo = None should NOT trigger the modifier
        let mut entry = make_entry("claude-opus-4-6", 1_000_000, 0);
        entry.inference_geo = None;
        let cost = calculate_cost(&entry);
        assert!(
            (cost.input - 5.0).abs() < 0.001,
            "None geo should NOT trigger data residency modifier: got {} expected 5.0",
            cost.input
        );
    }
}
