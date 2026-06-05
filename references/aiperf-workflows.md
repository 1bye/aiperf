# aiperf workflows

## agent-browser paired recording

Terminal 1:

```sh
agent-browser open http://localhost:3000
```

Terminal 2:

```sh
mkdir -p traces/aiperf
aiperf record \
  --cdp http://127.0.0.1:9222 \
  --out traces/aiperf/run.json \
  --network-sidecar traces/aiperf/run.network.jsonl \
  --stop-on-enter
```

Drive scenario in Terminal 1, press Enter in Terminal 2, then audit:

```sh
aiperf audit traces/aiperf/run.json \
  --network-sidecar traces/aiperf/run.network.jsonl \
  --project-root . \
  --out traces/aiperf/audit.md \
  --json-out traces/aiperf/audit.json \
  --evidence-out traces/aiperf/evidence.json
```

## Existing trace smoke test

```sh
mkdir -p traces/aiperf
aiperf audit /path/to/trace.json \
  --project-root . \
  --out traces/aiperf/audit.md \
  --json-out traces/aiperf/audit.json \
  --evidence-out traces/aiperf/evidence.json
```

## Fix loop

1. Read `audit.md` and `evidence.json`.
2. Pick top actionable `finding_*`.
3. Inspect cited files only first.
4. Change code.
5. Capture same flow again.
6. Compare:

```sh
aiperf compare traces/aiperf/before.json traces/aiperf/after.json \
  --project-root . \
  --out traces/aiperf/compare.md \
  --json-out traces/aiperf/compare.json
```

## Evidence discipline

- Use `finding_*` IDs for root-cause claims.
- Use `ev_*` IDs for trace event claims.
- Use `net_*` IDs for network sidecar claims.
- If source maps are missing, say source confidence is limited.
- If network sidecar is missing, do not claim WebSocket/fetch causality.
