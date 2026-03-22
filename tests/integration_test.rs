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
    let (stdout, _stderr, success) = run_cc_metrics(&[
        "--path",
        fixtures_path().to_str().unwrap(),
    ]);

    assert!(success, "cc-metrics should succeed");
    assert!(stdout.contains("ccmetrics v0.1.0"), "Should show version header");
    assert!(stdout.contains("Token Breakdown"), "Should show token table");
    assert!(stdout.contains("Main vs Subagent"), "Should show split table");
    assert!(stdout.contains("Dedup:"), "Should show dedup stats");
}

#[test]
fn test_json_output_structure() {
    let (stdout, _stderr, success) = run_cc_metrics(&[
        "--path",
        fixtures_path().to_str().unwrap(),
        "--json",
    ]);

    assert!(success, "ccmetrics --json should succeed");

    let json: serde_json::Value = serde_json::from_str(&stdout)
        .expect("Output should be valid JSON");

    assert_eq!(json["version"].as_str().unwrap(), "0.1.0");
    assert!(json["data_range"]["days"].as_u64().unwrap() >= 1);
    assert!(json["dedup"]["unique_requests"].as_u64().unwrap() > 0);
    assert!(json["tokens"]["input"].as_u64().unwrap() > 0);
    assert!(json["cost"]["total"].as_f64().unwrap() > 0.0);
    assert_eq!(json["cost"]["currency"].as_str().unwrap(), "USD");
}

#[test]
fn test_dedup_streaming_chunks() {
    let (stdout, _stderr, success) = run_cc_metrics(&[
        "--path",
        fixtures_path().to_str().unwrap(),
        "--json",
    ]);

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
    assert!(skipped >= 1, "Should have at least 1 skipped line (malformed JSON)");
}

#[test]
fn test_subagent_split() {
    let (stdout, _stderr, success) = run_cc_metrics(&[
        "--path",
        fixtures_path().to_str().unwrap(),
        "--json",
    ]);

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
    let (stdout, _stderr, success) = run_cc_metrics(&[
        "--path",
        fixtures_path().to_str().unwrap(),
        "--json",
    ]);

    assert!(success);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // The streaming session's final chunk has output_tokens=365 (not 10 or 11)
    // The simple session has output_tokens=50
    // The subagent has output_tokens=200
    // The edge case has output_tokens=200
    // Total output tokens should be 365 + 50 + 200 + 200 = 815
    let output_tokens = json["tokens"]["output"].as_u64().unwrap();
    assert_eq!(output_tokens, 815, "Output tokens should reflect final chunks only");
}

#[test]
fn test_cache_tier_disaggregation() {
    let (stdout, _stderr, success) = run_cc_metrics(&[
        "--path",
        fixtures_path().to_str().unwrap(),
        "--json",
    ]);

    assert!(success);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let cache_5m = json["tokens"]["cache_write_5m"].as_u64().unwrap();
    let cache_1h = json["tokens"]["cache_write_1h"].as_u64().unwrap();

    assert!(cache_5m > 0, "Should have 5m cache writes (from subagent)");
    assert!(cache_1h > 0, "Should have 1h cache writes (from main thread)");
}

#[test]
fn test_verbose_output() {
    let (stdout, _stderr, success) = run_cc_metrics(&[
        "--path",
        fixtures_path().to_str().unwrap(),
        "--verbose",
    ]);

    assert!(success);
    assert!(stdout.contains("Verbose Details"), "Should show verbose section");
    assert!(stdout.contains("Files scanned:"), "Should show file count");
    assert!(stdout.contains("Skipped lines:"), "Should show skipped lines");
    assert!(stdout.contains("Synthetic msgs:"), "Should show synthetic count");
}

#[test]
fn test_cost_calculation_correctness() {
    let (stdout, _stderr, success) = run_cc_metrics(&[
        "--path",
        fixtures_path().to_str().unwrap(),
        "--json",
    ]);

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
}

#[test]
fn test_explain_output() {
    let (stdout, _stderr, success) = run_cc_metrics(&[
        "--path",
        fixtures_path().to_str().unwrap(),
        "explain",
    ]);

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
    let (stdout, stderr, success) = run_cc_metrics(&[
        "--path",
        temp.path().to_str().unwrap(),
    ]);

    assert!(success, "Should succeed even with no files");
    assert!(
        stderr.contains("No JSONL files found") || stdout.contains("error"),
        "Should report no files found"
    );
}
