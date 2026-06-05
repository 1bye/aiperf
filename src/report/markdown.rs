use crate::report::schema::*;

pub fn audit_markdown(r: &AuditReport) -> String {
    let mut out = String::new();
    out.push_str("# AI Performance Audit\n\n");
    out.push_str("## Executive Summary\n\n");
    let top = r.findings.first();
    out.push_str(&format!(
        "- **Overall diagnosis**: {}\n",
        top.map(|f| f.title.as_str()).unwrap_or("No findings")
    ));
    out.push_str("- **Top 3 root causes**:\n");
    for f in r.findings.iter().take(3) {
        out.push_str(&format!(
            "  - {} ({:?}, {:?})\n",
            f.title, f.severity, f.confidence
        ));
    }
    let biggest = r
        .coverage
        .top_event_names_by_total_duration
        .first()
        .map(|e| format!("{} / {}", e.category.as_str(), e.name))
        .unwrap_or_else(|| "unknown".to_string());
    out.push_str(&format!("- **Biggest bottleneck category**: {biggest}\n"));
    out.push_str(&format!(
        "- **Trace reliability**: data_loss={} truncated={}\n",
        r.coverage.data_loss, r.coverage.trace_appears_truncated
    ));
    out.push_str(&format!(
        "- **Evidence availability**: React tracks={} source maps={} network sidecar events={}\n\n",
        r.coverage.react_tracks_present,
        r.coverage.source_maps_present,
        r.coverage.network_sidecar_event_count
    ));
    if !r.warnings.is_empty() {
        out.push_str("### Warnings\n\n");
        for w in &r.warnings {
            out.push_str(&format!("- {w}\n"));
        }
        out.push('\n');
    }
    coverage_markdown(&mut out, &r.coverage);
    out.push_str("## Top Findings\n\n");
    for f in &r.findings {
        finding_markdown(&mut out, f);
    }
    long_tasks_markdown(&mut out, r);
    react_markdown(&mut out, &r.react);
    realtime_markdown(&mut out, &r.realtime);
    layout_markdown(&mut out, &r.layout);
    cpu_markdown(&mut out, &r.cpu_profile);
    out.push_str("## LLM Evidence Pack\n\nPath is determined by `--evidence-out` or `aiperf pack --out`.\n\n");
    out.push_str("## Appendix\n\n### Event inventory by total duration\n\n| Event | Category | Count | Total |\n|---|---:|---:|---:|\n");
    for e in &r.coverage.top_event_names_by_total_duration {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            e.name,
            e.category.as_str(),
            e.count,
            fmt_ms(e.total_ms)
        ));
    }
    out.push_str("\n### Thresholds used\n\nLong task threshold and bucket size come from `.aiperf.toml` or CLI overrides.\n");
    out
}

