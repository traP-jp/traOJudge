#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo build -p traojudge-judge
mkdir -p deploy/judge/.build
cp target/debug/traojudge-judge deploy/judge/.build/traojudge-judge
strip deploy/judge/.build/traojudge-judge
