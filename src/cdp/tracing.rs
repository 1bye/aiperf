use super::client::{CdpClient, CdpMessage};
use super::target::resolve_ws;
use crate::trace::network_sidecar::redact_url;
use base64::Engine;
use flate2::read::GzDecoder;
use serde_json::{Value, json};
use std::io::Read;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, BufReader};

#[derive(Debug, Clone)]
pub struct RecordOptions {
    pub cdp: String,
    pub out: PathBuf,
    pub network_sidecar: Option<PathBuf>,
    pub stop_on_enter: bool,
    pub target_url: Option<String>,
    pub target_title: Option<String>,
    pub categories: Option<String>,
    pub preset: String,
    pub buffer_size_kb: Option<u64>,
    pub gzip: bool,
    pub include_payloads: bool,
}

pub async fn record_trace(opts: RecordOptions) -> anyhow::Result<()> {
    record_trace_until(opts.clone(), wait_for_stop(opts.stop_on_enter)).await
}

pub async fn record_trace_until<F>(opts: RecordOptions, stop: F) -> anyhow::Result<()>
where
    F: std::future::Future<Output = anyhow::Result<()>>,
{
    let ws = resolve_ws(
        &opts.cdp,
        opts.target_url.as_deref(),
        opts.target_title.as_deref(),
    )
    .await?;
    let mut client = CdpClient::connect(&ws).await?;
    let mut sidecar = match &opts.network_sidecar {
        Some(path) => Some(
            std::fs::File::create(path)
                .map_err(|e| anyhow::anyhow!("create network sidecar {}: {e}", path.display()))?,
        ),
        None => None,
    };
    if let Some(f) = &mut sidecar {
        use std::io::Write;
        writeln!(
            f,
            "{}",
            json!({"kind":"clock_alignment","wall_time_ms": now_ms(), "note":"CDP Network timestamps are monotonic seconds; trace timestamps are Chrome microseconds and are correlated by temporal order."})
        )?;
    }
    client
        .call_collecting_events("Network.enable", json!({}), |_, _| Ok(()))
        .await?;
    let categories = opts
        .categories
        .clone()
        .unwrap_or_else(|| preset_categories(&opts.preset).to_string());
    let trace_config = json!({"recordMode":"recordContinuously", "traceBufferSizeInKb": opts.buffer_size_kb.unwrap_or(200_000), "enableSampling": true, "includedCategories": categories.split(',').collect::<Vec<_>>()});
    let params = json!({"transferMode":"ReturnAsStream", "streamFormat":"json", "streamCompression": if opts.gzip { "gzip" } else { "none" }, "bufferUsageReportingInterval": 1000, "traceConfig": trace_config});
    client
        .call_collecting_events("Tracing.start", params, |method, params| {
            handle_sidecar(method, params, sidecar.as_mut(), opts.include_payloads)
        })
        .await?;
    stop.await?;
    let end_id = client.send("Tracing.end", json!({})).await?;
    let (stream_handle, compression, data_loss) = loop {
        match client.recv().await? {
            Some(CdpMessage::Response { id, error, .. }) if id == end_id => {
                if let Some(e) = error {
                    return Err(anyhow::anyhow!("CDP Tracing.end error: {e}"));
                }
            }
            Some(CdpMessage::Event { method, params }) if method == "Tracing.tracingComplete" => {
                let data_loss = params
                    .get("dataLossOccurred")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let stream_handle = params
                    .get("stream")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let compression = params
                    .get("streamCompression")
                    .and_then(Value::as_str)
                    .unwrap_or("none")
                    .to_string();
                break (stream_handle, compression, data_loss);
            }
            Some(CdpMessage::Event { method, params }) => {
                handle_sidecar(&method, &params, sidecar.as_mut(), opts.include_payloads)?
            }
            Some(_) => {}
            None => return Err(anyhow::anyhow!("CDP closed before tracingComplete")),
        }
    };
    let handle =
        stream_handle.ok_or_else(|| anyhow::anyhow!("Tracing completed without IO stream"))?;
    let mut bytes = Vec::new();
    loop {
        let result = client
            .call_collecting_events("IO.read", json!({"handle": handle}), |method, params| {
                handle_sidecar(method, params, sidecar.as_mut(), opts.include_payloads)
            })
            .await?;
        if let Some(data) = result.get("data").and_then(Value::as_str) {
            if result
                .get("base64Encoded")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                bytes.extend(base64::engine::general_purpose::STANDARD.decode(data)?);
            } else {
                bytes.extend_from_slice(data.as_bytes());
            }
        }
        if result.get("eof").and_then(Value::as_bool).unwrap_or(false) {
            break;
        }
    }
    let _ = client
        .call_collecting_events("IO.close", json!({"handle": handle}), |_, _| Ok(()))
        .await;
    let output = if compression == "gzip" {
        let mut gz = GzDecoder::new(bytes.as_slice());
        let mut decoded = Vec::new();
        gz.read_to_end(&mut decoded)?;
        decoded
    } else {
        bytes
    };
    std::fs::write(&opts.out, output)
        .map_err(|e| anyhow::anyhow!("write trace {}: {e}", opts.out.display()))?;
    if data_loss {
        eprintln!("warning: Chrome reported trace dataLossOccurred=true");
    }
    Ok(())
}

