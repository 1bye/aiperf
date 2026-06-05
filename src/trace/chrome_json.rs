use super::event_store::TraceStore;
use super::model::*;
use anyhow::Context;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct RawTraceFile {
    #[serde(rename = "traceEvents", default)]
    trace_events: Vec<RawTraceEvent>,
    #[serde(default)]
    metadata: Option<RawMetadata>,
}

#[derive(Debug, Deserialize, Default)]
struct RawMetadata {
    #[serde(rename = "cpuThrottling", default)]
    cpu_throttling: Option<f64>,
    #[serde(default)]
    source: Option<String>,
    #[serde(rename = "startTime", default)]
    start_time: Option<String>,
    #[serde(rename = "networkThrottling", default)]
    network_throttling: Option<String>,
    #[serde(rename = "hardwareConcurrency", default)]
    hardware_concurrency: Option<u32>,
    #[serde(rename = "hostDPR", default)]
    host_dpr: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct RawTraceEvent {
    #[serde(default)]
    name: String,
    #[serde(default)]
    cat: Option<String>,
    #[serde(default)]
    ph: String,
    #[serde(default)]
    ts: f64,
    #[serde(default)]
    dur: Option<f64>,
    #[serde(default)]
    pid: u64,
    #[serde(default)]
    tid: u64,
    #[serde(default)]
    args: Option<Value>,
}

pub fn parse_trace_file(path: &Path) -> anyhow::Result<TraceStore> {
    let file = File::open(path).with_context(|| format!("open trace {}", path.display()))?;
    let size = file.metadata().map(|m| m.len()).unwrap_or(0);
    let reader = BufReader::new(file);
    let mut de = serde_json::Deserializer::from_reader(reader);
    let raw: RawTraceFile = serde_path_to_error::deserialize(&mut de)
        .with_context(|| format!("parse Chrome trace JSON {}", path.display()))?;

    let mut metadata = TraceMetadata {
        trace_path: path.to_path_buf(),
        file_size_bytes: size,
        raw_metadata_present: raw.metadata.is_some(),
        ..TraceMetadata::default()
    };
    if let Some(m) = raw.metadata {
        metadata.cpu_throttling = m.cpu_throttling;
        metadata.source = m.source;
        metadata.start_time = m.start_time;
        metadata.network_throttling = m.network_throttling;
        metadata.hardware_concurrency = m.hardware_concurrency;
        metadata.host_dpr = m.host_dpr;
    }

    let mut process_names = BTreeMap::new();
    let mut thread_names = BTreeMap::new();
    let mut events = Vec::with_capacity(raw.trace_events.len());

    for (i, e) in raw.trace_events.into_iter().enumerate() {
        if e.name == "TracingStartedInBrowser" && metadata.page_url.is_none() {
            metadata.page_url = extract_page_url(e.args.as_ref());
        }
        if (e.name == "TracingComplete" || e.name == "Tracing.tracingComplete")
            && e.args
                .as_ref()
                .and_then(|a| a.get("dataLossOccurred"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
        {
            metadata.data_loss = true;
        }
        if e.ph == "M" {
            if e.name == "process_name"
                && let Some(name) = e
                    .args
                    .as_ref()
                    .and_then(|a| a.get("name"))
                    .and_then(Value::as_str)
            {
                process_names.insert(e.pid, name.to_string());
            }
            if e.name == "thread_name"
                && let Some(name) = e
                    .args
                    .as_ref()
                    .and_then(|a| a.get("name"))
                    .and_then(Value::as_str)
            {
                thread_names.insert((e.pid, e.tid), name.to_string());
            }
        }
        let dur = e.dur.unwrap_or(0.0);
        let args = select_args(e.args.as_ref(), &e.name, e.cat.as_deref());
        let category = classify_event(&e.name, e.cat.as_deref(), &args);
        events.push(TraceEvent {
            event_id: i,
            name: e.name,
            category_raw: e.cat,
            category,
            phase: e.ph,
            ts_us: e.ts,
            dur_us: dur,
            end_us: e.ts + dur,
            pid: e.pid,
            tid: e.tid,
            parent_id: None,
            args,
        });
    }

    Ok(TraceStore::new(
        metadata,
        events,
        process_names,
        thread_names,
    ))
}

fn extract_page_url(args: Option<&Value>) -> Option<String> {
    let frames = args?.get("data")?.get("frames")?.as_array()?;
    frames
        .iter()
        .filter_map(|f| f.get("url").and_then(Value::as_str))
        .find(|u| !u.is_empty() && *u != "about:blank")
        .map(str::to_string)
}

fn select_args(args: Option<&Value>, name: &str, cat: Option<&str>) -> EventArgs {
    let mut out = EventArgs::default();
    let Some(args) = args else {
        return out;
    };
    out.element_count = lookup_u64(args, &["elementCount", "data.elementCount"]);
    out.dirty_objects = lookup_u64(args, &["beginData.dirtyObjects", "data.dirtyObjects"]);
    out.total_objects = lookup_u64(args, &["beginData.totalObjects", "data.totalObjects"]);
    out.url = lookup_str(
        args,
        &["url", "data.url", "data.request.url", "beginData.url"],
    );
    out.frame = lookup_str(args, &["frame", "data.frame", "data.frameTreeNodeId"]);
    out.payload_bytes = lookup_u64(
        args,
        &[
            "payloadDataLength",
            "data.payloadDataLength",
            "encodedDataLength",
            "data.encodedDataLength",
        ],
    );
    let stack = extract_stack(args);
    out.stack = stack;
    out.source = extract_source(args).or_else(|| out.stack.first().cloned());
    out.react = extract_react(args, name, cat, out.source.as_ref(), &out.stack);
    for key in ["elementCount", "beginData", "data", "frame", "url"] {
        if let Some(v) = args.get(key) {
            out.extra.insert(key.to_string(), v.clone());
        }
    }
    out
}

fn lookup<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = root;
    for part in path.split('.') {
        cur = cur.get(part)?;
    }
    Some(cur)
}
fn lookup_str(root: &Value, paths: &[&str]) -> Option<String> {
    paths
        .iter()
        .filter_map(|p| lookup(root, p).and_then(Value::as_str))
        .find(|s| !s.is_empty())
        .map(str::to_string)
}
fn lookup_u64(root: &Value, paths: &[&str]) -> Option<u64> {
    paths.iter().find_map(|p| {
        lookup(root, p).and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|n| n as u64)))
    })
}

