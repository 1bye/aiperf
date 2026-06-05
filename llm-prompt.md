# Performance trace fix prompt

Use only evidence from `llm-pack.json`. Do not invent trace facts. Do not inspect raw Chrome traces. Inspect listed source files first. Every performance claim must cite finding IDs or evidence IDs from the pack. Propose code changes tied to findings. After changes, rerun same profiling command from `reproduce_commands`, then run compare command from `compare_commands`. Treat unsupported ideas as hypotheses, not causes.
