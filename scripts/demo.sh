#!/usr/bin/env bash
# Build the live demo maps published at https://sylphxai.github.io/repomap/demo
# usage: scripts/demo.sh <repomap-binary> <out-dir>
set -euo pipefail
BIN=$(realpath "$1"); OUT=$(realpath -m "$2")
WORK=$(mktemp -d)
demo() { # name repo tag
  git clone -q --depth 1 --branch "$3" "https://github.com/$2" "$WORK/$1"
  mkdir -p "$OUT/$1"
  "$BIN" export "$WORK/$1" --out "$OUT/$1/index.html"
}
demo excalidraw excalidraw/excalidraw v0.18.0
demo axum tokio-rs/axum axum-v0.8.9
demo flask pallets/flask 3.1.2
ls -la "$OUT"/*/index.html
