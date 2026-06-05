use crate::report::schema::*;

pub fn compare_reports(before: AuditReport, after: AuditReport) -> CompareReport {
    let before_busy = before
        .coverage
        .time_buckets
        .iter()
        .map(|b| b.main_thread_busy_ms)
        .sum::<f64>();
    let after_busy = after
        .coverage
        .time_buckets
        .iter()
        .map(|b| b.main_thread_busy_ms)
        .sum::<f64>();
    let before_worst = before.long_tasks.first().map(|t| t.dur_ms).unwrap_or(0.0);
    let after_worst = after.long_tasks.first().map(|t| t.dur_ms).unwrap_or(0.0);
    let deltas = CompareDeltas {
        long_tasks: after.long_tasks.len() as isize - before.long_tasks.len() as isize,
        total_busy_ms: after_busy - before_busy,
        worst_long_task_ms: after_worst - before_worst,
        react_events: after.react.event_count as isize - before.react.event_count as isize,
        websocket_bursts: after.realtime.bursts.len() as isize
            - before.realtime.bursts.len() as isize,
    };
    let mut regressions = Vec::new();
    let mut improvements = Vec::new();
    classify_delta(
        "long task count",
        deltas.long_tasks as f64,
        &mut regressions,
        &mut improvements,
    );
    classify_delta(
        "main-thread busy ms",
        deltas.total_busy_ms,
        &mut regressions,
        &mut improvements,
    );
    classify_delta(
        "worst long task ms",
        deltas.worst_long_task_ms,
        &mut regressions,
        &mut improvements,
    );
    classify_delta(
        "React event count",
        deltas.react_events as f64,
        &mut regressions,
        &mut improvements,
    );
    classify_delta(
        "WebSocket/network bursts",
        deltas.websocket_bursts as f64,
        &mut regressions,
        &mut improvements,
    );
    let changed_root_causes = changed_root_causes(&before.findings, &after.findings);
    let verdict = if regressions.len() > improvements.len() {
        "Regressed"
    } else if improvements.len() > regressions.len() {
        "Improved"
    } else {
        "Mixed/no significant change"
    };
    let pr_summary = format!(
        "Overall: {verdict}. Long tasks {:+}, busy {:+.2}ms, worst long task {:+.2}ms, React events {:+}, network bursts {:+}.",
        deltas.long_tasks,
        deltas.total_busy_ms,
        deltas.worst_long_task_ms,
        deltas.react_events,
        deltas.websocket_bursts
    );
    CompareReport {
        before: Box::new(before),
        after: Box::new(after),
        deltas,
        regressions,
        improvements,
        changed_root_causes,
        pr_summary,
    }
}
fn classify_delta(
    name: &str,
    delta: f64,
    regressions: &mut Vec<String>,
    improvements: &mut Vec<String>,
) {
    if delta > 0.0 {
        regressions.push(format!("{name} increased by {:.2}", delta));
    } else if delta < 0.0 {
        improvements.push(format!("{name} decreased by {:.2}", delta.abs()));
    }
}
fn changed_root_causes(before: &[Finding], after: &[Finding]) -> Vec<String> {
    let before_titles: std::collections::BTreeSet<_> =
        before.iter().map(|f| f.title.as_str()).collect();
    after
        .iter()
        .filter(|f| !before_titles.contains(f.title.as_str()))
        .map(|f| f.title.clone())
        .collect()
}
