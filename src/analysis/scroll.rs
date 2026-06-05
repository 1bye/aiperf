use crate::analysis::categories::add_event_to_breakdown;
use crate::report::schema::{Breakdown, ScrollAnalysis};
use crate::trace::{Category, MainThreadSelection, TraceStore, event_store::is_task_name};

pub fn analyze_scroll(store: &TraceStore, main: &MainThreadSelection) -> ScrollAnalysis {
    let mut durs = Vec::new();
    let mut breakdown = Breakdown::default();
    let mut thrashing = 0usize;
    let mut intersection = 0usize;
    for task in store
        .events
        .iter()
        .filter(|e| e.pid == main.pid && e.tid == main.tid && is_task_name(&e.name))
    {
        let children = store.child_events(task.event_id);
        let scroll_related = children.iter().any(|e| {
            matches!(
                e.category,
                Category::Scroll | Category::Input | Category::HitTest
            ) || e.name.contains("Scroll")
                || e.name == "IntersectionObserverController::computeIntersections"
        }) || children
            .iter()
            .any(|e| e.name == "UpdateLayoutTree" && e.dur_ms() > 16.0);
        if !scroll_related {
            continue;
        }
        durs.push(task.dur_ms());
        let mut last_was_js = false;
        let mut alternations = 0;
        for c in children {
            add_event_to_breakdown(&mut breakdown, c);
            if c.name == "IntersectionObserverController::computeIntersections" {
                intersection += 1;
            }
            match c.category {
                Category::Js => last_was_js = true,
                Category::Style | Category::Layout if last_was_js => {
                    alternations += 1;
                    last_was_js = false;
                }
                _ => {}
            }
        }
        if alternations >= 2 {
            thrashing += 1;
        }
    }
    durs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let count = durs.len();
    let avg_ms = if count > 0 {
        durs.iter().sum::<f64>() / count as f64
    } else {
        0.0
    };
    ScrollAnalysis {
        task_count: count,
        avg_ms,
        p50_ms: pct(&durs, 50.0),
        p75_ms: pct(&durs, 75.0),
        p90_ms: pct(&durs, 90.0),
        p95_ms: pct(&durs, 95.0),
        p99_ms: pct(&durs, 99.0),
        max_ms: durs.last().copied().unwrap_or(0.0),
        breakdown,
        layout_thrashing_tasks: thrashing,
        intersection_observer_events: intersection,
    }
}
fn pct(v: &[f64], p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let idx = ((p / 100.0) * (v.len() as f64 - 1.0)).round() as usize;
    v[idx.min(v.len() - 1)]
}