fn coverage_markdown(out: &mut String, c: &AnalysisCoverage) {
    out.push_str("## Analysis Coverage\n\n");
    out.push_str(&format!(
        "- **Trace file**: {} ({} bytes)\n",
        c.trace_file_path.display(),
        c.trace_file_size_bytes
    ));
    out.push_str(&format!(
        "- **Events scanned**: {}\n",
        c.event_count_scanned
    ));
    out.push_str(&format!(
        "- **Trace duration**: {}\n",
        fmt_ms(c.trace_duration_ms)
    ));
    out.push_str(&format!("- **PIDs**: {:?}\n", c.pids));
    out.push_str(&format!(
        "- **Selected main thread**: pid={} tid={} confidence={} ({})\n",
        c.selected_main_thread.pid,
        c.selected_main_thread.tid,
        c.selected_main_thread.confidence,
        c.selected_main_thread.explanation
    ));
    out.push_str(&format!("- **Data loss**: {}\n", c.data_loss));
    out.push_str(&format!(
        "- **Unknown event names**: {}\n",
        c.unknown_event_name_count
    ));
    out.push_str(&format!("- **Long tasks found**: {}\n", c.long_task_count));
    out.push_str(&format!(
        "- **React events found**: {}\n",
        c.react_event_count
    ));
    out.push_str(&format!(
        "- **Network/WebSocket sidecar events**: {}\n",
        c.network_sidecar_event_count
    ));
    out.push_str("\n### Time bucket scan summary\n\n| Bucket | Window | Busy | JS | Style | Layout | Paint | Composite | GC | React | Unknown |\n|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    for b in c.time_buckets.iter().take(30) {
        out.push_str(&format!(
            "| {} | {:.0}-{:.0}ms | {:.1}% | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            b.index,
            b.start_ms,
            b.end_ms,
            b.busy_pct,
            fmt_ms(b.js_ms),
            fmt_ms(b.style_ms),
            fmt_ms(b.layout_ms),
            fmt_ms(b.paint_ms),
            fmt_ms(b.composite_ms),
            fmt_ms(b.gc_ms),
            fmt_ms(b.react_ms),
            fmt_ms(b.unknown_ms)
        ));
    }
    out.push_str("\n### Anomaly windows\n\n");
    for a in &c.anomaly_windows {
        out.push_str(&format!("- {}: {}\n", a.kind, a.summary));
    }
    out.push('\n');
}
fn finding_markdown(out: &mut String, f: &Finding) {
    out.push_str(&format!("### {}: {}\n\nSeverity: {:?}\nConfidence: {:?}\nImpact: total {} worst {} count {}\n\nCause chain:\n", f.id, f.title, f.severity, f.confidence, fmt_ms(f.impact.total_ms), fmt_ms(f.impact.worst_ms), f.impact.count));
    for (i, s) in f.cause_chain.iter().enumerate() {
        out.push_str(&format!(
            "{}. {} — {} Evidence: {:?}\n",
            i + 1,
            s.step,
            s.summary,
            s.evidence_ids
        ));
    }
    out.push_str("\nEvidence:\n");
    for e in &f.evidence {
        out.push_str(&format!(
            "- {} trace_event={} {} @{} dur={} pid={} tid={} category={}\n",
            e.evidence_id,
            e.trace_event_id,
            e.name,
            fmt_ms(e.ts_ms),
            fmt_ms(e.dur_ms),
            e.pid,
            e.tid,
            e.category.as_str()
        ));
    }
    out.push_str("\nRecommended fix:\n");
    for r in &f.recommendations {
        out.push_str(&format!(
            "- **{}**: {} Files: {:?}. Risk: {}\n",
            r.title, r.suggested_change, r.files_to_inspect, r.risk
        ));
    }
    out.push_str("\nHow to verify:\n");
    out.push_str(&format!(
        "- {} Expected: {}\n\n",
        f.verification.how_to_measure_after_fix, f.verification.expected_metric_change
    ));
}
fn long_tasks_markdown(out: &mut String, r: &AuditReport) {
    out.push_str("## Long Tasks\n\n");
    out.push_str(&format!(
        "Total long tasks in JSON: {}\n\n",
        r.long_tasks.len()
    ));
    out.push_str("| ID | Event | Start | Duration | JS | React | Style | Layout | Paint/Composite | GC |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    for t in r.long_tasks.iter().take(20) {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            t.task_id,
            t.event_id,
            fmt_ms(t.ts_ms),
            fmt_ms(t.dur_ms),
            fmt_ms(t.breakdown.js_ms),
            fmt_ms(t.breakdown.react_ms),
            fmt_ms(t.breakdown.style_ms),
            fmt_ms(t.breakdown.layout_ms),
            fmt_ms(t.breakdown.paint_composite_ms),
            fmt_ms(t.breakdown.gc_ms)
        ));
    }
    out.push('\n');
}
fn react_markdown(out: &mut String, r: &ReactAnalysis) {
    out.push_str("## React Runtime\n\n");
    out.push_str(&format!(
        "- Tracks present: {}\n- Events: {} inferred: {}\n\n",
        r.tracks_present, r.event_count, r.inferred_event_count
    ));
    for h in r.component_hotspots.iter().take(10) {
        out.push_str(&format!(
            "- {:?} phase={} total={} count={} p95={} evidence={}\n",
            h.component,
            h.phase,
            fmt_ms(h.total_ms),
            h.count,
            fmt_ms(h.p95_ms),
            h.evidence_kind
        ));
    }
    out.push('\n');
}
fn realtime_markdown(out: &mut String, r: &RealtimeAnalysis) {
    out.push_str("## Realtime / Network Bursts\n\n");
    out.push_str(&format!("Sidecar events: {}\n\n", r.sidecar_event_count));
    for b in r.bursts.iter().take(10) {
        out.push_str(&format!(
            "- {} {} count={} bytes={} window={}-{}ms\n",
            b.burst_id, b.kind, b.count, b.payload_bytes, b.start_ms, b.end_ms
        ));
    }
    for c in &r.correlations {
        out.push_str(&format!(
            "- Correlation {} -> task {} confidence={}: {}\n",
            c.burst_id, c.long_task_event_id, c.confidence, c.summary
        ));
    }
    out.push('\n');
}
fn layout_markdown(out: &mut String, l: &LayoutAnalysis) {
    out.push_str("## Layout / Style / Paint\n\n");
    out.push_str(&format!(
        "Style total={} layout total={}\n\n",
        fmt_ms(l.total_style_ms),
        fmt_ms(l.total_layout_ms)
    ));
    for e in l.expensive_events.iter().take(10) {
        out.push_str(&format!(
            "- {} event={} {} dur={} dirty={:?}/{:?}\n",
            e.name,
            e.event_id,
            fmt_ms(e.ts_ms),
            fmt_ms(e.dur_ms),
            e.dirty_objects,
            e.total_objects
        ));
    }
    out.push('\n');
}
fn cpu_markdown(out: &mut String, c: &CpuProfileAnalysis) {
    out.push_str("## CPU Profile / Source Hotspots\n\n");
    if c.functions.is_empty() {
        out.push_str("No CPU profile data found.\n\n");
        return;
    }
    out.push_str(&format!(
        "Total sample={} app={} third_party={} react_runtime={} native={} unresolved={}\n\n",
        fmt_ms(c.total_sample_ms),
        fmt_ms(c.app_ms),
        fmt_ms(c.third_party_ms),
        fmt_ms(c.react_runtime_ms),
        fmt_ms(c.native_ms),
        c.unresolved_or_minified_frames
    ));
    for f in c.functions.iter().take(20) {
        out.push_str(&format!(
            "- {} {} {} {:?}\n",
            f.source_type,
            f.function,
            fmt_ms(f.total_ms),
            f.url
        ));
    }
    out.push('\n');
}

