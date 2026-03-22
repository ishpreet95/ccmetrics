use std::path::PathBuf;
use std::process::Command;

fn fixtures_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn run_cc_metrics(args: &[&str]) -> (String, String, bool) {
    let output = Command::new(env!("CARGO_BIN_EXE_ccmetrics"))
        .args(args)
        .output()
        .expect("Failed to execute cc-metrics");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (stdout, stderr, output.status.success())
}

#[test]
fn test_simple_session_table_output() {
    let (stdout, _stderr, success) = run_cc_metrics(&["--path", fixtures_path().to_str().unwrap()]);

    assert!(success, "cc-metrics should succeed");
    assert!(
        stdout.contains("ccmetrics v0.1.0"),
        "Should show version header"
    );
    assert!(
        stdout.contains("Token Breakdown"),
        "Should show token table"
    );
    assert!(
        stdout.contains("Main vs Subagent"),
        "Should show split table"
    );
    assert!(stdout.contains("Dedup:"), "Should show dedup stats");

    // Fixtures use 3 models, so the "By Model" table should appear
    assert!(stdout.contains("By Model"), "Should show By Model table");
    assert!(
        stdout.contains("claude-opus-4-6"),
        "Should show opus model in By Model table"
    );
    assert!(
        stdout.contains("claude-sonnet-4-5"),
        "Should show sonnet model in By Model table"
    );
    assert!(
        stdout.contains("claude-haiku-4-5"),
        "Should show haiku model in By Model table"
    );
}

#[test]
fn test_json_output_structure() {
    let (stdout, _stderr, success) =
        run_cc_metrics(&["--path", fixtures_path().to_str().unwrap(), "--json"]);

    assert!(success, "ccmetrics --json should succeed");

    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");

    assert_eq!(json["version"].as_str().unwrap(), "0.1.0");
    assert!(json["data_range"]["days"].as_u64().unwrap() >= 1);
    assert!(json["dedup"]["unique_requests"].as_u64().unwrap() > 0);
    assert!(json["tokens"]["input"].as_u64().unwrap() > 0);
    assert!(json["cost"]["total"].as_f64().unwrap() > 0.0);
    assert_eq!(json["cost"]["currency"].as_str().unwrap(), "USD");

    // by_model should be an array with at least 1 entry
    let by_model = json["by_model"]
        .as_array()
        .expect("by_model should be an array");
    assert!(
        !by_model.is_empty(),
        "by_model should have at least 1 entry"
    );

    // Each entry should have the expected fields
    for entry in by_model {
        assert!(entry["model"].is_string(), "model should be a string");
        assert!(entry["requests"].is_u64(), "requests should be a number");
        assert!(
            entry["input_tokens"].is_u64(),
            "input_tokens should be a number"
        );
        assert!(
            entry["output_tokens"].is_u64(),
            "output_tokens should be a number"
        );
        assert!(
            entry["cache_read_tokens"].is_u64(),
            "cache_read_tokens should be a number"
        );
        assert!(
            entry["cache_write_5m_tokens"].is_u64(),
            "cache_write_5m_tokens should be a number"
        );
        assert!(
            entry["cache_write_1h_tokens"].is_u64(),
            "cache_write_1h_tokens should be a number"
        );
        assert!(entry["cost"].is_f64(), "cost should be a number");
    }
}

#[test]
fn test_dedup_streaming_chunks() {
    let (stdout, _stderr, success) =
        run_cc_metrics(&["--path", fixtures_path().to_str().unwrap(), "--json"]);

    assert!(success);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // streaming_session.jsonl has 3 chunks for req_stream_001
    // plus simple_session.jsonl has 1 entry, subagent has 2 chunks for 1 entry,
    // synthetic_and_edge has 1 real entry (synthetic excluded, malformed skipped)
    // Total unique requests should be 4
    let unique = json["dedup"]["unique_requests"].as_u64().unwrap();
    assert_eq!(unique, 4, "Should have 4 unique requests after dedup");

    // Skipped lines should include the malformed JSON line
    let skipped = json["dedup"]["skipped_lines"].as_u64().unwrap();
    assert!(
        skipped >= 1,
        "Should have at least 1 skipped line (malformed JSON)"
    );
}

