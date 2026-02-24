#!/usr/bin/env bash
# run.sh — Launch parse_terminal_bot in a new tmux window
set -euo pipefail

WINDOW="parse_terminal_bot"
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="$DIR/target/release/parse_terminal_bot"
CONFIG="$DIR/config.toml"
PROJECT="${PROJECT:-python}"

if [ "${1:-}" = "--project" ]; then
  if [ -z "${2:-}" ]; then
    echo "Missing value for --project (expected: python|rust)"
    exit 2
  fi
  PROJECT="$2"
  shift 2
elif [ -n "${1:-}" ]; then
  PROJECT="$1"
  shift
fi

if [ "$PROJECT" != "python" ] && [ "$PROJECT" != "rust" ]; then
  echo "Invalid project '$PROJECT' (expected: python|rust)"
  exit 2
fi

if [ ! -f "$BINARY" ]; then
  echo "Binary not found — building..."
  cargo build --release --manifest-path "$DIR/Cargo.toml"
fi

if [ -n "${TMUX:-}" ]; then
  # Already inside tmux — open a new window
  tmux new-window -n "$WINDOW" "cd '$DIR' && RUST_LOG=info '$BINARY' '$CONFIG' --project '$PROJECT'"
else
  # Outside tmux — create a new session
  tmux new-session -s "$WINDOW" "cd '$DIR' && RUST_LOG=info '$BINARY' '$CONFIG' --project '$PROJECT'"
fi
