use aiperf::analysis::{AuditOptions, audit_trace, compare_traces};
use aiperf::report::{
    llm_pack::{pack_from_report, prompt_for_pack},
    markdown::{audit_markdown, compare_markdown},
};
use std::path::PathBuf;

fn opts(sidecar: Option<&str>) -> AuditOptions {
    AuditOptions {
        project_root: PathBuf::from("."),
        network_sidecar: sidecar.map(PathBuf::from),
        long_task_ms: Some(50.0),
        bucket_ms: Some(100.0),
        top_n: None,
    }
}

#[test]
fn parses_minimal_trace_metadata_and_main_thread() {
    let report = audit_trace("tests/fixtures/minimal_trace.json".as_ref(), &opts(None)).unwrap();
    assert_eq!(report.coverage.event_count_scanned, 4);
    assert_eq!(report.coverage.selected_main_thread.tid, 10);
    assert_eq!(report.metadata.cpu_throttling, Some(4.0));
    assert!(
        report
            .metadata
            .page_url
            .as_deref()
            .unwrap()
            .contains("localhost:3000")
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("Network sidecar absent"))
    );
}

#[test]
fn detects_long_task_nested_breakdown_react_layout_gc_and_cpu() {
    let report = audit_trace(
        "tests/fixtures/realtime_ws_trace.json".as_ref(),
        &opts(Some("tests/fixtures/realtime_ws_trace.network.jsonl")),
    )
    .unwrap();
    assert_eq!(report.long_tasks.len(), 1);
    let task = &report.long_tasks[0];
    assert!(task.breakdown.js_ms >= 52.0);
    assert!(task.breakdown.react_ms >= 28.0);
    assert!(task.breakdown.style_ms >= 18.0);
    assert!(task.breakdown.layout_ms >= 22.0);
    assert!(report.react.tracks_present);
    assert!(!report.react.long_commits.is_empty());
    assert!(!report.layout.expensive_events.is_empty());
    assert_eq!(report.gc.events.len(), 1);
    assert!(
        report
            .cpu_profile
            .functions
            .iter()
            .any(|f| f.function == "applyPriceBatch")
    );
}

#[test]
fn detects_websocket_burst_and_correlates_with_task_without_payloads() {
    let report = audit_trace(
        "tests/fixtures/realtime_ws_trace.json".as_ref(),
        &opts(Some("tests/fixtures/realtime_ws_trace.network.jsonl")),
    )
    .unwrap();
    assert_eq!(report.realtime.sidecar_event_count, 3);
    assert!(!report.realtime.bursts.is_empty());
    assert!(!report.realtime.correlations.is_empty());
    let ev = &report.realtime.bursts[0].evidence[0];
    assert_eq!(ev.direction.as_deref(), Some("received"));
    assert!(!ev.url.as_deref().unwrap_or_default().contains("token"));
}

#[test]
fn markdown_json_and_pack_include_required_sections_and_evidence() {
    let report = audit_trace(
        "tests/fixtures/realtime_ws_trace.json".as_ref(),
        &opts(Some("tests/fixtures/realtime_ws_trace.network.jsonl")),
    )
    .unwrap();
    let md = audit_markdown(&report);
    assert!(md.contains("AI Performance Audit"));
    assert!(md.contains("Analysis Coverage"));
    assert!(md.contains("Top Findings"));
    assert!(md.contains("Long Tasks"));
    assert!(md.contains("LLM Evidence Pack"));
    let json = serde_json::to_value(&report).unwrap();
    assert!(json.get("findings").is_some());
    assert!(json["findings"][0].get("evidence").is_some());
    let pack = pack_from_report(&report, 10);
    assert!(!pack.top_findings.is_empty());
    assert!(prompt_for_pack("llm-pack.json".as_ref()).contains("Do not invent trace facts"));
}

#[test]
fn compare_reports_metric_deltas_and_pr_summary() {
    let report = compare_traces(
        "tests/fixtures/compare_before.json".as_ref(),
        "tests/fixtures/compare_after.json".as_ref(),
        &opts(None),
    )
    .unwrap();
    assert!(report.deltas.long_tasks < 0);
    assert!(report.deltas.total_busy_ms < 0.0);
    assert!(report.improvements.iter().any(|s| s.contains("long task")));
    let md = compare_markdown(&report);
    assert!(md.contains("AI Performance Before/After Comparison"));
    assert!(md.contains("Metric Deltas"));
}

#[test]
fn large_trace_smoke_scans_all_events() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large.json");
    let mut events = vec![
        serde_json::json!({ "name": "thread_name", "ph": "M", "ts": 0, "pid": 1, "tid": 10, "args": { "name": "CrRendererMain" } }),
    ];
    for i in 0..5000u64 {
        events.push(serde_json::json!({ "name": "RunTask", "cat": "devtools.timeline", "ph": "X", "ts": 1000 + i * 1000, "dur": 100, "pid": 1, "tid": 10, "args": {} }));
    }
    std::fs::write(
        &path,
        serde_json::json!({"traceEvents": events}).to_string(),
    )
    .unwrap();
    let report = audit_trace(&path, &opts(None)).unwrap();
    assert_eq!(report.coverage.event_count_scanned, 5001);
    assert!(!report.coverage.time_buckets.is_empty());
}