#[test]
fn test_subagent_split() {
    let (stdout, _stderr, success) =
        run_cc_metrics(&["--path", fixtures_path().to_str().unwrap(), "--json"]);

    assert!(success);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let main_requests = json["split"]["main"]["requests"].as_u64().unwrap();
    let sub_requests = json["split"]["subagent"]["requests"].as_u64().unwrap();

    assert!(main_requests > 0, "Should have main thread requests");
    assert!(sub_requests > 0, "Should have subagent requests");
    assert_eq!(main_requests + sub_requests, 4, "Total should be 4");
}

#[test]
fn test_streaming_dedup_keeps_correct_tokens() {
    let (stdout, _stderr, success) =
        run_cc_metrics(&["--path", fixtures_path().to_str().unwrap(), "--json"]);

    assert!(success);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // The streaming session's final chunk has output_tokens=365 (not 10 or 11)
    // The simple session has output_tokens=50
    // The subagent has output_tokens=200
    // The edge case has output_tokens=200
    // Total output tokens should be 365 + 50 + 200 + 200 = 815
    let output_tokens = json["tokens"]["output"].as_u64().unwrap();
    assert_eq!(
        output_tokens, 815,
        "Output tokens should reflect final chunks only"
    );
}

#[test]
fn test_cache_tier_disaggregation() {
    let (stdout, _stderr, success) =
        run_cc_metrics(&["--path", fixtures_path().to_str().unwrap(), "--json"]);

    assert!(success);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let cache_5m = json["tokens"]["cache_write_5m"].as_u64().unwrap();
    let cache_1h = json["tokens"]["cache_write_1h"].as_u64().unwrap();

    assert!(cache_5m > 0, "Should have 5m cache writes (from subagent)");
    assert!(
        cache_1h > 0,
        "Should have 1h cache writes (from main thread)"
    );
}

#[test]
fn test_verbose_output() {
    let (stdout, _stderr, success) =
        run_cc_metrics(&["--path", fixtures_path().to_str().unwrap(), "--verbose"]);

    assert!(success);
    assert!(
        stdout.contains("Verbose Details"),
        "Should show verbose section"
    );
    assert!(stdout.contains("Files scanned:"), "Should show file count");
    assert!(
        stdout.contains("Skipped lines:"),
        "Should show skipped lines"
    );
    assert!(
        stdout.contains("Synthetic msgs:"),
        "Should show synthetic count"
    );
}

#[test]
fn test_cost_calculation_correctness() {
    let (stdout, _stderr, success) =
        run_cc_metrics(&["--path", fixtures_path().to_str().unwrap(), "--json"]);

    assert!(success);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let total_cost = json["cost"]["total"].as_f64().unwrap();
    assert!(total_cost > 0.0, "Total cost should be positive");

    // Verify cost breakdown sums to total
    let by_type = &json["cost"]["by_type"];
    let sum = by_type["input"].as_f64().unwrap()
        + by_type["output"].as_f64().unwrap()
        + by_type["cache_read"].as_f64().unwrap()
        + by_type["cache_write_5m"].as_f64().unwrap()
        + by_type["cache_write_1h"].as_f64().unwrap()
        + by_type["web_search"].as_f64().unwrap();

    assert!(
        (sum - total_cost).abs() < 0.02,
        "Cost breakdown should sum to total (got {sum} vs {total_cost})"
    );

    // Verify cost.total has no IEEE 754 excess digits (e.g., 0.18000000000000002)
    let cost_str = json["cost"]["total"].to_string();
    let decimal_digits = cost_str.split('.').nth(1).map(|d| d.len()).unwrap_or(0);
    assert!(
        decimal_digits <= 2,
        "cost.total should have at most 2 decimal places, got '{cost_str}'"
    );

    // Verify by_model costs sum close to total
    let by_model = json["by_model"].as_array().unwrap();
    let model_cost_sum: f64 = by_model.iter().map(|m| m["cost"].as_f64().unwrap()).sum();
    assert!(
        (model_cost_sum - total_cost).abs() < 0.02,
        "by_model costs should sum close to total (got {model_cost_sum} vs {total_cost})"
    );
}

