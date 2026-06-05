use crate::report::schema::{NetworkBurst, RealtimeAnalysis, RealtimeCorrelation};
use crate::trace::{NetworkSidecar, TraceStore, event_store::is_task_name};
use std::collections::HashMap;

pub fn analyze_realtime(
    store: &TraceStore,
    sidecar: &NetworkSidecar,
    window_ms: f64,
    corr_ms: f64,
) -> RealtimeAnalysis {
    let mut events = sidecar.events.clone();
    events.sort_by(|a, b| {
        a.ts_ms
            .partial_cmp(&b.ts_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut bursts = Vec::new();
    let mut i = 0;
    while i < events.len() {
        let start = events[i].ts_ms;
        let kind = burst_kind(&events[i].kind);
        let mut j = i;
        let mut payload = 0u64;
        let mut evidence = Vec::new();
        while j < events.len()
            && events[j].ts_ms <= start + window_ms
            && burst_kind(&events[j].kind) == kind
        {
            payload = payload.saturating_add(events[j].payload_bytes.unwrap_or(0));
            evidence.push((&events[j]).into());
            j += 1;
        }
        if evidence.len() >= 2 || payload >= 64 * 1024 {
            bursts.push(NetworkBurst {
                burst_id: format!("burst_{:03}", bursts.len() + 1),
                kind: kind.to_string(),
                start_ms: start,
                end_ms: events[j.saturating_sub(1)].ts_ms,
                count: evidence.len(),
                payload_bytes: payload,
                evidence,
            });
            i = j;
        } else {
            i += 1;
        }
    }
    let correlations = correlate(store, &bursts, corr_ms);
    RealtimeAnalysis {
        sidecar_event_count: sidecar.events.len(),
        bursts,
        correlations,
    }
}

fn burst_kind(method: &str) -> &'static str {
    if method.contains("webSocket") || method.contains("WebSocket") {
        "websocket"
    } else if method.contains("eventSource") || method.contains("EventSource") {
        "event_source"
    } else if method.contains("response")
        || method.contains("loading")
        || method.contains("request")
    {
        "http"
    } else {
        "network"
    }
}

fn correlate(
    store: &TraceStore,
    bursts: &[NetworkBurst],
    corr_ms: f64,
) -> Vec<RealtimeCorrelation> {
    let mut out = Vec::new();
    for burst in bursts {
        let window_start = store.origin_ts_us + (burst.end_ms * 1000.0);
        let window_end = window_start + corr_ms * 1000.0;
        if let Some(task) = store
            .events_in_window(window_start, window_end)
            .into_iter()
            .filter(|e| is_task_name(&e.name) && e.dur_ms() >= 50.0)
            .max_by(|a, b| {
                a.dur_us
                    .partial_cmp(&b.dur_us)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        {
            out.push(RealtimeCorrelation {
                burst_id: burst.burst_id.clone(),
                long_task_event_id: task.event_id,
                task_ts_ms: task.ts_ms(store.origin_ts_us),
                task_dur_ms: task.dur_ms(),
                confidence: if burst.count >= 3 { "medium" } else { "low" }.to_string(),
                summary: format!(
                    "{} {} messages / {} bytes ended {:.2}ms before {:.2}ms main-thread task",
                    burst.kind,
                    burst.count,
                    burst.payload_bytes,
                    (task.ts_ms(store.origin_ts_us) - burst.end_ms).max(0.0),
                    task.dur_ms()
                ),
            });
        }
    }
    dedup_correlations(out)
}
fn dedup_correlations(mut rows: Vec<RealtimeCorrelation>) -> Vec<RealtimeCorrelation> {
    let mut seen = HashMap::new();
    rows.retain(|r| {
        seen.insert((r.burst_id.clone(), r.long_task_event_id), ())
            .is_none()
    });
    rows
}
