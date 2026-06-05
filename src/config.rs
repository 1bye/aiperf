use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub trace: TraceConfig,
    #[serde(default)]
    pub project: ProjectConfig,
    #[serde(default)]
    pub analysis: AnalysisConfig,
    #[serde(default)]
    pub report: ReportConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceConfig {
    #[serde(default = "default_preset")]
    pub preset: String,
    #[serde(default = "default_long_task_ms")]
    pub long_task_ms: f64,
    #[serde(default = "default_bucket_ms")]
    pub bucket_ms: f64,
    #[serde(default = "default_true")]
    pub record_network_sidecar: bool,
    #[serde(default = "default_true")]
    pub redact_urls: bool,
    #[serde(default)]
    pub include_payloads: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    #[serde(default = "default_root")]
    pub root: PathBuf,
    #[serde(default = "default_source_map_globs")]
    pub source_map_globs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    #[serde(default = "default_true")]
    pub react: bool,
    #[serde(default = "default_true")]
    pub scroll: bool,
    #[serde(default = "default_true")]
    pub realtime: bool,
    #[serde(default = "default_true")]
    pub layout: bool,
    #[serde(default = "default_true")]
    pub gc: bool,
    #[serde(default = "default_true")]
    pub cpu_profile: bool,
    #[serde(default = "default_frame_budget_ms")]
    pub frame_budget_ms: f64,
    #[serde(default = "default_realtime_window_ms")]
    pub realtime_burst_window_ms: f64,
    #[serde(default = "default_network_correlation_ms")]
    pub network_correlation_window_ms: f64,
    #[serde(default = "default_react_commit_ms")]
    pub react_commit_ms: f64,
    #[serde(default = "default_layout_ms")]
    pub layout_style_ms: f64,
    #[serde(default = "default_gc_ms")]
    pub gc_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportConfig {
    #[serde(default = "default_top_n")]
    pub top_n: usize,
    #[serde(default = "default_true")]
    pub full_json: bool,
}

impl Default for TraceConfig {
    fn default() -> Self {
        Self {
            preset: default_preset(),
            long_task_ms: default_long_task_ms(),
            bucket_ms: default_bucket_ms(),
            record_network_sidecar: true,
            redact_urls: true,
            include_payloads: false,
        }
    }
}
impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            root: default_root(),
            source_map_globs: default_source_map_globs(),
        }
    }
}
impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            react: true,
            scroll: true,
            realtime: true,
            layout: true,
            gc: true,
            cpu_profile: true,
            frame_budget_ms: default_frame_budget_ms(),
            realtime_burst_window_ms: default_realtime_window_ms(),
            network_correlation_window_ms: default_network_correlation_ms(),
            react_commit_ms: default_react_commit_ms(),
            layout_style_ms: default_layout_ms(),
            gc_ms: default_gc_ms(),
        }
    }
}
impl Default for ReportConfig {
    fn default() -> Self {
        Self {
            top_n: default_top_n(),
            full_json: true,
        }
    }
}

pub fn load_config(project_root: &Path) -> anyhow::Result<AppConfig> {
    let path = project_root.join(".aiperf.toml");
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let bytes = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", path.display()))?;
    toml::from_str(&bytes).map_err(|e| anyhow::anyhow!("parse {}: {e}", path.display()))
}

fn default_preset() -> String {
    "runtime-react".to_string()
}
fn default_root() -> PathBuf {
    PathBuf::from(".")
}
fn default_source_map_globs() -> Vec<String> {
    vec![
        "dist/**/*.map".into(),
        ".next/**/*.map".into(),
        "build/**/*.map".into(),
    ]
}
fn default_long_task_ms() -> f64 {
    50.0
}
fn default_bucket_ms() -> f64 {
    1000.0
}
fn default_frame_budget_ms() -> f64 {
    16.67
}
fn default_realtime_window_ms() -> f64 {
    250.0
}
fn default_network_correlation_ms() -> f64 {
    250.0
}
fn default_react_commit_ms() -> f64 {
    16.0
}
fn default_layout_ms() -> f64 {
    16.0
}
fn default_gc_ms() -> f64 {
    16.0
}
fn default_top_n() -> usize {
    20
}
fn default_true() -> bool {
    true
}
