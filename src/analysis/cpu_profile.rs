use crate::analysis::long_tasks::classify_source;
use crate::report::schema::{CpuProfileAnalysis, SourceHotspot};
use crate::trace::TraceStore;
use serde_json::Value;
use std::collections::HashMap;

pub fn analyze_cpu_profile(store: &TraceStore) -> CpuProfileAnalysis {
    let mut node_map: HashMap<u64, (String, Option<String>)> = HashMap::new();
    let mut self_ms: HashMap<u64, f64> = HashMap::new();
    let mut unresolved = 0usize;
    for e in &store.events {
        if e.name != "ProfileChunk" {
            continue;
        }
        let data = match e.args.extra.get("data") {
            Some(v) => v,
            None => continue,
        };
        let Some(cpu_profile) = data.get("cpuProfile") else {
            continue;
        };
        if let Some(nodes) = cpu_profile.get("nodes").and_then(Value::as_array) {
            for node in nodes {
                let id = node.get("id").and_then(Value::as_u64).unwrap_or(0);
                let cf = node.get("callFrame");
                let function = cf
                    .and_then(|v| v.get("functionName"))
                    .and_then(Value::as_str)
                    .unwrap_or("(anonymous)")
                    .to_string();
                let url = cf
                    .and_then(|v| v.get("url"))
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string);
                if url
                    .as_deref()
                    .is_none_or(|u| u.is_empty() || u.contains(".min."))
                {
                    unresolved += 1;
                }
                node_map.entry(id).or_insert((function, url));
            }
        }
        let samples = cpu_profile.get("samples").and_then(Value::as_array);
        let deltas = data
            .get("timeDeltas")
            .and_then(Value::as_array)
            .or_else(|| cpu_profile.get("timeDeltas").and_then(Value::as_array));
        if let (Some(samples), Some(deltas)) = (samples, deltas) {
            for (sample, delta) in samples.iter().zip(deltas.iter()) {
                let id = sample.as_u64().unwrap_or(0);
                let dt_ms = delta.as_f64().unwrap_or(0.0) / 1000.0;
                *self_ms.entry(id).or_default() += dt_ms;
            }
        }
    }
    let mut functions: Vec<_> = self_ms
        .into_iter()
        .map(|(id, total_ms)| {
            let (function, url) = node_map
                .get(&id)
                .cloned()
                .unwrap_or_else(|| ("(unknown)".to_string(), None));
            let source_type = classify_source(url.as_deref());
            SourceHotspot {
                function,
                url,
                total_ms,
                count: 1,
                source_type,
            }
        })
        .filter(|f| f.total_ms > 0.0)
        .collect();
    functions.sort_by(|a, b| {
        b.total_ms
            .partial_cmp(&a.total_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let total_sample_ms: f64 = functions.iter().map(|f| f.total_ms).sum();
    let app_ms = sum_type(&functions, "app");
    let react_runtime_ms = sum_type(&functions, "react/runtime");
    let third_party_ms = sum_type(&functions, "node_modules");
    let native_ms = sum_type(&functions, "browser/native");
    functions.truncate(100);
    CpuProfileAnalysis {
        functions,
        total_sample_ms,
        app_ms,
        third_party_ms,
        react_runtime_ms,
        native_ms,
        unresolved_or_minified_frames: unresolved,
    }
}
fn sum_type(functions: &[SourceHotspot], ty: &str) -> f64 {
    functions
        .iter()
        .filter(|f| f.source_type == ty)
        .map(|f| f.total_ms)
        .sum()
}
