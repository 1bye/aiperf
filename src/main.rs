use aiperf::analysis::{AuditOptions, audit_trace, compare_traces};
use aiperf::cdp::{RecordOptions, record_trace};
use aiperf::report::{
    json::write_json,
    llm_pack::{pack_from_report, prompt_for_pack, write_pack},
    markdown::{audit_markdown, compare_markdown},
};
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use tokio::process::Command;
use tokio::sync::oneshot;

#[derive(Parser)]
#[command(
    name = "aiperf",
    version,
    about = "Local-first Chrome/React performance trace analyzer"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Audit(AuditCmd),
    Compare(CompareCmd),
    Query(QueryCmd),
    Evidence(EvidenceCmd),
    Pack(PackCmd),
    Record(RecordCmd),
    Run(RunCmd),
    Mcp,
    Doctor(DoctorCmd),
}

#[derive(Args, Clone)]
struct AuditCmd {
    trace: PathBuf,
    #[arg(long, default_value = ".")]
    project_root: PathBuf,
    #[arg(long)]
    network_sidecar: Option<PathBuf>,
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long)]
    json_out: Option<PathBuf>,
    #[arg(long)]
    evidence_out: Option<PathBuf>,
    #[arg(long)]
    long_task_ms: Option<f64>,
    #[arg(long)]
    bucket_ms: Option<f64>,
}
#[derive(Args)]
struct CompareCmd {
    before: PathBuf,
    after: PathBuf,
    #[arg(long, default_value = ".")]
    project_root: PathBuf,
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long)]
    json_out: Option<PathBuf>,
}
#[derive(Args)]
struct QueryCmd {
    trace: PathBuf,
    query: String,
    #[arg(long, default_value_t = 50.0)]
    min_ms: f64,
    #[arg(long)]
    ts_ms: Option<f64>,
    #[arg(long, default_value_t = 250.0)]
    window_ms: f64,
    #[arg(long, default_value_t = 50)]
    limit: usize,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    network_sidecar: Option<PathBuf>,
    #[arg(long, default_value = ".")]
    project_root: PathBuf,
}
#[derive(Args)]
struct EvidenceCmd {
    trace: PathBuf,
    finding_id: String,
    #[arg(long, default_value_t = 250.0)]
    context_ms: f64,
    #[arg(long)]
    json: bool,
    #[arg(long, default_value = ".")]
    project_root: PathBuf,
    #[arg(long)]
    network_sidecar: Option<PathBuf>,
}
#[derive(Args)]
struct PackCmd {
    trace: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    prompt_out: Option<PathBuf>,
    #[arg(long, default_value = ".")]
    project_root: PathBuf,
    #[arg(long)]
    network_sidecar: Option<PathBuf>,
}
#[derive(Args, Clone)]
struct RecordCmd {
    #[arg(long, default_value = "http://127.0.0.1:9222")]
    cdp: String,
    #[arg(long)]
    out: PathBuf,
    #[arg(long)]
    network_sidecar: Option<PathBuf>,
    #[arg(long)]
    stop_on_enter: bool,
    #[arg(long)]
    target_url: Option<String>,
    #[arg(long)]
    target_title: Option<String>,
    #[arg(long)]
    categories: Option<String>,
    #[arg(long, default_value = "runtime-react")]
    preset: String,
    #[arg(long)]
    buffer_size_kb: Option<u64>,
    #[arg(long)]
    gzip: bool,
    #[arg(long)]
    include_payloads: bool,
}
#[derive(Args)]
struct RunCmd {
    #[command(flatten)]
    record: RecordCmd,
    #[arg(last = true, required = true)]
    command: Vec<String>,
}
#[derive(Args)]
struct DoctorCmd {
    #[arg(long, default_value = "http://127.0.0.1:9222")]
    cdp: String,
    #[arg(long, default_value = ".")]
    project_root: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .ok();
    match Cli::parse().command {
        Commands::Audit(cmd) => run_audit(cmd)?,
        Commands::Compare(cmd) => run_compare(cmd)?,
        Commands::Query(cmd) => run_query(cmd)?,
        Commands::Evidence(cmd) => run_evidence(cmd)?,
        Commands::Pack(cmd) => run_pack(cmd)?,
        Commands::Record(cmd) => record_trace(record_options(cmd)).await?,
        Commands::Run(cmd) => run_wrapped(cmd).await?,
        Commands::Mcp => aiperf::mcp::server::run_stdio()?,
        Commands::Doctor(cmd) => run_doctor(cmd).await?,
    }
    Ok(())
}

