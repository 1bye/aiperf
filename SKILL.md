---
name: aiperf
description: Chrome/React runtime performance profiling with local-first aiperf traces, evidence-backed findings, agent-browser pairing, and before/after verification.
---

# aiperf runtime performance profiling

Use this skill for Chrome/React performance investigations, especially realtime-heavy apps and large Chrome DevTools traces.

## Hard rules

- MUST use `aiperf` for trace auditing. Do not inspect raw 300MB+ Chrome trace JSON manually.
- MUST NOT send raw traces to an LLM.
- MUST cite `finding_*`, `ev_*`, and/or `net_*` IDs for every performance claim.
- MUST treat recommendations without cited evidence as hypotheses, not root causes.
- MUST generate or read Markdown report, JSON report, and LLM evidence pack before code changes.
- MUST make smallest safe code fix tied to top actionable evidence.
- MUST reprofile after changes and run `aiperf compare before after` when possible.

## Preferred workflow

1. Open app and drive scenario with `agent-browser` or existing automation.
2. Capture trace using either:
   - `aiperf record --cdp http://127.0.0.1:9222 --out traces/run.json --network-sidecar traces/run.network.jsonl --stop-on-enter`
   - or `agent-browser trace start/stop` then audit saved trace.
3. Audit:

```sh
aiperf audit traces/run.json \
  --network-sidecar traces/run.network.jsonl \
  --project-root . \
  --out traces/aiperf/audit.md \
  --json-out traces/aiperf/audit.json \
  --evidence-out traces/aiperf/evidence.json
```

4. Read `audit.md` and `evidence.json`; identify top actionable finding.
5. Inspect only cited source files/components first.
6. Fix code. Keep claim chain tied to evidence IDs.
7. Reprofile same flow and compare:

```sh
aiperf compare traces/before.json traces/after.json \
  --project-root . \
  --out traces/aiperf/compare.md \
  --json-out traces/aiperf/compare.json
```

## Commands

### Audit existing trace

```sh
aiperf audit TRACE.json --project-root . --out audit.md --json-out audit.json --evidence-out evidence.json
```

### Query facts

```sh
aiperf query TRACE.json long-tasks --min-ms 50 --json
aiperf query TRACE.json event-window --ts-ms 12345 --window-ms 250 --json
aiperf query TRACE.json top-functions --limit 50 --json
aiperf query TRACE.json react-commits --min-ms 16 --json
aiperf query TRACE.json websocket-bursts --window-ms 250 --json
```

### Evidence drilldown

```sh
aiperf evidence TRACE.json finding_001 --context-ms 250 --json
```

### LLM evidence pack

```sh
aiperf pack TRACE.json --out llm-pack.json --prompt-out llm-prompt.md
```

## Interpretation guidance

- `critical/high` findings with `high/medium` confidence are preferred fix targets.
- Missing network sidecar means realtime/WebSocket causality is limited.
- Missing source maps means source-level confidence is limited.
- React Performance Tracks present => React evidence is first-class.
- React fallback inference from stacks => use lower-confidence language.
- Long task with dominant JS => split work, workerize parse/normalize, batch state updates.
- Long task with layout/style => batch DOM reads/writes, reduce measured DOM, virtualize.
- WebSocket burst correlated with long task => coalesce messages, batch per frame, selectors/backpressure.

## Output requirement for agents

When reporting, include:

- finding IDs addressed
- evidence IDs cited
- files changed and why
- verification command run
- before/after comparison result or exact blocker
