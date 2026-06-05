use crate::trace::{Category, MainThreadSelection, SourceFrame, TraceEvent, TraceStore};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportMetadata {
    pub trace_path: PathBuf,
    pub project_root: PathBuf,
    pub network_sidecar: Option<PathBuf>,
    pub file_size_bytes: u64,
    pub page_url: Option<String>,
    pub start_time: Option<String>,
    pub cpu_throttling: Option<f64>,
    pub network_throttling: Option<String>,
    pub host_dpr: Option<f64>,
    pub hardware_concurrency: Option<u32>,
}
impl ReportMetadata {
    pub fn from_store(
        store: &TraceStore,
        project_root: PathBuf,
        network_sidecar: Option<PathBuf>,
    ) -> Self {
        let m = &store.metadata;
        Self {
            trace_path: m.trace_path.clone(),
            project_root,
            network_sidecar,
            file_size_bytes: m.file_size_bytes,
            page_url: m.page_url.clone(),
            start_time: m.start_time.clone(),
            cpu_throttling: m.cpu_throttling,
            network_throttling: m.network_throttling.clone(),
            host_dpr: m.host_dpr,
            hardware_concurrency: m.hardware_concurrency,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    pub metadata: ReportMetadata,
    pub coverage: AnalysisCoverage,
    pub findings: Vec<Finding>,
    pub long_tasks: Vec<LongTask>,
    pub react: ReactAnalysis,
    pub realtime: RealtimeAnalysis,
    pub scroll: ScrollAnalysis,
    pub layout: LayoutAnalysis,
    pub gc: GcAnalysis,
    pub cpu_profile: CpuProfileAnalysis,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisCoverage {
    pub trace_file_path: PathBuf,
    pub trace_file_size_bytes: u64,
    pub event_count_scanned: usize,
    pub trace_duration_ms: f64,
    pub pids: Vec<u64>,
    pub tids_analyzed: Vec<ThreadCoverage>,
    pub selected_main_thread: MainThreadSelection,
    pub data_loss: bool,
    pub unknown_event_name_count: usize,
    pub top_event_names_by_total_duration: Vec<EventInventoryRow>,
    pub top_event_names_by_count: Vec<EventInventoryRow>,
    pub time_buckets: Vec<TimeBucket>,
    pub anomaly_windows: Vec<AnomalyWindow>,
    pub long_task_count: usize,
    pub react_event_count: usize,
    pub network_sidecar_event_count: usize,
    pub react_tracks_present: bool,
    pub source_maps_present: bool,
    pub trace_appears_truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadCoverage {
    pub pid: u64,
    pub tid: u64,
    pub thread_name: Option<String>,
    pub process_name: Option<String>,
    pub event_count: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventInventoryRow {
    pub name: String,
    pub total_ms: f64,
    pub count: usize,
    pub category: Category,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeBucket {
    pub index: usize,
    pub start_ms: f64,
    pub end_ms: f64,
    pub main_thread_busy_ms: f64,
    pub js_ms: f64,
    pub style_ms: f64,
    pub layout_ms: f64,
    pub paint_ms: f64,
    pub composite_ms: f64,
    pub gc_ms: f64,
    pub react_ms: f64,
    pub network_adjacent_ms: f64,
    pub unknown_ms: f64,
    pub busy_pct: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyWindow {
    pub kind: String,
    pub bucket_index: usize,
    pub start_ms: f64,
    pub end_ms: f64,
    pub value_ms: f64,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LongTask {
    pub task_id: String,
    pub event_id: usize,
    pub ts_ms: f64,
    pub dur_ms: f64,
    pub breakdown: Breakdown,
    pub child_event_breakdown: Vec<EventInventoryRow>,
    pub top_source_functions: Vec<SourceHotspot>,
    pub preceding_network_events: Vec<NetworkEvidence>,
    pub following_rendering_events: Vec<TraceEvidence>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Breakdown {
    pub js_ms: f64,
    pub react_ms: f64,
    pub style_ms: f64,
    pub layout_ms: f64,
    pub paint_composite_ms: f64,
    pub gc_ms: f64,
    pub parse_compile_ms: f64,
    pub unknown_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceHotspot {
    pub function: String,
    pub url: Option<String>,
    pub total_ms: f64,
    pub count: usize,
    pub source_type: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvidence {
    pub evidence_id: String,
    pub trace_event_id: usize,
    pub name: String,
    pub ts_ms: f64,
    pub dur_ms: f64,
    pub pid: u64,
    pub tid: u64,
    pub category: Category,
    pub source: Option<SourceFrame>,
}
impl TraceEvidence {
    pub fn from_event(evidence_id: String, e: &TraceEvent, origin_us: f64) -> Self {
        Self {
            evidence_id,
            trace_event_id: e.event_id,
            name: e.name.clone(),
            ts_ms: e.ts_ms(origin_us),
            dur_ms: e.dur_ms(),
            pid: e.pid,
            tid: e.tid,
            category: e.category,
            source: e.args.source.clone(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEvidence {
    pub evidence_id: String,
    pub sidecar_event_id: usize,
    pub kind: String,
    pub ts_ms: f64,
    pub direction: Option<String>,
    pub payload_bytes: Option<u64>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactAnalysis {
    pub tracks_present: bool,
    pub event_count: usize,
    pub inferred_event_count: usize,
    pub long_commits: Vec<ReactFinding>,
    pub component_hotspots: Vec<ReactFinding>,
    pub cascading_updates: Vec<ReactFinding>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactFinding {
    pub component: Option<String>,
    pub phase: String,
    pub total_ms: f64,
    pub count: usize,
    pub p95_ms: f64,
    pub source: Option<SourceFrame>,
    pub event_id: Option<usize>,
    pub evidence_kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeAnalysis {
    pub sidecar_event_count: usize,
    pub bursts: Vec<NetworkBurst>,
    pub correlations: Vec<RealtimeCorrelation>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkBurst {
    pub burst_id: String,
    pub kind: String,
    pub start_ms: f64,
    pub end_ms: f64,
    pub count: usize,
    pub payload_bytes: u64,
    pub evidence: Vec<NetworkEvidence>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeCorrelation {
    pub burst_id: String,
    pub long_task_event_id: usize,
    pub task_ts_ms: f64,
    pub task_dur_ms: f64,
    pub confidence: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrollAnalysis {
    pub task_count: usize,
    pub avg_ms: f64,
    pub p50_ms: f64,
    pub p75_ms: f64,
    pub p90_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
    pub breakdown: Breakdown,
    pub layout_thrashing_tasks: usize,
    pub intersection_observer_events: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutAnalysis {
    pub expensive_events: Vec<LayoutFinding>,
    pub forced_reflow_candidates: Vec<LayoutFinding>,
    pub total_style_ms: f64,
    pub total_layout_ms: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutFinding {
    pub event_id: usize,
    pub name: String,
    pub ts_ms: f64,
    pub dur_ms: f64,
    pub dirty_objects: Option<u64>,
    pub total_objects: Option<u64>,
    pub nearby_js: Vec<SourceHotspot>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcAnalysis {
    pub events: Vec<GcEvent>,
    pub total_ms: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcEvent {
    pub event_id: usize,
    pub name: String,
    pub ts_ms: f64,
    pub dur_ms: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuProfileAnalysis {
    pub functions: Vec<SourceHotspot>,
    pub total_sample_ms: f64,
    pub app_ms: f64,
    pub third_party_ms: f64,
    pub react_runtime_ms: f64,
    pub native_ms: f64,
    pub unresolved_or_minified_frames: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub title: String,
    pub severity: Severity,
    pub confidence: Confidence,
    pub impact: Impact,
    pub cause_chain: Vec<CauseStep>,
    pub evidence: Vec<TraceEvidence>,
    pub recommendations: Vec<Recommendation>,
    pub verification: Verification,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    High,
    Medium,
    Low,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Impact {
    pub total_ms: f64,
    pub worst_ms: f64,
    pub count: usize,
    pub affected_windows: Vec<[f64; 2]>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CauseStep {
    pub step: String,
    pub summary: String,
    pub evidence_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub title: String,
    pub why: String,
    pub files_to_inspect: Vec<String>,
    pub suggested_change: String,
    pub risk: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verification {
    pub how_to_measure_after_fix: String,
    pub expected_metric_change: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareReport {
    pub before: Box<AuditReport>,
    pub after: Box<AuditReport>,
    pub deltas: CompareDeltas,
    pub regressions: Vec<String>,
    pub improvements: Vec<String>,
    pub changed_root_causes: Vec<String>,
    pub pr_summary: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareDeltas {
    pub long_tasks: isize,
    pub total_busy_ms: f64,
    pub worst_long_task_ms: f64,
    pub react_events: isize,
    pub websocket_bursts: isize,
}

impl From<&crate::trace::SidecarEvent> for NetworkEvidence {
    fn from(e: &crate::trace::SidecarEvent) -> Self {
        Self {
            evidence_id: format!("net_{:06}", e.event_id),
            sidecar_event_id: e.event_id,
            kind: e.kind.clone(),
            ts_ms: e.ts_ms,
            direction: e.direction.clone(),
            payload_bytes: e.payload_bytes,
            url: e.url.clone(),
        }
    }
}
