#!/bin/bash
set -eu
case "${1:-development}" in
  development|production) ;;
  *) echo "Usage: $0 [development|production]" >&2; exit 1 ;;
esac
cd "$(dirname "$0")/.."
SPEC=$(mktemp)
trap 'rm -f "$SPEC"' EXIT
node scripts/openapi.mjs "$SPEC"
# 分割定義を束ね，共通スキーマの名前を生成型に使う．
# 古い生成物を残すと旧APIのimportが通るため，毎回生成先を作り直す．
rm -rf src/api/generated
npx --no-install @openapitools/openapi-generator-cli generate \
  -i "$SPEC" -g typescript-fetch -o src/api/generated \
  --additional-properties=useSingleRequestParameter=true \
  --global-property 'apis,models,supportingFiles=runtime.ts:index.ts'
