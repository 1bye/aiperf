# aiperf

Local-first Rust CLI for Chrome/React runtime performance trace analysis. CLI-first, JSON/Markdown/LLM-pack output, no TUI, no cloud upload, no raw-trace-to-LLM workflow.

Inspired by [`chperf`](https://github.com/azihsoyn/chperf) trace parsing and analysis ideas. chperf is MIT licensed; aiperf keeps attribution and does not reuse its TUI.

## Workflows

### Analyze existing trace

```sh
aiperf audit tests/fixtures/realtime_ws_trace.json \
  --project-root . \
  --out report.md \
  --json-out report.json \
  --evidence-out evidence.json
```

### Analyze agent-browser trace

```sh
agent-browser trace start
agent-browser eval "window.scrollTo(0, document.body.scrollHeight)"
agent-browser trace stop traces/run.json

aiperf audit traces/run.json \
  --project-root . \
  --out traces/run.md \
  --json-out traces/run.report.json
```

### Record while another agent drives Chrome

Terminal 1:

```sh
agent-browser open http://localhost:3000
```

Terminal 2:

```sh
aiperf record \
  --cdp http://127.0.0.1:9222 \
  --out traces/run.json \
  --network-sidecar traces/run.network.jsonl \
  --stop-on-enter
```

Terminal 1:

```sh
agent-browser snapshot
agent-browser click @e1
agent-browser eval "window.scrollTo(0, document.body.scrollHeight)"
```

Terminal 2: press Enter, then:

```sh
aiperf audit traces/run.json \
  --network-sidecar traces/run.network.jsonl \
  --project-root . \
  --out traces/run.md \
  --json-out traces/run.report.json \
  --evidence-out traces/run.evidence.json
```

### Wrap automation command

```sh
aiperf run \
  --cdp http://127.0.0.1:9222 \
  --out traces/agent-run.trace.json \
  --network-sidecar traces/agent-run.network.jsonl \
  -- agent-browser eval "window.scrollTo(0, document.body.scrollHeight)"
```

### Compare before/after

```sh
aiperf compare tests/fixtures/compare_before.json tests/fixtures/compare_after.json \
  --project-root . \
  --out compare.md \
  --json-out compare.json
```

### Query and evidence drilldown

```sh
aiperf query trace.json long-tasks --min-ms 50 --json
aiperf query trace.json event-window --ts-ms 12345 --window-ms 250 --json
aiperf query trace.json top-functions --limit 50 --json
aiperf query trace.json react-commits --min-ms 16 --json
aiperf query trace.json websocket-bursts --window-ms 250 --json

aiperf evidence trace.json finding_001 --context-ms 250 --json
```

### LLM evidence pack

```sh
aiperf pack trace.json --out llm-pack.json --prompt-out llm-prompt.md
```

Coding agents should use only finding/evidence IDs from the pack and rerun profiling + compare after changes.

## Config

`.aiperf.toml`:

```toml
[trace]
preset = "runtime-react"
long_task_ms = 50
bucket_ms = 1000
record_network_sidecar = true
redact_urls = true
include_payloads = false

[project]
root = "."
source_map_globs = ["dist/**/*.map", ".next/**/*.map", "build/**/*.map"]

[analysis]
react = true
scroll = true
realtime = true
layout = true
gc = true
cpu_profile = true

[report]
top_n = 20
full_json = true
```

## Runtime performance profiling policy

Use `aiperf` for Chrome/React performance investigations. Do not inspect raw Chrome traces manually and do not send raw traces to an LLM.

1. Use `agent-browser` to open and interact with app.
2. Use `aiperf record` or `agent-browser trace start/stop` to capture trace.
3. Use `aiperf audit` to generate Markdown report, JSON report, and LLM evidence pack.
4. Use only evidence IDs from report when making performance claims.
5. After code changes, capture new trace and run `aiperf compare before after`.

Never claim performance root cause unless it appears in `aiperf` findings or evidence pack.
