use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkSidecar {
    pub path: Option<std::path::PathBuf>,
    pub events: Vec<SidecarEvent>,
    pub clock_alignment: Option<ClockAlignment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClockAlignment {
    pub sidecar_wall_time_ms: f64,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarEvent {
    pub event_id: usize,
    pub kind: String,
    pub ts_ms: f64,
    pub request_id: Option<String>,
    pub url: Option<String>,
    pub direction: Option<String>,
    pub payload_bytes: Option<u64>,
    pub opcode: Option<u64>,
    pub status: Option<u16>,
    pub error_text: Option<String>,
}

impl NetworkSidecar {
    pub fn empty() -> Self {
        Self::default()
    }
}

pub fn parse_sidecar(path: &Path, redact_urls: bool) -> anyhow::Result<NetworkSidecar> {
    let f = File::open(path)
        .map_err(|e| anyhow::anyhow!("open network sidecar {}: {e}", path.display()))?;
    let r = BufReader::new(f);
    let mut out = NetworkSidecar {
        path: Some(path.to_path_buf()),
        ..NetworkSidecar::default()
    };
    for (idx, line) in r.lines().enumerate() {
        let line = line.map_err(|e| {
            anyhow::anyhow!(
                "read network sidecar {} line {}: {e}",
                path.display(),
                idx + 1
            )
        })?;
        if line.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(&line).map_err(|e| {
            anyhow::anyhow!(
                "parse network sidecar {} line {}: {e}",
                path.display(),
                idx + 1
            )
        })?;
        if v.get("kind").and_then(Value::as_str) == Some("clock_alignment") {
            out.clock_alignment = Some(ClockAlignment {
                sidecar_wall_time_ms: num(&v, &["wall_time_ms", "sidecar_wall_time_ms"])
                    .unwrap_or(0.0),
                note: v
                    .get("note")
                    .and_then(Value::as_str)
                    .unwrap_or("sidecar clock alignment")
                    .to_string(),
            });
            continue;
        }
        if let Some(ev) = normalize_event(idx, &v, redact_urls) {
            out.events.push(ev);
        }
    }
    Ok(out)
}

fn normalize_event(event_id: usize, v: &Value, redact_urls: bool) -> Option<SidecarEvent> {
    let method = v
        .get("method")
        .and_then(Value::as_str)
        .or_else(|| v.get("kind").and_then(Value::as_str))?
        .to_string();
    let params = v.get("params").unwrap_or(v);
    let ts_ms = num(params, &["ts_ms", "timestamp_ms"])
        .or_else(|| num(params, &["timestamp", "wallTime"]).map(|t| t * 1000.0))
        .unwrap_or(0.0);
    let request_id = strv(params, &["requestId", "request_id"]).map(str::to_string);
    let url = strv(params, &["request.url", "response.url", "url"]).map(|s| {
        if redact_urls {
            redact_url(s)
        } else {
            s.to_string()
        }
    });
    let direction = if method.contains("FrameReceived") {
        Some("received".to_string())
    } else if method.contains("FrameSent") {
        Some("sent".to_string())
    } else {
        strv(params, &["direction"]).map(str::to_string)
    };
    let payload_bytes = num(
        params,
        &[
            "response.encodedDataLength",
            "encodedDataLength",
            "dataLength",
            "payload_bytes",
            "response.headers.Content-Length",
            "response.headers.content-length",
            "response.payload_bytes",
        ],
    )
    .map(|n| n.max(0.0) as u64)
    .or_else(|| {
        strv(
            params,
            &[
                "response.payloadData",
                "response.payloadDataLength",
                "response.payload_data",
                "response.payloadData",
            ],
        )
        .map(|s| s.len() as u64)
    });
    let opcode = num(params, &["response.opcode", "opcode"]).map(|n| n as u64);
    let status = num(params, &["response.status", "status"]).map(|n| n as u16);
    let error_text = strv(params, &["errorText", "error_text"]).map(str::to_string);
    Some(SidecarEvent {
        event_id,
        kind: method,
        ts_ms,
        request_id,
        url,
        direction,
        payload_bytes,
        opcode,
        status,
        error_text,
    })
}

pub fn redact_url(input: &str) -> String {
    if let Ok(url) = Url::parse(input) {
        let origin = match url.port() {
            Some(p) => format!("{}://{}:{}", url.scheme(), url.host_str().unwrap_or(""), p),
            None => format!("{}://{}", url.scheme(), url.host_str().unwrap_or("")),
        };
        format!("{}{}", origin, url.path())
    } else {
        input
            .split('?')
            .next()
            .unwrap_or(input)
            .split('#')
            .next()
            .unwrap_or(input)
            .to_string()
    }
}

fn strv<'a>(root: &'a Value, paths: &[&str]) -> Option<&'a str> {
    paths
        .iter()
        .find_map(|p| lookup(root, p).and_then(Value::as_str))
        .filter(|s| !s.is_empty())
}
fn num(root: &Value, paths: &[&str]) -> Option<f64> {
    paths.iter().find_map(|p| {
        lookup(root, p).and_then(|v| {
            v.as_f64()
                .or_else(|| v.as_u64().map(|n| n as f64))
                .or_else(|| v.as_str()?.parse::<f64>().ok())
        })
    })
}
fn lookup<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = root;
    for part in path.split('.') {
        cur = cur.get(part)?;
    }
    Some(cur)
}
