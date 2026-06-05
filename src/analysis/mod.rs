pub mod categories;
pub mod causality;
pub mod compare;
pub mod cpu_profile;
pub mod gc;
pub mod inventory;
pub mod layout;
pub mod long_tasks;
pub mod main_thread;
pub mod network;
pub mod react;
pub mod realtime;
pub mod recommendations;
pub mod scroll;

use crate::config::load_config;
use crate::report::schema::*;
use crate::trace::{NetworkSidecar, network_sidecar::parse_sidecar, parse_trace_file};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AuditOptions {
    pub project_root: PathBuf,
    pub network_sidecar: Option<PathBuf>,
    pub long_task_ms: Option<f64>,
    pub bucket_ms: Option<f64>,
    pub top_n: Option<usize>,
}

impl Default for AuditOptions {
    fn default() -> Self {
        Self {
            project_root: PathBuf::from("."),
            network_sidecar: None,
            long_task_ms: None,
            bucket_ms: None,
            top_n: None,
        }
    }
}

pub type AuditReport = crate::report::schema::AuditReport;
pub type CompareReport = crate::report::schema::CompareReport;

pub fn audit_trace(path: &Path, options: &AuditOptions) -> anyhow::Result<AuditReport> {
    let mut cfg = load_config(&options.project_root)?;
    if let Some(v) = options.long_task_ms {
        cfg.trace.long_task_ms = v;
    }
    if let Some(v) = options.bucket_ms {
        cfg.trace.bucket_ms = v;
    }
    if let Some(v) = options.top_n {
        cfg.report.top_n = v;
    }
    let store = parse_trace_file(path)?;
    let sidecar = match &options.network_sidecar {
        Some(p) => parse_sidecar(p, cfg.trace.redact_urls)?,
        None => NetworkSidecar::empty(),
    };
    let main = main_thread::detect_main_thread(&store);
    let cpu = cpu_profile::analyze_cpu_profile(&store);
    let react = react::analyze_react(&store, &cpu, cfg.analysis.react_commit_ms);
    let realtime = realtime::analyze_realtime(
        &store,
        &sidecar,
        cfg.analysis.realtime_burst_window_ms,
        cfg.analysis.network_correlation_window_ms,
    );
    let long_tasks = long_tasks::analyze_long_tasks(
        &store,
        &main,
        cfg.trace.long_task_ms,
        &sidecar,
        cfg.analysis.network_correlation_window_ms,
    );
    let scroll = scroll::analyze_scroll(&store, &main);
    let layout = layout::analyze_layout(&store, &main, cfg.analysis.layout_style_ms);
    let gc = gc::analyze_gc(&store, &main, cfg.analysis.gc_ms);
    let coverage = inventory::analysis_coverage(
        &store,
        &main,
        &sidecar,
        cfg.trace.bucket_ms,
        &react,
        long_tasks.len(),
    );
    let findings = causality::build_findings(&store, &long_tasks, &react, &realtime, &layout, &gc);
    Ok(AuditReport {
        metadata: ReportMetadata::from_store(
            &store,
            options.project_root.clone(),
            options.network_sidecar.clone(),
        ),
        coverage,
        findings,
        long_tasks,
        react,
        realtime,
        scroll,
        layout,
        gc,
        cpu_profile: cpu,
        warnings: warnings(&store, &sidecar, options.project_root.as_path()),
    })
}

fn warnings(
    store: &crate::trace::TraceStore,
    sidecar: &NetworkSidecar,
    project_root: &Path,
) -> Vec<String> {
    let mut w = Vec::new();
    if store.metadata.data_loss {
        w.push("Trace reports data loss; missing events may hide or distort findings.".to_string());
    }
    if sidecar.events.is_empty() {
        w.push(
            "Network sidecar absent; realtime/WebSocket correlation limited to trace events."
                .to_string(),
        );
    }
    if !store
        .events
        .iter()
        .any(|e| e.args.react.as_ref().is_some_and(|r| !r.inferred))
    {
        w.push("React Performance Tracks absent; React findings use lower-confidence inference when stack data exists.".to_string());
    }
    if !crate::trace::source_maps::source_maps_available(project_root) {
        w.push("Source maps absent or not found; code-level confidence is limited for bundled/minified frames.".to_string());
    }
    if store.duration_ms() <= 0.0 {
        w.push("Trace has no positive duration; analysis coverage is limited.".to_string());
    }
    w
}

pub fn compare_traces(
    before: &Path,
    after: &Path,
    options: &AuditOptions,
) -> anyhow::Result<CompareReport> {
    let before_report = audit_trace(before, options)?;
    let after_report = audit_trace(after, options)?;
    Ok(compare::compare_reports(before_report, after_report))
}
