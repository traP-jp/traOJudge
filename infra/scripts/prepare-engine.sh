#!/usr/bin/env bash
set -euo pipefail
source /workspace/tooling/env.sh
cd "$(dirname "$0")/../.."
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 cargo build --locked -p traojudge-infra-probe
install -m 755 target/debug/traojudge-infra-probe infra/runtime/engine
strip infra/runtime/engine
