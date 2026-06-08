#!/usr/bin/env sh
set -eu

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO_ROOT="$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)"
BIN="$REPO_ROOT/target/debug/sponzey"

if [ ! -x "$BIN" ]; then
  cargo build --manifest-path "$REPO_ROOT/Cargo.toml" -p fleet-cli
fi

if [ "$#" -eq 0 ]; then
  set -- --host 127.0.0.1 --port 7700 --data-dir .sponzey --dev-insecure-loopback
fi

exec "$BIN" controller start "$@"
