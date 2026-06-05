use crate::analysis::long_tasks::top_sources;
use crate::report::schema::{LayoutAnalysis, LayoutFinding};
use crate::trace::{Category, MainThreadSelection, TraceEvent, TraceStore};

pub fn analyze_layout(
    store: &TraceStore,
    main: &MainThreadSelection,
    threshold_ms: f64,
) -> LayoutAnalysis {
    let main_events: Vec<&TraceEvent> = store
        .events
        .iter()
        .filter(|e| e.pid == main.pid && e.tid == main.tid)
        .collect();
    let total_style_ms = main_events
        .iter()
        .filter(|e| e.category == Category::Style)
        .map(|e| e.dur_ms())
        .sum();
    let total_layout_ms = main_events
        .iter()
        .filter(|e| e.category == Category::Layout)
        .map(|e| e.dur_ms())
        .sum();
    let mut expensive_events = Vec::new();
    let mut forced_reflow_candidates = Vec::new();
    for e in main_events.iter().copied().filter(|e| {
        matches!(e.category, Category::Style | Category::Layout) && e.dur_ms() >= threshold_ms
    }) {
        let nearby = store.events_in_window(e.ts_us - 25_000.0, e.end_us + 25_000.0);
        let js: Vec<&TraceEvent> = nearby
            .into_iter()
            .filter(|n| n.category == Category::Js && n.args.source.is_some())
            .collect();
        let row = LayoutFinding {
            event_id: e.event_id,
            name: e.name.clone(),
            ts_ms: e.ts_ms(store.origin_ts_us),
            dur_ms: e.dur_ms(),
            dirty_objects: e.args.dirty_objects,
            total_objects: e.args.total_objects,
            nearby_js: top_sources(&js),
        };
        if e.parent_id
            .and_then(|id| store.event(id))
            .is_some_and(|p| p.category == Category::Js || p.name.contains("RunTask"))
        {
            forced_reflow_candidates.push(row.clone());
        }
        expensive_events.push(row);
    }
    expensive_events.sort_by(|a, b| {
        b.dur_ms
            .partial_cmp(&a.dur_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    forced_reflow_candidates.sort_by(|a, b| {
        b.dur_ms
            .partial_cmp(&a.dur_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    LayoutAnalysis {
        expensive_events,
        forced_reflow_candidates,
        total_style_ms,
        total_layout_ms,
    }
}
