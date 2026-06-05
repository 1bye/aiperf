#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
OUT=${1:-"$REPO_DIR/tmp/aiperf-skill.zip"}
case "$OUT" in
  /*) ;;
  *) OUT="$REPO_DIR/$OUT" ;;
esac
STAGE=$(mktemp -d)
cleanup() { rm -rf "$STAGE"; }
trap cleanup EXIT

mkdir -p "$STAGE/aiperf"
cp "$REPO_DIR/SKILL.md" "$STAGE/aiperf/SKILL.md"
for dir in scripts references agents; do
  if [ -d "$REPO_DIR/$dir" ]; then
    cp -R "$REPO_DIR/$dir" "$STAGE/aiperf/$dir"
  fi
done
if [ -f "$REPO_DIR/README.md" ]; then cp "$REPO_DIR/README.md" "$STAGE/README.md"; fi
if [ -f "$REPO_DIR/LICENSE" ]; then cp "$REPO_DIR/LICENSE" "$STAGE/LICENSE"; fi
chmod +x "$STAGE"/aiperf/scripts/*.sh 2>/dev/null || true
mkdir -p "$(dirname "$OUT")"
(cd "$STAGE" && zip -qr "$OUT" .)
echo "$OUT"
