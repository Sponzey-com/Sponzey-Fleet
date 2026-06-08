#!/usr/bin/env sh
set -eu

PACKAGE="${SPONZEY_NPM_PACKAGE:-@sponzey/fleet}"
WORK_DIR="${TMPDIR:-/tmp}/sponzey-fleet-npm-registry-smoke-$$"

cleanup() {
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT INT TERM

mkdir -p "$WORK_DIR"
NPM_CONFIG_PREFIX="$WORK_DIR/prefix" \
NPM_CONFIG_CACHE="$WORK_DIR/cache" \
  npm install -g "$PACKAGE"

"$WORK_DIR/prefix/bin/sponzey" --help >/dev/null

echo "manual npm registry smoke ok: $PACKAGE"
