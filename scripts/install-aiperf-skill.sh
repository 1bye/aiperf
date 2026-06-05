#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)

install_codex=false
install_all=false
install_admin_codex=false
cargo_install=""
binary=""

while [ $# -gt 0 ]; do
  case "$1" in
    --codex) install_codex=true ;;
    --all) install_all=true ;;
    --admin-codex) install_admin_codex=true ;;
    --cargo-install) shift; cargo_install=${1:?missing path for --cargo-install} ;;
    --binary) shift; binary=${1:?missing path for --binary} ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
  shift
done

copy_dir_if_present() {
  local source=$1
  local target=$2
  if [ -d "$source" ]; then
    mkdir -p "$target"
    cp -R "$source" "$target/"
  fi
}

copy_skill() {
  local target_root=$1
  local target="$target_root/aiperf"
  mkdir -p "$target_root"
  rm -rf "$target"
  mkdir -p "$target"
  cp "$REPO_DIR/SKILL.md" "$target/SKILL.md"
  copy_dir_if_present "$REPO_DIR/scripts" "$target"
  copy_dir_if_present "$REPO_DIR/references" "$target"
  copy_dir_if_present "$REPO_DIR/agents" "$target"
  if [ -f "$REPO_DIR/LICENSE" ]; then cp "$REPO_DIR/LICENSE" "$target/LICENSE"; fi
  if [ -f "$REPO_DIR/README.md" ]; then cp "$REPO_DIR/README.md" "$target/README.md"; fi
  chmod +x "$target"/scripts/*.sh 2>/dev/null || true
  echo "installed $target/SKILL.md"
}

if [ "$install_all" = true ]; then
  copy_skill "$HOME/.agents/skills"
  copy_skill "$HOME/.claude/skills"
  copy_skill "$HOME/.config/opencode/skills"
  copy_skill "$HOME/.codex/skills"
elif [ "$install_codex" = true ]; then
  copy_skill "$HOME/.agents/skills"
elif [ "$install_admin_codex" = true ]; then
  copy_skill "/etc/codex/skills"
else
  copy_skill "$HOME/.agents/skills"
fi

if [ -n "$cargo_install" ]; then
  cargo install --path "$cargo_install" --locked
fi

if [ -n "$binary" ]; then
  mkdir -p "$HOME/.local/bin"
  ln -sf "$binary" "$HOME/.local/bin/aiperf"
  echo "linked $HOME/.local/bin/aiperf -> $binary"
fi