#[test]
fn test_explain_output() {
    let (stdout, _stderr, success) =
        run_cc_metrics(&["--path", fixtures_path().to_str().unwrap(), "explain"]);

    assert!(success, "ccmetrics explain should succeed");
    assert!(
        stdout.contains("Methodology Walkthrough"),
        "Should contain walkthrough header"
    );
    assert!(stdout.contains("STEP 1"), "Should contain STEP 1");
    assert!(stdout.contains("STEP 2"), "Should contain STEP 2");
    assert!(stdout.contains("STEP 3"), "Should contain STEP 3");
    assert!(stdout.contains("STEP 4"), "Should contain STEP 4");
}

#[test]
fn test_empty_path_no_crash() {
    let temp = tempfile::tempdir().unwrap();
    let (stdout, stderr, success) = run_cc_metrics(&["--path", temp.path().to_str().unwrap()]);

    assert!(success, "Should succeed even with no files");
    assert!(
        stderr.contains("No JSONL files found") || stdout.contains("error"),
        "Should report no files found"
    );
}

#[test]
fn test_model_filter() {
    let (stdout, _stderr, success) = run_cc_metrics(&[
        "--path",
        fixtures_path().to_str().unwrap(),
        "--json",
        "--model",
        "opus",
    ]);

    assert!(success, "ccmetrics --model opus should succeed");
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // With model filter, all entries should be opus
    let by_model = json["by_model"].as_array().unwrap();
    assert_eq!(by_model.len(), 1, "Should have only 1 model after filter");
    assert!(
        by_model[0]["model"].as_str().unwrap().contains("opus"),
        "Filtered model should be opus"
    );

    // Filter should be in JSON output
    assert!(
        json["filter"].is_object(),
        "Should have filter info in JSON"
    );
    assert_eq!(
        json["filter"]["model"].as_str().unwrap(),
        "opus",
        "Filter should show model"
    );
}

#[test]
fn test_model_filter_no_match() {
    let (stdout, _stderr, success) = run_cc_metrics(&[
        "--path",
        fixtures_path().to_str().unwrap(),
        "--json",
        "--model",
        "gemini",
    ]);

    assert!(success, "Should succeed even with no matching model");
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let unique = json["dedup"]["unique_requests"].as_u64().unwrap();
    assert_eq!(
        unique, 0,
        "No matching entries should give 0 unique requests"
    );
}

#[test]
fn test_date_filter_since() {
    // All fixture entries have timestamps in 2025-2026 range
    // Filtering with --since far in the future should give 0 entries
    let (stdout, _stderr, success) = run_cc_metrics(&[
        "--path",
        fixtures_path().to_str().unwrap(),
        "--json",
        "--since",
        "2030-01-01",
    ]);

    assert!(success, "Should succeed with future --since");
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let unique = json["dedup"]["unique_requests"].as_u64().unwrap();
    assert_eq!(unique, 0, "Future --since should give 0 entries");
}

#[test]
fn test_date_filter_invalid() {
    let (_stdout, stderr, success) = run_cc_metrics(&[
        "--path",
        fixtures_path().to_str().unwrap(),
        "--since",
        "not-a-date",
    ]);

    assert!(!success, "Invalid date should fail");
    assert!(
        stderr.contains("Unrecognized date format"),
        "Should show date parse error: {stderr}"
    );
}

#[test]
fn test_quiet_flag_suppresses_pipeline() {
    let (_stdout, stderr, success) =
        run_cc_metrics(&["--path", fixtures_path().to_str().unwrap(), "--quiet"]);

    assert!(success, "ccmetrics --quiet should succeed");
    // Pipeline output goes to stderr; --quiet should suppress it
    assert!(
        !stderr.contains("Scanning"),
        "Pipeline should be suppressed with --quiet"
    );
}

#[test]
fn test_filter_indicator_in_table() {
    let (stdout, _stderr, success) = run_cc_metrics(&[
        "--path",
        fixtures_path().to_str().unwrap(),
        "--model",
        "opus",
        "--quiet",
    ]);

    assert!(success);
    assert!(
        stdout.contains("filtered"),
        "Should show filter indicator in header"
    );
    assert!(
        stdout.contains("model: opus"),
        "Should describe active filter"
    );
}

#[test]
fn test_json_no_filter_field_when_unfiltered() {
    let (stdout, _stderr, success) =
        run_cc_metrics(&["--path", fixtures_path().to_str().unwrap(), "--json"]);

    assert!(success);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(
        json.get("filter").is_none(),
        "Unfiltered JSON should not have filter field"
    );
}

