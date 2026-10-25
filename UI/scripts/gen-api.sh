#!/usr/bin/env bash
set -euo pipefail

# 既存の呼び出しとの互換性のため引数を受け付ける。接続先は実行時に設定する。
if [[ $# -gt 1 || ( $# -eq 1 && "$1" != development && "$1" != production ) ]]; then
  echo "Usage: $0 [development|production]" >&2
  exit 1
fi
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR/.."
SCHEMA_COMMIT=e2dc2baecf60dc51c0b4ae383f99f75d12855de6
npx --no-install openapi-generator-cli generate \
  -i "https://raw.githubusercontent.com/traP-jp/traO-Judge-docs/$SCHEMA_COMMIT/api/backend/frontend-backend.yaml" \
  -g typescript-fetch \
  -o src/api/generated

bash "$SCRIPT_DIR/gen-api-types.sh"
