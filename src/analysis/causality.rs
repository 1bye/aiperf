use crate::analysis::recommendations::{
    layout_recommendation, long_task_recommendation, realtime_recommendation,
};
use crate::report::schema::*;
use crate::trace::{Category, TraceStore};

pub fn build_findings(
    store: &TraceStore,
    long_tasks: &[LongTask],
    react: &ReactAnalysis,
    realtime: &RealtimeAnalysis,
    layout: &LayoutAnalysis,
    gc: &GcAnalysis,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    if !long_tasks.is_empty() {
        let worst = &long_tasks[0];
        let evidence = store
            .event(worst.event_id)
            .map(|e| {
                vec![TraceEvidence::from_event(
                    "ev_000001".to_string(),
                    e,
                    store.origin_ts_us,
                )]
            })
            .unwrap_or_default();
        let files = files_from_sources(&worst.top_source_functions);
        let mut chain = Vec::new();
        if !worst.preceding_network_events.is_empty() {
            chain.push(CauseStep {
                step: "network_or_message_activity".to_string(),
                summary: format!(
                    "{} network/WS events occur before worst task",
                    worst.preceding_network_events.len()
                ),
                evidence_ids: worst
                    .preceding_network_events
                    .iter()
                    .map(|e| e.evidence_id.clone())
                    .collect(),
            });
        }
        chain.push(CauseStep {
            step: "main_thread_long_task".to_string(),
            summary: format!(
                "{}ms task blocks main thread with JS {:.2}ms React {:.2}ms style/layout {:.2}ms",
                round(worst.dur_ms),
                worst.breakdown.js_ms,
                worst.breakdown.react_ms,
                worst.breakdown.style_ms + worst.breakdown.layout_ms
            ),
            evidence_ids: evidence.iter().map(|e| e.evidence_id.clone()).collect(),
        });
        if worst.breakdown.paint_composite_ms > 0.0 {
            chain.push(CauseStep {
                step: "layout_paint".to_string(),
                summary: format!(
                    "Rendering work follows or occurs inside task: {:.2}ms paint/composite",
                    worst.breakdown.paint_composite_ms
                ),
                evidence_ids: worst
                    .following_rendering_events
                    .iter()
                    .map(|e| e.evidence_id.clone())
                    .collect(),
            });
        }
        findings.push(Finding {
            id: next_id(findings.len()),
            title: "Main-thread long tasks dominate runtime trace".to_string(),
            severity: severity_for(worst.dur_ms),
            confidence: if evidence.is_empty() {
                Confidence::Low
            } else {
                Confidence::Medium
            },
            impact: Impact {
                total_ms: long_tasks.iter().map(|t| t.dur_ms).sum(),
                worst_ms: worst.dur_ms,
                count: long_tasks.len(),
                affected_windows: long_tasks
                    .iter()
                    .map(|t| [t.ts_ms, t.ts_ms + t.dur_ms])
                    .collect(),
            },
            cause_chain: chain,
            evidence,
            recommendations: vec![long_task_recommendation(files)],
            verification: default_verification("long tasks over threshold"),
        });
    }
    for corr in realtime.correlations.iter().take(3) {
        if let Some(task) = store.event(corr.long_task_event_id) {
            let evidence = vec![TraceEvidence::from_event(
                format!("ev_{:06}", task.event_id),
                task,
                store.origin_ts_us,
            )];
            findings.push(Finding {
                id: next_id(findings.len()),
                title: "Realtime burst aligns with blocking main-thread work".to_string(),
                severity: severity_for(corr.task_dur_ms),
                confidence: Confidence::Medium,
                impact: Impact {
                    total_ms: corr.task_dur_ms,
                    worst_ms: corr.task_dur_ms,
                    count: 1,
                    affected_windows: vec![[corr.task_ts_ms, corr.task_ts_ms + corr.task_dur_ms]],
                },
                cause_chain: vec![
                    CauseStep {
                        step: "websocket_burst".to_string(),
                        summary: corr.summary.clone(),
                        evidence_ids: vec![corr.burst_id.clone()],
                    },
                    CauseStep {
                        step: "state_update_or_js_work".to_string(),
                        summary: format!("Following task lasts {:.2}ms", corr.task_dur_ms),
                        evidence_ids: evidence.iter().map(|e| e.evidence_id.clone()).collect(),
                    },
                ],
                evidence,
                recommendations: vec![realtime_recommendation(Vec::new())],
                verification: default_verification("WebSocket bursts and correlated long tasks"),
            });
        }
    }
    if let Some(r) = react
        .long_commits
        .first()
        .or_else(|| react.component_hotspots.first())
        .filter(|r| r.total_ms > 0.0)
    {
        let evidence = r
            .event_id
            .and_then(|id| store.event(id))
            .map(|e| {
                vec![TraceEvidence::from_event(
                    format!("ev_{:06}", e.event_id),
                    e,
                    store.origin_ts_us,
                )]
            })
            .unwrap_or_default();
        findings.push(Finding {
            id: next_id(findings.len()),
            title: "React work is measurable in runtime trace".to_string(),
            severity: if r.total_ms > 50.0 {
                Severity::Medium
            } else {
                Severity::Low
            },
            confidence: if r.evidence_kind == "react_performance_track" {
                Confidence::High
            } else {
                Confidence::Low
            },
            impact: Impact {
                total_ms: r.total_ms,
                worst_ms: r.p95_ms,
                count: r.count,
                affected_windows: evidence
                    .iter()
                    .map(|e| [e.ts_ms, e.ts_ms + e.dur_ms])
                    .collect(),
            },
            cause_chain: vec![CauseStep {
                step: "react_render_or_commit".to_string(),
                summary: format!(
                    "React phase {} component {:?} totals {:.2}ms",
                    r.phase, r.component, r.total_ms
                ),
                evidence_ids: evidence.iter().map(|e| e.evidence_id.clone()).collect(),
            }],
            evidence,
            recommendations: vec![long_task_recommendation(
                r.source
                    .as_ref()
                    .and_then(|s| s.url.clone())
                    .into_iter()
                    .collect(),
            )],
            verification: default_verification("React commit/render duration"),
        });
    }
    if let Some(l) = layout.expensive_events.first()
        && let Some(ev) = store.event(l.event_id)
    {
        let evidence = vec![TraceEvidence::from_event(
            format!("ev_{:06}", ev.event_id),
            ev,
            store.origin_ts_us,
        )];
        findings.push(Finding {
            id: next_id(findings.len()),
            title: "Expensive style/layout work found".to_string(),
            severity: if l.dur_ms > 50.0 {
                Severity::High
            } else {
                Severity::Medium
            },
            confidence: Confidence::Medium,
            impact: Impact {
                total_ms: layout.total_style_ms + layout.total_layout_ms,
                worst_ms: l.dur_ms,
                count: layout.expensive_events.len(),
                affected_windows: vec![[l.ts_ms, l.ts_ms + l.dur_ms]],
            },
            cause_chain: vec![CauseStep {
                step: "layout_paint".to_string(),
                summary: format!(
                    "{} takes {:.2}ms dirty {:?}/{:?}",
                    l.name, l.dur_ms, l.dirty_objects, l.total_objects
                ),
                evidence_ids: evidence.iter().map(|e| e.evidence_id.clone()).collect(),
            }],
            evidence,
            recommendations: vec![layout_recommendation(files_from_sources(&l.nearby_js))],
            verification: default_verification("style/layout duration and dirty object counts"),
        });
    }
    if let Some(g) = gc.events.first().filter(|g| g.dur_ms > 16.0)
        && let Some(ev) = store.event(g.event_id)
    {
        let evidence = vec![TraceEvidence::from_event(
            format!("ev_{:06}", ev.event_id),
            ev,
            store.origin_ts_us,
        )];
        findings.push(Finding {
            id: next_id(findings.len()),
            title: "Garbage collection contributes to frame risk".to_string(),
            severity: Severity::Medium,
            confidence: Confidence::Medium,
            impact: Impact {
                total_ms: gc.total_ms,
                worst_ms: g.dur_ms,
                count: gc.events.len(),
                affected_windows: vec![[g.ts_ms, g.ts_ms + g.dur_ms]],
            },
            cause_chain: vec![CauseStep {
                step: "gc_pause".to_string(),
                summary: format!("{} lasts {:.2}ms", g.name, g.dur_ms),
                evidence_ids: evidence.iter().map(|e| e.evidence_id.clone()).collect(),
            }],
            evidence,
            recommendations: vec![long_task_recommendation(Vec::new())],
            verification: default_verification("GC pause duration"),
        });
    }
    if findings.is_empty() {
        findings.push(Finding { id: next_id(0), title: "No dominant supported performance root cause found".to_string(), severity: Severity::Info, confidence: Confidence::Low, impact: Impact { total_ms: 0.0, worst_ms: 0.0, count: 0, affected_windows: Vec::new() }, cause_chain: vec![CauseStep { step: "analysis_coverage".to_string(), summary: "Trace scanned, but deterministic analyzers did not find threshold-crossing long task, React, realtime, layout, or GC evidence.".to_string(), evidence_ids: Vec::new() }], evidence: Vec::new(), recommendations: Vec::new(), verification: default_verification("same audit command") });
    }
    findings
}

fn next_id(n: usize) -> String {
    format!("finding_{:03}", n + 1)
}
fn round(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
fn severity_for(ms: f64) -> Severity {
    if ms >= 500.0 {
        Severity::Critical
    } else if ms >= 200.0 {
        Severity::High
    } else if ms >= 50.0 {
        Severity::Medium
    } else {
        Severity::Low
    }
}
fn files_from_sources(srcs: &[SourceHotspot]) -> Vec<String> {
    srcs.iter()
        .filter(|s| s.source_type == "app")
        .filter_map(|s| s.url.clone())
        .take(10)
        .collect()
}
fn default_verification(metric: &str) -> Verification {
    Verification { how_to_measure_after_fix: "Record same user flow, run `aiperf audit` on new trace, then `aiperf compare before after`.".to_string(), expected_metric_change: format!("Reduced {metric} with same or better analysis coverage and no trace data loss.") }
}
#[allow(dead_code)]
fn category_evidence(store: &TraceStore, category: Category) -> Vec<TraceEvidence> {
    store
        .events
        .iter()
        .filter(|e| e.category == category)
        .take(5)
        .map(|e| TraceEvidence::from_event(format!("ev_{:06}", e.event_id), e, store.origin_ts_us))
        .collect()
}
