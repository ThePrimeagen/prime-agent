#!/usr/bin/env sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
BIN_DIR="${HOME}/.local/bin"
BIN_NAME="prime-agent"

cd "$ROOT_DIR"

echo "Building ${BIN_NAME}..."
cargo build --release

mkdir -p "$BIN_DIR"
install -m 755 "$ROOT_DIR/target/release/${BIN_NAME}" "$BIN_DIR/${BIN_NAME}"

echo "Installed ${BIN_NAME} to ${BIN_DIR}/${BIN_NAME}"
