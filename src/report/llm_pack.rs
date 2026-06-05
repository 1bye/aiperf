use crate::report::schema::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmEvidencePack {
    pub metadata: ReportMetadata,
    pub analysis_coverage: AnalysisCoverage,
    pub top_findings: Vec<Finding>,
    pub evidence_snippets: Vec<TraceEvidence>,
    pub top_source_files: Vec<String>,
    pub top_source_functions: Vec<SourceHotspot>,
    pub suggested_files_to_inspect: Vec<String>,
    pub reproduce_commands: Vec<String>,
    pub compare_commands: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn pack_from_report(r: &AuditReport, top_n: usize) -> LlmEvidencePack {
    let mut files = BTreeSet::new();
    let mut funcs = Vec::new();
    let mut ev = Vec::new();
    for f in &r.findings {
        for e in &f.evidence {
            ev.push(e.clone());
        }
        for rec in &f.recommendations {
            for file in &rec.files_to_inspect {
                files.insert(file.clone());
            }
        }
    }
    for lt in &r.long_tasks {
        for s in &lt.top_source_functions {
            if let Some(url) = &s.url {
                files.insert(url.clone());
            }
            funcs.push(s.clone());
        }
    }
    for f in &r.cpu_profile.functions {
        if let Some(url) = &f.url {
            files.insert(url.clone());
        }
        funcs.push(f.clone());
    }
    funcs.sort_by(|a, b| {
        b.total_ms
            .partial_cmp(&a.total_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    funcs.truncate(top_n);
    ev.truncate(top_n * 3);
    let trace = r.metadata.trace_path.display().to_string();
    let reproduce = vec![format!(
        "aiperf audit {} --project-root {} --out report.md --json-out report.json --evidence-out evidence.json",
        trace,
        r.metadata.project_root.display()
    )];
    let compare = vec!["aiperf compare before.json after.json --project-root . --out compare.md --json-out compare.json".to_string()];
    LlmEvidencePack {
        metadata: r.metadata.clone(),
        analysis_coverage: r.coverage.clone(),
        top_findings: r.findings.iter().take(top_n).cloned().collect(),
        evidence_snippets: ev,
        top_source_files: files.iter().take(top_n).cloned().collect(),
        top_source_functions: funcs,
        suggested_files_to_inspect: files.iter().take(top_n).cloned().collect(),
        reproduce_commands: reproduce,
        compare_commands: compare,
        warnings: r.warnings.clone(),
    }
}

pub fn write_pack(pack: &LlmEvidencePack, path: &Path) -> anyhow::Result<()> {
    crate::report::json::write_json(pack, path)
}

pub fn prompt_for_pack(pack_path: &Path) -> String {
    format!(
        "# Performance trace fix prompt\n\nUse only evidence from `{}`. Do not invent trace facts. Do not inspect raw Chrome traces. Inspect listed source files first. Every performance claim must cite finding IDs or evidence IDs from the pack. Propose code changes tied to findings. After changes, rerun same profiling command from `reproduce_commands`, then run compare command from `compare_commands`. Treat unsupported ideas as hypotheses, not causes.\n",
        pack_path.display()
    )
}