fn extract_source(args: &Value) -> Option<SourceFrame> {
    let candidates = [
        "data.callFrame",
        "callFrame",
        "data",
        "beginData",
        "stackTrace.0",
        "data.stackTrace.0",
    ];
    for c in candidates {
        if let Some(v) = lookup(args, c) {
            let src = SourceFrame {
                url: lookup_str(v, &["url", "scriptName", "sourceURL"]),
                line: lookup_u64(v, &["lineNumber", "line"]).map(|n| n as u32),
                column: lookup_u64(v, &["columnNumber", "column"]).map(|n| n as u32),
                function: lookup_str(v, &["functionName", "name"]),
            };
            if src.is_known() {
                return Some(src);
            }
        }
    }
    None
}

fn extract_stack(args: &Value) -> Vec<SourceFrame> {
    let mut frames = Vec::new();
    for key in [
        "stackTrace",
        "data.stackTrace",
        "data.callFrames",
        "callFrames",
    ] {
        if let Some(arr) = lookup(args, key).and_then(Value::as_array) {
            for frame in arr.iter().take(64) {
                let src = SourceFrame {
                    url: lookup_str(frame, &["url", "scriptName", "sourceURL"]),
                    line: lookup_u64(frame, &["lineNumber", "line"]).map(|n| n as u32),
                    column: lookup_u64(frame, &["columnNumber", "column"]).map(|n| n as u32),
                    function: lookup_str(frame, &["functionName", "name"]),
                };
                if src.is_known() {
                    frames.push(src);
                }
            }
        }
    }
    frames
}

