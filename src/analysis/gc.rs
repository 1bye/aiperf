use crate::report::schema::{GcAnalysis, GcEvent};
use crate::trace::{Category, MainThreadSelection, TraceStore};

pub fn analyze_gc(store: &TraceStore, main: &MainThreadSelection, threshold_ms: f64) -> GcAnalysis {
    let mut events: Vec<_> = store
        .events
        .iter()
        .filter(|e| {
            e.pid == main.pid
                && e.tid == main.tid
                && e.category == Category::Gc
                && e.dur_ms() >= threshold_ms
        })
        .map(|e| GcEvent {
            event_id: e.event_id,
            name: e.name.clone(),
            ts_ms: e.ts_ms(store.origin_ts_us),
            dur_ms: e.dur_ms(),
        })
        .collect();
    events.sort_by(|a, b| {
        b.dur_ms
            .partial_cmp(&a.dur_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let total_ms = events.iter().map(|e| e.dur_ms).sum();
    GcAnalysis { events, total_ms }
}
