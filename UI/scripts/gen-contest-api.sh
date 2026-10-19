#!/bin/bash
set -eu

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
UI_DIR="$SCRIPT_DIR/.."
cd "$UI_DIR"

# 既存画面は旧契約を使うため，新契約の生成物を分けて段階的に移行する．
# getContestと共通エラーが参照するモデルだけを生成対象に含める．
npx --no-install @openapitools/openapi-generator-cli generate \
  -i "$UI_DIR/../docs/API/front_back.yaml" \
  -g typescript-fetch \
  -o "$UI_DIR/src/api/generated/contest" \
  --openapi-normalizer 'FILTER=operationId:getContest' \
  --global-property 'apis=Contests,models=getContest_200_response:getContest_200_response_rules:requestEmailRegistration_403_response:requestEmailRegistration_403_response_details_inner,supportingFiles=runtime.ts:index.ts'
