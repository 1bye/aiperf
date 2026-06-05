use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub type EventId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Js,
    React,
    Style,
    Layout,
    Paint,
    Composite,
    Raster,
    Gpu,
    Network,
    Gc,
    ParseCompile,
    Timers,
    AnimationFrame,
    Input,
    Scroll,
    HitTest,
    Idle,
    Unknown,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Js => "js",
            Self::React => "react",
            Self::Style => "style",
            Self::Layout => "layout",
            Self::Paint => "paint",
            Self::Composite => "composite",
            Self::Raster => "raster",
            Self::Gpu => "gpu",
            Self::Network => "network",
            Self::Gc => "gc",
            Self::ParseCompile => "parse_compile",
            Self::Timers => "timers",
            Self::AnimationFrame => "animation_frame",
            Self::Input => "input",
            Self::Scroll => "scroll",
            Self::HitTest => "hit_test",
            Self::Idle => "idle",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TraceMetadata {
    pub trace_path: PathBuf,
    pub file_size_bytes: u64,
    pub page_url: Option<String>,
    pub cpu_throttling: Option<f64>,
    pub source: Option<String>,
    pub start_time: Option<String>,
    pub network_throttling: Option<String>,
    pub hardware_concurrency: Option<u32>,
    pub host_dpr: Option<f64>,
    pub data_loss: bool,
    pub raw_metadata_present: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SourceFrame {
    pub url: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub function: Option<String>,
}

impl SourceFrame {
    pub fn is_known(&self) -> bool {
        self.url.as_ref().is_some_and(|u| !u.is_empty())
            || self.function.as_ref().is_some_and(|f| !f.is_empty())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReactInfo {
    pub phase: Option<String>,
    pub component: Option<String>,
    pub track: Option<String>,
    pub inferred: bool,
    pub changed_props: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventArgs {
    pub element_count: Option<u64>,
    pub dirty_objects: Option<u64>,
    pub total_objects: Option<u64>,
    pub source: Option<SourceFrame>,
    pub stack: Vec<SourceFrame>,
    pub react: Option<ReactInfo>,
    pub url: Option<String>,
    pub frame: Option<String>,
    pub payload_bytes: Option<u64>,
    pub extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    pub event_id: EventId,
    pub name: String,
    pub category_raw: Option<String>,
    pub category: Category,
    pub phase: String,
    pub ts_us: f64,
    pub dur_us: f64,
    pub end_us: f64,
    pub pid: u64,
    pub tid: u64,
    pub parent_id: Option<EventId>,
    pub args: EventArgs,
}

impl TraceEvent {
    pub fn is_complete(&self) -> bool {
        self.phase == "X" && self.dur_us > 0.0
    }
    pub fn ts_ms(&self, origin_us: f64) -> f64 {
        (self.ts_us - origin_us) / 1000.0
    }
    pub fn dur_ms(&self) -> f64 {
        self.dur_us / 1000.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadInfo {
    pub pid: u64,
    pub tid: u64,
    pub thread_name: Option<String>,
    pub process_name: Option<String>,
    pub event_count: usize,
    pub run_task_count: usize,
    pub run_task_duration_us: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MainThreadSelection {
    pub pid: u64,
    pub tid: u64,
    pub confidence: String,
    pub explanation: String,
}
