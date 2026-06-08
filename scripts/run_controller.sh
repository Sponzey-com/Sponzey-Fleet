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

case "${1:-}" in
  controller)
    shift
    if [ "${1:-}" = "start" ]; then
      shift
    fi
    ;;
  start)
    shift
    ;;
  agent)
    if [ "${2:-}" = "-h" ] || [ "${2:-}" = "--help" ]; then
      exec "$BIN" agent --help
    fi
    cat >&2 <<EOF
error: run_controller.sh wraps only 'sponzey controller start'

For agent commands, use:

  ./scripts/run_agent.sh --help
  "$BIN" agent --help
EOF
    exit 2
    ;;
esac

exec "$BIN" controller start "$@"
