use crate::report::schema::{CpuProfileAnalysis, ReactAnalysis, ReactFinding};
use crate::trace::{Category, SourceFrame, TraceEvent, TraceStore};
use std::collections::HashMap;

pub fn analyze_react(
    store: &TraceStore,
    cpu: &CpuProfileAnalysis,
    commit_threshold_ms: f64,
) -> ReactAnalysis {
    let react_events: Vec<&TraceEvent> = store
        .events
        .iter()
        .filter(|e| e.category == Category::React || e.args.react.is_some())
        .collect();
    let tracks_present = react_events
        .iter()
        .any(|e| e.args.react.as_ref().is_some_and(|r| !r.inferred));
    let inferred_event_count = react_events
        .iter()
        .filter(|e| e.args.react.as_ref().is_some_and(|r| r.inferred))
        .count();
    let mut long_commits = Vec::new();
    let mut cascading_updates = Vec::new();
    let mut grouped: HashMap<(Option<String>, String, String), Vec<&TraceEvent>> = HashMap::new();
    for e in &react_events {
        let info = e.args.react.as_ref();
        let phase = info
            .and_then(|r| r.phase.clone())
            .unwrap_or_else(|| infer_phase(e.name.as_str()));
        let component = info.and_then(|r| r.component.clone());
        let kind = if info.is_some_and(|r| r.inferred) {
            "inferred_call_stack"
        } else {
            "react_performance_track"
        }
        .to_string();
        grouped
            .entry((component.clone(), phase.clone(), kind))
            .or_default()
            .push(*e);
        if phase.contains("commit") && e.dur_ms() >= commit_threshold_ms {
            long_commits.push(finding_from_events(
                component.clone(),
                phase.clone(),
                &[*e],
                info.and_then(|r| (!r.inferred).then_some("react_performance_track"))
                    .unwrap_or("inferred_call_stack"),
            ));
        }
        if phase.contains("cascading") || e.name.to_lowercase().contains("cascading") {
            cascading_updates.push(finding_from_events(
                component.clone(),
                phase.clone(),
                &[*e],
                "react_performance_track",
            ));
        }
    }
    let mut component_hotspots: Vec<_> = grouped
        .into_iter()
        .map(|((component, phase, kind), events)| {
            finding_from_events(component, phase, &events, &kind)
        })
        .collect();
    if component_hotspots.is_empty() {
        component_hotspots = cpu
            .functions
            .iter()
            .filter(|f| {
                f.source_type == "react/runtime"
                    || f.function.contains("useSyncExternalStore")
                    || f.function.contains("commitRoot")
                    || f.function.contains("renderRoot")
            })
            .take(20)
            .map(|f| ReactFinding {
                component: None,
                phase: "inferred_cpu_profile".to_string(),
                total_ms: f.total_ms,
                count: f.count,
                p95_ms: f.total_ms,
                source: Some(SourceFrame {
                    url: f.url.clone(),
                    line: None,
                    column: None,
                    function: Some(f.function.clone()),
                }),
                event_id: None,
                evidence_kind: "inferred_cpu_profile".to_string(),
            })
            .collect();
    }
    component_hotspots.sort_by(|a, b| {
        b.total_ms
            .partial_cmp(&a.total_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    component_hotspots.truncate(50);
    long_commits.sort_by(|a, b| {
        b.total_ms
            .partial_cmp(&a.total_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    ReactAnalysis {
        tracks_present,
        event_count: react_events.len(),
        inferred_event_count,
        long_commits,
        component_hotspots,
        cascading_updates,
    }
}

fn infer_phase(name: &str) -> String {
    let l = name.to_lowercase();
    if l.contains("commit") {
        "commit"
    } else if l.contains("effect") {
        "effects"
    } else if l.contains("render") {
        "render"
    } else if l.contains("scheduler") {
        "scheduler"
    } else {
        "react"
    }
    .to_string()
}

fn finding_from_events(
    component: Option<String>,
    phase: String,
    events: &[&TraceEvent],
    evidence_kind: &str,
) -> ReactFinding {
    let mut durs: Vec<f64> = events.iter().map(|e| e.dur_ms()).collect();
    durs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let total_ms = durs.iter().sum();
    let idx = if durs.is_empty() {
        0
    } else {
        ((durs.len() as f64 - 1.0) * 0.95).round() as usize
    };
    let p95_ms = durs.get(idx).copied().unwrap_or(0.0);
    let source = events.iter().find_map(|e| e.args.source.clone());
    let event_id = events
        .iter()
        .max_by(|a, b| {
            a.dur_us
                .partial_cmp(&b.dur_us)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|e| e.event_id);
    ReactFinding {
        component,
        phase,
        total_ms,
        count: events.len(),
        p95_ms,
        source,
        event_id,
        evidence_kind: evidence_kind.to_string(),
    }
}
