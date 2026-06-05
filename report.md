# AI Performance Audit

## Executive Summary

- **Overall diagnosis**: Main-thread long tasks dominate runtime trace
- **Top 3 root causes**:
  - Main-thread long tasks dominate runtime trace (Medium, Medium)
  - Realtime burst aligns with blocking main-thread work (Medium, Medium)
  - React work is measurable in runtime trace (Low, High)
- **Biggest bottleneck category**: js / RunTask
- **Trace reliability**: data_loss=false truncated=false
- **Evidence availability**: React tracks=true source maps=false network sidecar events=3

### Warnings

- Source maps absent or not found; code-level confidence is limited for bundled/minified frames.

## Analysis Coverage

- **Trace file**: tests/fixtures/realtime_ws_trace.json (2301 bytes)
- **Events scanned**: 12
- **Trace duration**: 280.00ms
- **PIDs**: [1]
- **Selected main thread**: pid=1 tid=10 confidence=high (Selected thread name Some("CrRendererMain") as renderer/browser main thread.)
- **Data loss**: false
- **Unknown event names**: 4
- **Long tasks found**: 1
- **React events found**: 1
- **Network/WebSocket sidecar events**: 3

### Time bucket scan summary

| Bucket | Window | Busy | JS | Style | Layout | Paint | Composite | GC | React | Unknown |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 0 | 0-1000ms | 16.0% | 212.00ms | 18.00ms | 22.00ms | 6.00ms | 0.00ms | 18.00ms | 28.00ms | 0.00ms |

### Anomaly windows

- highest_main_thread_busy_pct: highest_main_thread_busy_pct at 0.00-1000.00ms value 16.00
- highest_js_time: highest_js_time at 0.00-1000.00ms value 212.00
- highest_style_layout_time: highest_style_layout_time at 0.00-1000.00ms value 40.00
- highest_paint_composite_time: highest_paint_composite_time at 0.00-1000.00ms value 6.00
- highest_gc_time: highest_gc_time at 0.00-1000.00ms value 18.00
- highest_react_time: highest_react_time at 0.00-1000.00ms value 28.00
- densest_network_burst: densest_network_burst at 0.00-1000.00ms value 28.67

## Top Findings

### finding_001: Main-thread long tasks dominate runtime trace

Severity: Medium
Confidence: Medium
Impact: total 130.00ms worst 130.00ms count 1

Cause chain:
1. network_or_message_activity — 3 network/WS events occur before worst task Evidence: ["net_000001", "net_000002", "net_000003"]
2. main_thread_long_task — 130ms task blocks main thread with JS 52.00ms React 28.00ms style/layout 40.00ms Evidence: ["ev_000001"]

Evidence:
- ev_000001 trace_event=4 RunTask @1.00ms dur=130.00ms pid=1 tid=10 category=js

Recommended fix:
- **Reduce dominant main-thread work inside long tasks**: Split work across frames, remove unnecessary renders/layout reads, and move parsing/normalization to a worker when CPU work dominates. Files: ["src/stores/prices.ts"]. Risk: medium

How to verify:
- Record same user flow, run `aiperf audit` on new trace, then `aiperf compare before after`. Expected: Reduced long tasks over threshold with same or better analysis coverage and no trace data loss.

### finding_002: Realtime burst aligns with blocking main-thread work

Severity: Medium
Confidence: Medium
Impact: total 130.00ms worst 130.00ms count 1

Cause chain:
1. websocket_burst — websocket 3 messages / 28672 bytes ended 0.30ms before 130.00ms main-thread task Evidence: ["burst_001"]
2. state_update_or_js_work — Following task lasts 130.00ms Evidence: ["ev_000004"]

Evidence:
- ev_000004 trace_event=4 RunTask @1.00ms dur=130.00ms pid=1 tid=10 category=js

Recommended fix:
- **Batch realtime updates before React/state fanout**: Coalesce WebSocket/EventSource messages per animation frame, drop intermediate states, use store selectors, and defer heavy parse/normalize work to a Worker. Files: []. Risk: medium

How to verify:
- Record same user flow, run `aiperf audit` on new trace, then `aiperf compare before after`. Expected: Reduced WebSocket bursts and correlated long tasks with same or better analysis coverage and no trace data loss.

### finding_003: React work is measurable in runtime trace

Severity: Low
Confidence: High
Impact: total 28.00ms worst 28.00ms count 1

Cause chain:
1. react_render_or_commit — React phase commit component Some("PriceTable") totals 28.00ms Evidence: ["ev_000006"]

