use crate::report::schema::*;
use crate::trace::{
    Category, MainThreadSelection, NetworkSidecar, ReactInfo, TraceEvent, TraceStore,
};
use std::collections::{BTreeMap, HashMap};

pub fn analysis_coverage(
    store: &TraceStore,
    main: &MainThreadSelection,
    sidecar: &NetworkSidecar,
    bucket_ms: f64,
    react: &ReactAnalysis,
    long_task_count: usize,
) -> AnalysisCoverage {
    let mut duration_by_name: HashMap<String, (f64, usize, Category)> = HashMap::new();
    let mut count_by_name: HashMap<String, (f64, usize, Category)> = HashMap::new();
    let mut unknown = BTreeMap::new();
    for e in &store.events {
        let dur_ms = e.dur_ms();
        let entry = duration_by_name
            .entry(e.name.clone())
            .or_insert((0.0, 0, e.category));
        entry.0 += dur_ms;
        entry.1 += 1;
        let centry = count_by_name
            .entry(e.name.clone())
            .or_insert((0.0, 0, e.category));
        centry.0 += dur_ms;
        centry.1 += 1;
        if e.category == Category::Unknown {
            *unknown.entry(e.name.clone()).or_insert(0usize) += 1;
        }
    }
    let mut top_duration: Vec<_> = duration_by_name
        .into_iter()
        .map(|(name, (total_ms, count, category))| EventInventoryRow {
            name,
            total_ms,
            count,
            category,
        })
        .collect();
    top_duration.sort_by(|a, b| {
        b.total_ms
            .partial_cmp(&a.total_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    top_duration.truncate(30);
    let mut top_count: Vec<_> = count_by_name
        .into_iter()
        .map(|(name, (total_ms, count, category))| EventInventoryRow {
            name,
            total_ms,
            count,
            category,
        })
        .collect();
    top_count.sort_by_key(|row| std::cmp::Reverse(row.count));
    top_count.truncate(30);
    let buckets = time_buckets(store, main, bucket_ms, sidecar);
    let anomalies = anomaly_windows(&buckets);
    AnalysisCoverage {
        trace_file_path: store.metadata.trace_path.clone(),
        trace_file_size_bytes: store.metadata.file_size_bytes,
        event_count_scanned: store.events.len(),
        trace_duration_ms: store.duration_ms(),
        pids: store.unique_pids(),
        tids_analyzed: store
            .threads
            .iter()
            .map(|t| ThreadCoverage {
                pid: t.pid,
                tid: t.tid,
                thread_name: t.thread_name.clone(),
                process_name: t.process_name.clone(),
                event_count: t.event_count,
            })
            .collect(),
        selected_main_thread: main.clone(),
        data_loss: store.metadata.data_loss,
        unknown_event_name_count: unknown.len(),
        top_event_names_by_total_duration: top_duration,
        top_event_names_by_count: top_count,
        time_buckets: buckets,
        anomaly_windows: anomalies,
        long_task_count,
        react_event_count: react.event_count,
        network_sidecar_event_count: sidecar.events.len(),
        react_tracks_present: react.tracks_present,
        source_maps_present: crate::trace::source_maps::source_maps_available(
            std::path::Path::new("."),
        ),
        trace_appears_truncated: store.metadata.data_loss || store.events.len() < 2,
    }
}

pub fn time_buckets(
    store: &TraceStore,
    main: &MainThreadSelection,
    bucket_ms: f64,
    sidecar: &NetworkSidecar,
) -> Vec<TimeBucket> {
    let duration_ms = store.duration_ms();
    let bucket_ms = bucket_ms.max(1.0);
    let bucket_count = ((duration_ms / bucket_ms).ceil() as usize).max(1);
    let mut buckets = (0..bucket_count)
        .map(|i| TimeBucket {
            index: i,
            start_ms: i as f64 * bucket_ms,
            end_ms: (i + 1) as f64 * bucket_ms,
            main_thread_busy_ms: 0.0,
            js_ms: 0.0,
            style_ms: 0.0,
            layout_ms: 0.0,
            paint_ms: 0.0,
            composite_ms: 0.0,
            gc_ms: 0.0,
            react_ms: 0.0,
            network_adjacent_ms: 0.0,
            unknown_ms: 0.0,
            busy_pct: 0.0,
        })
        .collect::<Vec<_>>();
    for e in &store.events {
        if e.pid != main.pid || e.tid != main.tid || !e.is_complete() {
            continue;
        }
        distribute_event(store, &mut buckets, bucket_ms, e);
    }
    for ev in &sidecar.events {
        if ev.ts_ms >= 0.0 && duration_ms > 0.0 {
            let idx = (ev.ts_ms / bucket_ms).floor() as usize;
            if let Some(b) = buckets.get_mut(idx) {
                b.network_adjacent_ms += ev.payload_bytes.unwrap_or(1) as f64 / 1000.0;
            }
        }
    }
    for b in &mut buckets {
        b.busy_pct = (b.main_thread_busy_ms / (b.end_ms - b.start_ms).max(1.0) * 100.0).min(100.0);
    }
    buckets
}

fn distribute_event(
    store: &TraceStore,
    buckets: &mut [TimeBucket],
    bucket_ms: f64,
    e: &TraceEvent,
) {
    let start_ms = e.ts_ms(store.origin_ts_us);
    let end_ms = start_ms + e.dur_ms();
    let start_idx = (start_ms / bucket_ms).floor().max(0.0) as usize;
    let end_idx = (end_ms / bucket_ms).floor().max(0.0) as usize;
    for idx in start_idx..=end_idx {
        let Some(b) = buckets.get_mut(idx) else {
            continue;
        };
        let clipped = (end_ms.min(b.end_ms) - start_ms.max(b.start_ms)).max(0.0);
        if clipped <= 0.0 {
            continue;
        }
        if crate::trace::event_store::is_task_name(&e.name) {
            b.main_thread_busy_ms += clipped;
        }
        match e.category {
            Category::Js
            | Category::Timers
            | Category::AnimationFrame
            | Category::Input
            | Category::Scroll => b.js_ms += clipped,
            Category::React => b.react_ms += clipped,
            Category::Style => b.style_ms += clipped,
            Category::Layout => b.layout_ms += clipped,
            Category::Paint => b.paint_ms += clipped,
            Category::Composite | Category::Raster | Category::Gpu => b.composite_ms += clipped,
            Category::Gc => b.gc_ms += clipped,
            Category::Unknown => b.unknown_ms += clipped,
            _ => {}
        }
    }
}

type BucketMetric = (&'static str, fn(&TimeBucket) -> f64);
fn anomaly_windows(buckets: &[TimeBucket]) -> Vec<AnomalyWindow> {
    let metrics: [BucketMetric; 7] = [
        ("highest_main_thread_busy_pct", |b| b.busy_pct),
        ("highest_js_time", |b| b.js_ms),
        ("highest_style_layout_time", |b| b.style_ms + b.layout_ms),
        ("highest_paint_composite_time", |b| {
            b.paint_ms + b.composite_ms
        }),
        ("highest_gc_time", |b| b.gc_ms),
        ("highest_react_time", |b| b.react_ms),
        ("densest_network_burst", |b| b.network_adjacent_ms),
    ];
    metrics
        .iter()
        .filter_map(|(kind, f)| {
            buckets
                .iter()
                .max_by(|a, b| f(a).partial_cmp(&f(b)).unwrap_or(std::cmp::Ordering::Equal))
                .map(|b| AnomalyWindow {
                    kind: (*kind).to_string(),
                    bucket_index: b.index,
                    start_ms: b.start_ms,
                    end_ms: b.end_ms,
                    value_ms: f(b),
                    summary: format!(
                        "{} at {:.2}-{:.2}ms value {:.2}",
                        kind,
                        b.start_ms,
                        b.end_ms,
                        f(b)
                    ),
                })
        })
        .collect()
}

#[allow(dead_code)]
fn _react_present(events: &[TraceEvent]) -> bool {
    events.iter().any(|e| {
        matches!(
            e.args.react,
            Some(ReactInfo {
                inferred: false,
                ..
            })
        )
    })
}
