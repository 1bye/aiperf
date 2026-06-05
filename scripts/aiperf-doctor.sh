#!/usr/bin/env bash
set -euo pipefail

if ! command -v aiperf >/dev/null 2>&1; then
  echo "aiperf: missing from PATH" >&2
  exit 1
fi

echo "aiperf: $(command -v aiperf)"
aiperf --version

echo "doctor:"
aiperf doctor || true

echo "skills:"
for dir in "$HOME/.agents/skills/aiperf" "$HOME/.claude/skills/aiperf" "$HOME/.config/opencode/skills/aiperf" "$HOME/.codex/skills/aiperf"; do
  if [ -f "$dir/SKILL.md" ]; then
    echo "ok $dir/SKILL.md"
  else
    echo "missing $dir/SKILL.md"
  fi
done