Evidence:
- ev_000006 trace_event=6 React Commit @60.00ms dur=28.00ms pid=1 tid=10 category=react

Recommended fix:
- **Reduce dominant main-thread work inside long tasks**: Split work across frames, remove unnecessary renders/layout reads, and move parsing/normalization to a worker when CPU work dominates. Files: []. Risk: medium

How to verify:
- Record same user flow, run `aiperf audit` on new trace, then `aiperf compare before after`. Expected: Reduced React commit/render duration with same or better analysis coverage and no trace data loss.

### finding_004: Expensive style/layout work found

Severity: Medium
Confidence: Medium
Impact: total 40.00ms worst 22.00ms count 2

Cause chain:
1. layout_paint — Layout takes 22.00ms dirty Some(1200)/Some(5000) Evidence: ["ev_000008"]

Evidence:
- ev_000008 trace_event=8 Layout @110.00ms dur=22.00ms pid=1 tid=10 category=layout

Recommended fix:
- **Remove forced layout/style work from interaction path**: Batch DOM reads before writes, reduce measured DOM size, virtualize large lists, and reduce IntersectionObserver targets. Files: []. Risk: medium

How to verify:
- Record same user flow, run `aiperf audit` on new trace, then `aiperf compare before after`. Expected: Reduced style/layout duration and dirty object counts with same or better analysis coverage and no trace data loss.

### finding_005: Garbage collection contributes to frame risk

Severity: Medium
Confidence: Medium
Impact: total 18.00ms worst 18.00ms count 1

Cause chain:
1. gc_pause — MinorGC lasts 18.00ms Evidence: ["ev_000011"]

Evidence:
- ev_000011 trace_event=11 MinorGC @255.00ms dur=18.00ms pid=1 tid=10 category=gc

Recommended fix:
- **Reduce dominant main-thread work inside long tasks**: Split work across frames, remove unnecessary renders/layout reads, and move parsing/normalization to a worker when CPU work dominates. Files: []. Risk: medium

How to verify:
- Record same user flow, run `aiperf audit` on new trace, then `aiperf compare before after`. Expected: Reduced GC pause duration with same or better analysis coverage and no trace data loss.

## Long Tasks

Total long tasks in JSON: 1

| ID | Event | Start | Duration | JS | React | Style | Layout | Paint/Composite | GC |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| long_task_001 | 4 | 1.00ms | 130.00ms | 52.00ms | 28.00ms | 18.00ms | 22.00ms | 0.00ms | 0.00ms |

## React Runtime

- Tracks present: true
- Events: 1 inferred: 0

- Some("PriceTable") phase=commit total=28.00ms count=1 p95=28.00ms evidence=react_performance_track

## Realtime / Network Bursts

Sidecar events: 3

- burst_001 websocket count=3 bytes=28672 window=0.3-0.7ms
- Correlation burst_001 -> task 4 confidence=medium: websocket 3 messages / 28672 bytes ended 0.30ms before 130.00ms main-thread task

## Layout / Style / Paint

Style total=18.00ms layout total=22.00ms

- Layout event=8 110.00ms dur=22.00ms dirty=Some(1200)/Some(5000)
- UpdateLayoutTree event=7 90.00ms dur=18.00ms dirty=None/None

## CPU Profile / Source Hotspots

Total sample=60.00ms app=50.00ms third_party=-0.00ms react_runtime=10.00ms native=-0.00ms unresolved=0

- app applyPriceBatch 50.00ms Some("src/stores/prices.ts")
- react/runtime commitRoot 10.00ms Some("node_modules/react-dom/cjs/react-dom-client.production.js")

## LLM Evidence Pack

Path is determined by `--evidence-out` or `aiperf pack --out`.

## Appendix

### Event inventory by total duration

| Event | Category | Count | Total |
|---|---:|---:|---:|
| RunTask | js | 2 | 160.00ms |
| FunctionCall | js | 1 | 52.00ms |
| React Commit | react | 1 | 28.00ms |
| Layout | layout | 1 | 22.00ms |
| UpdateLayoutTree | style | 1 | 18.00ms |
| MinorGC | gc | 1 | 18.00ms |
| Paint | paint | 1 | 6.00ms |
| ProfileChunk | unknown | 1 | 0.00ms |
| TracingStartedInBrowser | unknown | 1 | 0.00ms |
| thread_name | unknown | 1 | 0.00ms |
| process_name | unknown | 1 | 0.00ms |

### Thresholds used

Long task threshold and bucket size come from `.aiperf.toml` or CLI overrides.
