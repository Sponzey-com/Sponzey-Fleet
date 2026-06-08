#!/usr/bin/env sh
set -eu

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
REPO_ROOT="$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)"
WORK_DIR="${TMPDIR:-/tmp}/sponzey-fleet-npm-platform-smoke-$$"

cleanup() {
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT INT TERM

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

case "$OS" in
  darwin) PLATFORM_OS="darwin" ;;
  linux) PLATFORM_OS="linux" ;;
  *)
    echo "unsupported smoke OS: $OS" >&2
    exit 1
    ;;
esac

case "$ARCH" in
  arm64|aarch64) PLATFORM_ARCH="arm64" ;;
  x86_64|amd64) PLATFORM_ARCH="x64" ;;
  *)
    echo "unsupported smoke arch: $ARCH" >&2
    exit 1
    ;;
esac

PLATFORM_DIR="$REPO_ROOT/npm/fleet-$PLATFORM_OS-$PLATFORM_ARCH"
STAGED_PLATFORM_DIR="$WORK_DIR/fleet-$PLATFORM_OS-$PLATFORM_ARCH"
PLATFORM_BIN="$STAGED_PLATFORM_DIR/bin/sponzey"

cd "$REPO_ROOT"
cargo build -p fleet-cli >/dev/null

mkdir -p "$STAGED_PLATFORM_DIR/bin" "$WORK_DIR"
cp "$PLATFORM_DIR/package.json" "$STAGED_PLATFORM_DIR/package.json"
cp "$PLATFORM_DIR/README.md" "$STAGED_PLATFORM_DIR/README.md"
cp "$REPO_ROOT/target/debug/sponzey" "$PLATFORM_BIN"
chmod +x "$PLATFORM_BIN"

(
  cd "$STAGED_PLATFORM_DIR"
  NPM_CONFIG_CACHE="$WORK_DIR/npm-cache" npm pack --pack-destination "$WORK_DIR" >/dev/null
)
(
  cd "$REPO_ROOT/npm/fleet"
  NPM_CONFIG_CACHE="$WORK_DIR/npm-cache" npm pack --pack-destination "$WORK_DIR" >/dev/null
)

WRAPPER_TARBALL="$(find "$WORK_DIR" -name 'sponzey-fleet-*.tgz' ! -name '*darwin*' ! -name '*linux*' -print -quit)"
PLATFORM_TARBALL="$(find "$WORK_DIR" -name "sponzey-fleet-$PLATFORM_OS-$PLATFORM_ARCH-*.tgz" -print -quit)"

if [ -z "$WRAPPER_TARBALL" ] || [ -z "$PLATFORM_TARBALL" ]; then
  echo "missing wrapper or platform tarball" >&2
  exit 1
fi

INSTALL_SCOPE="$WORK_DIR/prefix/lib/node_modules/@sponzey"
mkdir -p "$INSTALL_SCOPE/fleet" "$INSTALL_SCOPE/fleet-$PLATFORM_OS-$PLATFORM_ARCH" "$WORK_DIR/prefix/bin"
tar -xzf "$WRAPPER_TARBALL" -C "$INSTALL_SCOPE/fleet" --strip-components 1
tar -xzf "$PLATFORM_TARBALL" -C "$INSTALL_SCOPE/fleet-$PLATFORM_OS-$PLATFORM_ARCH" --strip-components 1
ln -s "../lib/node_modules/@sponzey/fleet/bin/sponzey" "$WORK_DIR/prefix/bin/sponzey"

SPONZEY_FLEET_NPM_OS="$PLATFORM_OS" \
SPONZEY_FLEET_NPM_ARCH="$PLATFORM_ARCH" \
  "$WORK_DIR/prefix/bin/sponzey" --help >/dev/null

echo "npm platform local install smoke ok: $PLATFORM_OS-$PLATFORM_ARCH"