fn extract_react(
    args: &Value,
    name: &str,
    cat: Option<&str>,
    source: Option<&SourceFrame>,
    stack: &[SourceFrame],
) -> Option<ReactInfo> {
    let lower = format!(
        "{} {}",
        name.to_lowercase(),
        cat.unwrap_or("").to_lowercase()
    );
    let track = lookup_str(
        args,
        &["track", "data.track", "data.trackName", "data.lane"],
    );
    let component = lookup_str(
        args,
        &[
            "componentName",
            "data.componentName",
            "data.displayName",
            "data.component",
            "data.name",
        ],
    );
    let phase =
        lookup_str(args, &["phase", "data.phase", "data.type"]).or_else(|| infer_react_phase(name));
    let changed_props = lookup(args, "data.changedProps")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let stack_says_react = source.into_iter().chain(stack.iter()).any(|f| {
        f.url.as_deref().is_some_and(is_react_like_url)
            || f.function.as_deref().is_some_and(is_react_like_name)
    });
    if lower.contains("react")
        || track
            .as_deref()
            .is_some_and(|t| t.to_lowercase().contains("react"))
        || component.is_some()
        || phase.is_some() && lower.contains("scheduler")
        || stack_says_react
    {
        let inferred = !(lower.contains("react") || component.is_some());
        Some(ReactInfo {
            phase,
            component,
            track,
            inferred,
            changed_props,
        })
    } else {
        None
    }
}

fn infer_react_phase(name: &str) -> Option<String> {
    let l = name.to_lowercase();
    for phase in [
        "commit",
        "render",
        "layout effect",
        "passive effect",
        "remaining effects",
        "cascading update",
        "scheduler",
        "update",
    ] {
        if l.contains(phase) {
            return Some(phase.replace(' ', "_"));
        }
    }
    None
}

fn is_react_like_url(url: &str) -> bool {
    let l = url.to_lowercase();
    l.contains("react-dom")
        || l.contains("scheduler")
        || l.contains("zustand")
        || l.contains("redux")
        || l.contains("jotai")
        || l.contains("tanstack")
        || l.contains("usesyncexternalstore")
}
fn is_react_like_name(name: &str) -> bool {
    name.contains("useSyncExternalStore")
        || name.contains("performConcurrentWork")
        || name.contains("commitRoot")
        || name.contains("renderRoot")
}

pub fn classify_event(name: &str, cat: Option<&str>, args: &EventArgs) -> Category {
    let n = name.to_lowercase();
    let c = cat.unwrap_or("").to_lowercase();
    if args.react.is_some() || n.contains("react") {
        return Category::React;
    }
    match name {
        "FunctionCall"
        | "EvaluateScript"
        | "v8.execute"
        | "EventDispatch"
        | "RunTask"
        | "ThreadControllerImpl::RunTask" => Category::Js,
        "TimerFire" | "TimerInstall" | "TimerRemove" => Category::Timers,
        "FireAnimationFrame" | "RequestAnimationFrame" => Category::AnimationFrame,
        "UpdateLayoutTree" | "RecalculateStyles" => Category::Style,
        "Layout" | "InvalidateLayout" => Category::Layout,
        "Paint" | "PrePaint" => Category::Paint,
        "CompositeLayers" | "Layerize" | "Commit" | "DrawFrame" => Category::Composite,
        "HitTest" | "IntersectionObserverController::computeIntersections" => Category::HitTest,
        "MajorGC" | "MinorGC" => Category::Gc,
        "ParseHTML" | "ParseScript" | "CompileScript" => Category::ParseCompile,
        _ => {
            if n.contains("gc") || n.starts_with("v8.gc") {
                Category::Gc
            } else if n.contains("websocket")
                || n.contains("request")
                || c.contains("netlog")
                || c.contains("loading")
            {
                Category::Network
            } else if n.contains("scroll") {
                Category::Scroll
            } else if n.contains("input") || n.contains("mouse") || n.contains("key") {
                Category::Input
            } else if n.contains("paint") {
                Category::Paint
            } else if n.contains("layout") {
                Category::Layout
            } else if n.contains("style") {
                Category::Style
            } else if n.contains("raster") {
                Category::Raster
            } else if n.contains("gpu") {
                Category::Gpu
            } else if n.contains("parse") || n.contains("compile") {
                Category::ParseCompile
            } else if n.contains("idle") {
                Category::Idle
            } else {
                Category::Unknown
            }
        }
    }
}