pub fn compare_markdown(c: &CompareReport) -> String {
    let mut out = String::new();
    out.push_str("# AI Performance Before/After Comparison\n\n");
    out.push_str(&format!("{}\n\n", c.pr_summary));
    out.push_str("## Metric Deltas\n\n");
    out.push_str(&format!("- Long tasks: {:+}\n- Main-thread busy: {:+.2}ms\n- Worst long task: {:+.2}ms\n- React events: {:+}\n- WebSocket/network bursts: {:+}\n\n", c.deltas.long_tasks, c.deltas.total_busy_ms, c.deltas.worst_long_task_ms, c.deltas.react_events, c.deltas.websocket_bursts));
    out.push_str("## Regressions\n\n");
    for r in &c.regressions {
        out.push_str(&format!("- {r}\n"));
    }
    out.push_str("\n## Improvements\n\n");
    for i in &c.improvements {
        out.push_str(&format!("- {i}\n"));
    }
    out.push_str("\n## Changed root causes\n\n");
    for r in &c.changed_root_causes {
        out.push_str(&format!("- {r}\n"));
    }
    out
}

fn fmt_ms(ms: f64) -> String {
    if ms.abs() >= 1000.0 {
        format!("{:.2}s", ms / 1000.0)
    } else {
        format!("{:.2}ms", ms)
    }
}
