use crate::analysis::categories::{add_event_to_breakdown, categorized_ms};
use crate::report::schema::*;
use crate::trace::{
    Category, MainThreadSelection, NetworkSidecar, SourceFrame, TraceEvent, TraceStore,
    event_store::is_task_name,
};
use std::collections::HashMap;

pub fn analyze_long_tasks(
    store: &TraceStore,
    main: &MainThreadSelection,
    threshold_ms: f64,
    sidecar: &NetworkSidecar,
    network_window_ms: f64,
) -> Vec<LongTask> {
    let mut out = Vec::new();
    let mut tasks: Vec<&TraceEvent> = store
        .events
        .iter()
        .filter(|e| {
            e.pid == main.pid
                && e.tid == main.tid
                && is_task_name(&e.name)
                && e.dur_ms() >= threshold_ms
        })
        .collect();
    tasks.sort_by(|a, b| {
        b.dur_us
            .partial_cmp(&a.dur_us)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for (idx, task) in tasks.into_iter().enumerate() {
        let children = store.child_events(task.event_id);
        let mut breakdown = Breakdown::default();
        let mut by_name: HashMap<String, (f64, usize, Category)> = HashMap::new();
        for c in &children {
            add_event_to_breakdown(&mut breakdown, c);
            let entry = by_name
                .entry(c.name.clone())
                .or_insert((0.0, 0, c.category));
            entry.0 += c.dur_ms();
            entry.1 += 1;
        }
        let known = categorized_ms(&breakdown);
        breakdown.unknown_ms = (task.dur_ms() - known).max(breakdown.unknown_ms);
        let mut child_event_breakdown: Vec<_> = by_name
            .into_iter()
            .map(|(name, (total_ms, count, category))| EventInventoryRow {
                name,
                total_ms,
                count,
                category,
            })
            .collect();
        child_event_breakdown.sort_by(|a, b| {
            b.total_ms
                .partial_cmp(&a.total_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let top_source_functions = top_sources(&children);
        let start_ms = task.ts_ms(store.origin_ts_us);
        let preceding_network_events = sidecar
            .events
            .iter()
            .filter(|e| e.ts_ms >= start_ms - network_window_ms && e.ts_ms <= start_ms)
            .take(25)
            .map(NetworkEvidence::from)
            .collect();
        let following_rendering_events = store
            .events_in_window(task.end_us, task.end_us + 50_000.0)
            .into_iter()
            .filter(|e| {
                matches!(
                    e.category,
                    Category::Style | Category::Layout | Category::Paint | Category::Composite
                )
            })
            .take(10)
            .enumerate()
            .map(|(i, e)| {
                TraceEvidence::from_event(
                    format!("ev_{:06}_f{}", e.event_id, i),
                    e,
                    store.origin_ts_us,
                )
            })
            .collect();
        out.push(LongTask {
            task_id: format!("long_task_{:03}", idx + 1),
            event_id: task.event_id,
            ts_ms: start_ms,
            dur_ms: task.dur_ms(),
            breakdown,
            child_event_breakdown,
            top_source_functions,
            preceding_network_events,
            following_rendering_events,
        });
    }
    out
}

pub fn top_sources(events: &[&TraceEvent]) -> Vec<SourceHotspot> {
    let mut map: HashMap<(String, Option<String>, String), (f64, usize)> = HashMap::new();
    for e in events {
        let frames: Vec<SourceFrame> = e
            .args
            .source
            .clone()
            .into_iter()
            .chain(e.args.stack.iter().cloned())
            .collect();
        if let Some(f) = frames.into_iter().next() {
            let function = f
                .function
                .clone()
                .unwrap_or_else(|| "(anonymous)".to_string());
            let url = f.url.clone();
            let source_type = classify_source(url.as_deref());
            let ent = map.entry((function, url, source_type)).or_default();
            ent.0 += e.dur_ms();
            ent.1 += 1;
        }
    }
    let mut rows: Vec<_> = map
        .into_iter()
        .map(
            |((function, url, source_type), (total_ms, count))| SourceHotspot {
                function,
                url,
                total_ms,
                count,
                source_type,
            },
        )
        .collect();
    rows.sort_by(|a, b| {
        b.total_ms
            .partial_cmp(&a.total_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rows.truncate(20);
    rows
}

pub fn classify_source(url: Option<&str>) -> String {
    let Some(u) = url else {
        return "browser/native".to_string();
    };
    let l = u.to_lowercase();
    if l.is_empty() || l.starts_with("native") {
        "browser/native".to_string()
    } else if l.contains("react") || l.contains("scheduler") {
        "react/runtime".to_string()
    } else if l.contains("node_modules") || l.contains("vendor") || l.contains(".min.") {
        "node_modules".to_string()
    } else {
        "app".to_string()
    }
}
