#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR/.."

npx --no-install openapi-typescript ../docs/API/front_back.yaml \
  --output src/api/generated/schema.ts