// --- Sprint 2: by_project, daily, session ---

#[test]
fn test_by_project_in_json() {
    let (stdout, _stderr, success) =
        run_cc_metrics(&["--path", fixtures_path().to_str().unwrap(), "--json"]);

    assert!(success);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(
        json.get("by_project").is_some(),
        "JSON should have by_project field"
    );
    let by_project = json["by_project"].as_array().unwrap();
    // Should have at least 1 project from fixtures
    assert!(!by_project.is_empty(), "Should have at least 1 project");
    // Each project should have expected fields
    for p in by_project {
        assert!(p.get("project").is_some(), "project field");
        assert!(p.get("sessions").is_some(), "sessions field");
        assert!(p.get("requests").is_some(), "requests field");
        assert!(p.get("cost").is_some(), "cost field");
    }
}

#[test]
fn test_daily_subcommand() {
    let (stdout, _stderr, success) = run_cc_metrics(&[
        "--path",
        fixtures_path().to_str().unwrap(),
        "--quiet",
        "daily",
    ]);

    assert!(success, "ccmetrics daily should succeed");
    assert!(
        stdout.contains("Daily breakdown"),
        "Should show daily header"
    );
    assert!(stdout.contains("Date"), "Should have Date column");
    assert!(stdout.contains("Avg:"), "Should show average line");
}

#[test]
fn test_daily_json() {
    let (stdout, _stderr, success) = run_cc_metrics(&[
        "--path",
        fixtures_path().to_str().unwrap(),
        "--json",
        "daily",
    ]);

    assert!(success, "ccmetrics daily --json should succeed");
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(
        json.get("daily").is_some(),
        "Daily JSON should have daily array"
    );
    let daily = json["daily"].as_array().unwrap();
    assert!(!daily.is_empty(), "Should have at least 1 day");
    for d in daily {
        assert!(d.get("date").is_some(), "date field");
        assert!(d.get("requests").is_some(), "requests field");
        assert!(d.get("cost").is_some(), "cost field");
    }
}

#[test]
fn test_session_list_subcommand() {
    let (stdout, _stderr, success) = run_cc_metrics(&[
        "--path",
        fixtures_path().to_str().unwrap(),
        "--quiet",
        "session",
    ]);

    assert!(success, "ccmetrics session should succeed");
    assert!(
        stdout.contains("Recent sessions"),
        "Should show session list header"
    );
    assert!(
        stdout.contains("Session ID"),
        "Should have Session ID column"
    );
}

#[test]
fn test_session_list_json() {
    let (stdout, _stderr, success) = run_cc_metrics(&[
        "--path",
        fixtures_path().to_str().unwrap(),
        "--json",
        "session",
    ]);

    assert!(success, "ccmetrics session --json should succeed");
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(
        json.get("sessions").is_some(),
        "Session JSON should have sessions array"
    );
    let sessions = json["sessions"].as_array().unwrap();
    assert!(!sessions.is_empty(), "Should have at least 1 session");
    for s in sessions {
        assert!(s.get("session_id").is_some(), "session_id field");
        assert!(s.get("date").is_some(), "date field");
        assert!(s.get("cost").is_some(), "cost field");
        assert!(s.get("subagent_spawns").is_some(), "subagent_spawns field");
        assert!(
            s.get("cache_read_tokens").is_some(),
            "cache_read_tokens field"
        );
        assert!(
            s.get("cache_write_5m_tokens").is_some(),
            "cache_write_5m_tokens field"
        );
    }
}

#[test]
fn test_session_drill_down_no_match() {
    let (_stdout, stderr, success) = run_cc_metrics(&[
        "--path",
        fixtures_path().to_str().unwrap(),
        "session",
        "nonexistent-session-id-12345",
    ]);

    assert!(!success, "Non-existent session ID should fail");
    assert!(
        stderr.contains("No session found"),
        "Should show no match message"
    );
}

#[test]
fn test_daily_with_filter() {
    let (stdout, _stderr, success) = run_cc_metrics(&[
        "--path",
        fixtures_path().to_str().unwrap(),
        "--quiet",
        "--model",
        "opus",
        "daily",
    ]);

    assert!(success, "ccmetrics daily --model opus should succeed");
    assert!(stdout.contains("filtered"), "Should show filter indicator");
}