fn audit_options(
    project_root: PathBuf,
    sidecar: Option<PathBuf>,
    long_task_ms: Option<f64>,
    bucket_ms: Option<f64>,
) -> AuditOptions {
    AuditOptions {
        project_root,
        network_sidecar: sidecar,
        long_task_ms,
        bucket_ms,
        top_n: None,
    }
}
fn run_audit(cmd: AuditCmd) -> anyhow::Result<()> {
    let opts = audit_options(
        cmd.project_root,
        cmd.network_sidecar,
        cmd.long_task_ms,
        cmd.bucket_ms,
    );
    let report = audit_trace(&cmd.trace, &opts)?;
    if let Some(out) = cmd.out {
        std::fs::write(&out, audit_markdown(&report))
            .map_err(|e| anyhow::anyhow!("write {}: {e}", out.display()))?;
    } else {
        println!("{}", audit_markdown(&report));
    }
    if let Some(out) = cmd.json_out {
        write_json(&report, &out)?;
    }
    if let Some(out) = cmd.evidence_out {
        let pack = pack_from_report(&report, 20);
        write_pack(&pack, &out)?;
    }
    Ok(())
}
fn run_compare(cmd: CompareCmd) -> anyhow::Result<()> {
    let opts = audit_options(cmd.project_root, None, None, None);
    let report = compare_traces(&cmd.before, &cmd.after, &opts)?;
    if let Some(out) = cmd.out {
        std::fs::write(&out, compare_markdown(&report))
            .map_err(|e| anyhow::anyhow!("write {}: {e}", out.display()))?;
    } else {
        println!("{}", compare_markdown(&report));
    }
    if let Some(out) = cmd.json_out {
        write_json(&report, &out)?;
    }
    Ok(())
}
fn run_pack(cmd: PackCmd) -> anyhow::Result<()> {
    let opts = audit_options(cmd.project_root, cmd.network_sidecar, None, None);
    let report = audit_trace(&cmd.trace, &opts)?;
    let pack = pack_from_report(&report, 20);
    write_pack(&pack, &cmd.out)?;
    if let Some(prompt) = cmd.prompt_out {
        std::fs::write(&prompt, prompt_for_pack(&cmd.out))
            .map_err(|e| anyhow::anyhow!("write {}: {e}", prompt.display()))?;
    }
    Ok(())
}

fn run_query(cmd: QueryCmd) -> anyhow::Result<()> {
    let opts = audit_options(
        cmd.project_root,
        cmd.network_sidecar,
        Some(cmd.min_ms),
        None,
    );
    let report = audit_trace(&cmd.trace, &opts)?;
    let value = match cmd.query.as_str() {
        "long-tasks" => serde_json::to_value(
            report
                .long_tasks
                .iter()
                .filter(|t| t.dur_ms >= cmd.min_ms)
                .take(cmd.limit)
                .collect::<Vec<_>>(),
        )?,
        "event-window" => {
            let ts = cmd.ts_ms.unwrap_or(0.0);
            let rows: Vec<_> = report
                .long_tasks
                .iter()
                .filter(|t| {
                    t.ts_ms <= ts + cmd.window_ms && t.ts_ms + t.dur_ms >= ts - cmd.window_ms
                })
                .collect();
            serde_json::to_value(rows)?
        }
        "top-functions" => serde_json::to_value(
            report
                .cpu_profile
                .functions
                .iter()
                .take(cmd.limit)
                .collect::<Vec<_>>(),
        )?,
        "react-commits" => serde_json::to_value(
            report
                .react
                .long_commits
                .iter()
                .filter(|r| r.total_ms >= cmd.min_ms)
                .take(cmd.limit)
                .collect::<Vec<_>>(),
        )?,
        "websocket-bursts" => serde_json::to_value(
            report
                .realtime
                .bursts
                .iter()
                .take(cmd.limit)
                .collect::<Vec<_>>(),
        )?,
        other => return Err(anyhow::anyhow!("unknown query {other}")),
    };
    let _ = cmd.json;
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}
fn run_evidence(cmd: EvidenceCmd) -> anyhow::Result<()> {
    let opts = audit_options(cmd.project_root, cmd.network_sidecar, None, None);
    let report = audit_trace(&cmd.trace, &opts)?;
    let finding = report
        .findings
        .iter()
        .find(|f| f.id == cmd.finding_id)
        .ok_or_else(|| anyhow::anyhow!("finding {} not found", cmd.finding_id))?;
    let _ = cmd.json;
    println!("{}", serde_json::to_string_pretty(finding)?);
    let _ = cmd.context_ms;
    Ok(())
}

fn record_options(cmd: RecordCmd) -> RecordOptions {
    RecordOptions {
        cdp: cmd.cdp,
        out: cmd.out,
        network_sidecar: cmd.network_sidecar,
        stop_on_enter: cmd.stop_on_enter,
        target_url: cmd.target_url,
        target_title: cmd.target_title,
        categories: cmd.categories,
        preset: cmd.preset,
        buffer_size_kb: cmd.buffer_size_kb,
        gzip: cmd.gzip,
        include_payloads: cmd.include_payloads,
    }
}
async fn run_wrapped(cmd: RunCmd) -> anyhow::Result<()> {
    let mut record = cmd.record.clone();
    record.stop_on_enter = false;
    let (tx, rx) = oneshot::channel::<()>();
    let trace_task = tokio::spawn(async move {
        aiperf::cdp::tracing::record_trace_until(record_options(record), async move {
            rx.await
                .map_err(|_| anyhow::anyhow!("wrapped command stop signal dropped"))?;
            Ok(())
        })
        .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let mut child = Command::new(&cmd.command[0])
        .args(&cmd.command[1..])
        .spawn()
        .map_err(|e| anyhow::anyhow!("spawn wrapped command {:?}: {e}", cmd.command))?;
    let status = child.wait().await?;
    let _ = tx.send(());
    trace_task.await??;
    if !status.success() {
        return Err(anyhow::anyhow!("wrapped command exited with {status}"));
    }
    Ok(())
}
async fn run_doctor(cmd: DoctorCmd) -> anyhow::Result<()> {
    println!("aiperf doctor");
    println!(
        "project root: {} exists={}",
        cmd.project_root.display(),
        cmd.project_root.exists()
    );
    match aiperf::cdp::target::resolve_ws(&cmd.cdp, None, None).await {
        Ok(ws) => println!("CDP reachable: {ws}"),
        Err(e) => println!("CDP not reachable: {e}"),
    }
    println!("analysis without Chrome: supported");
    Ok(())
}
