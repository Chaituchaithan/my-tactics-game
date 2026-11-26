#!/usr/bin/env bash
# Builds the browser version and serves it at http://127.0.0.1:8080
#   ./scripts/serve-web.sh
set -euo pipefail
cd "$(dirname "$0")/.."
rustup target add wasm32-unknown-unknown >/dev/null
cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/emberfall.wasm web/
echo "Open http://127.0.0.1:8080"
cd web && python3 -m http.server 8080 --bind 127.0.0.1
