use crate::analysis::{AuditOptions, audit_trace, compare_traces};
use crate::report::llm_pack::pack_from_report;
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::path::PathBuf;

pub fn run_stdio() -> anyhow::Result<()> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    writeln!(
        stdout,
        "{}",
        json!({"server":"aiperf","tools":["analyze_trace","compare_traces","query_trace","get_evidence","generate_llm_pack","start_trace","stop_trace"]})
    )?;
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = serde_json::from_str(&line)?;
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let tool = req
            .get("tool")
            .or_else(|| req.get("method"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let args = req
            .get("args")
            .or_else(|| req.get("params"))
            .cloned()
            .unwrap_or(Value::Null);
        let result = match tool {
            "analyze_trace" => analyze(args),
            "compare_traces" => compare(args),
            "generate_llm_pack" => pack(args),
            "start_trace" | "stop_trace" => Ok(
                json!({"ok":false,"error":"Use `aiperf record` for streaming trace capture in this CLI build."}),
            ),
            _ => Ok(json!({"ok":false,"error":format!("unknown tool {tool}")})),
        };
        match result {
            Ok(v) => writeln!(stdout, "{}", json!({"id":id,"result":v}))?,
            Err(e) => writeln!(stdout, "{}", json!({"id":id,"error":e.to_string()}))?,
        }
        stdout.flush()?;
    }
    Ok(())
}
fn opts(args: &Value) -> AuditOptions {
    AuditOptions {
        project_root: args
            .get("project_root")
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(".")),
        network_sidecar: args
            .get("network_sidecar")
            .and_then(Value::as_str)
            .map(PathBuf::from),
        long_task_ms: None,
        bucket_ms: None,
        top_n: Some(10),
    }
}
fn analyze(args: Value) -> anyhow::Result<Value> {
    let trace = args
        .get("trace")
        .or_else(|| args.get("path"))
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("trace path required"))?;
    let report = audit_trace(PathBuf::from(trace).as_path(), &opts(&args))?;
    Ok(json!({"metadata":report.metadata,"coverage":report.coverage,"findings":report.findings}))
}
fn compare(args: Value) -> anyhow::Result<Value> {
    let before = args
        .get("before")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("before path required"))?;
    let after = args
        .get("after")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("after path required"))?;
    let report = compare_traces(
        PathBuf::from(before).as_path(),
        PathBuf::from(after).as_path(),
        &opts(&args),
    )?;
    Ok(
        json!({"deltas":report.deltas,"regressions":report.regressions,"improvements":report.improvements,"pr_summary":report.pr_summary}),
    )
}
fn pack(args: Value) -> anyhow::Result<Value> {
    let trace = args
        .get("trace")
        .or_else(|| args.get("path"))
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("trace path required"))?;
    let report = audit_trace(PathBuf::from(trace).as_path(), &opts(&args))?;
    Ok(serde_json::to_value(pack_from_report(&report, 10))?)
}