async fn wait_for_stop(stop_on_enter: bool) -> anyhow::Result<()> {
    if stop_on_enter {
        let mut line = String::new();
        let mut reader = BufReader::new(tokio::io::stdin());
        eprintln!("Recording trace. Press Enter to stop.");
        reader.read_line(&mut line).await?;
    } else {
        tokio::signal::ctrl_c().await?;
    }
    Ok(())
}

fn handle_sidecar(
    method: &str,
    params: &Value,
    sidecar: Option<&mut std::fs::File>,
    include_payloads: bool,
) -> anyhow::Result<()> {
    if let Some(f) = sidecar
        && method.starts_with("Network.")
    {
        use std::io::Write;
        let event = sanitize_network_event(method, params, include_payloads);
        writeln!(f, "{}", event)?;
    }
    if method == "Tracing.bufferUsage" {
        let pct = params
            .get("percentFull")
            .or_else(|| params.get("value"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        if pct > 0.80 {
            eprintln!("warning: trace buffer {:.0}% full", pct * 100.0);
        }
    }
    Ok(())
}

pub fn sanitize_network_event(method: &str, params: &Value, include_payloads: bool) -> Value {
    let mut v = json!({"method": method, "params": params});
    if !include_payloads && let Some(p) = v.get_mut("params") {
        redact_url_fields(p);
        if let Some(resp) = p.get_mut("response")
            && let Some(data) = resp.get_mut("payloadData")
        {
            let len = data.as_str().map(|s| s.len()).unwrap_or(0);
            *data = json!(format!("<redacted:{} bytes>", len));
        }
        if let Some(req) = p.get_mut("request")
            && let Some(o) = req.as_object_mut()
        {
            o.remove("postData");
        }
    }
    v
}
fn redact_url_fields(v: &mut Value) {
    for path in ["url", "documentURL"] {
        if let Some(u) = v.get(path).and_then(Value::as_str).map(redact_url) {
            v[path] = json!(u);
        }
    }
    for obj in ["request", "response"] {
        if let Some(o) = v.get_mut(obj)
            && let Some(u) = o.get("url").and_then(Value::as_str).map(redact_url)
        {
            o["url"] = json!(u);
        }
    }
}
fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}
fn preset_categories(preset: &str) -> &'static str {
    match preset {
        "scroll" => "devtools.timeline,disabled-by-default-devtools.timeline,blink,cc,input",
        "realtime" => "devtools.timeline,disabled-by-default-devtools.timeline,v8,netlog,loading",
        "loading" => "devtools.timeline,loading,netlog,v8,blink",
        "full" => "*",
        _ => {
            "devtools.timeline,disabled-by-default-devtools.timeline,disabled-by-default-v8.cpu_profiler,blink,cc,v8,loading,netlog"
        }
    }
}
