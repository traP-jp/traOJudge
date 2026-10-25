#!/bin/bash
set -eu

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
UI_DIR="$SCRIPT_DIR/.."
cd "$UI_DIR"

# 旧画面の移行が終わるまで，ログイン画面に必要な新契約の操作だけを分けて生成する．
npx --no-install @openapitools/openapi-generator-cli generate \
  -i "$UI_DIR/../docs/API/front_back.yaml" \
  -g typescript-fetch \
  -o "$UI_DIR/src/api/generated/auth" \
  --openapi-normalizer 'FILTER=operationId:verifyEmailAuthentication\|deleteSession\|getMe' \
  --global-property 'apis=Authentication:Me,models=getMe_200_response:getMe_200_response_roles_inner:getMe_200_response_roles_inner_scope:getMe_200_response_roles_inner_scope_oneOf:getMe_200_response_roles_inner_scope_oneOf_1:getMe_200_response_roles_inner_scope_oneOf_2:getMe_200_response_roles_inner_scope_oneOf_3:getMe_200_response_roles_inner_scope_oneOf_4:requestEmailRegistration_403_response:requestEmailRegistration_403_response_details_inner:verifyEmailAuthentication_200_response:verifyEmailAuthentication_200_response_oneOf:verifyEmailAuthentication_200_response_oneOf_1:verifyEmailAuthentication_200_response_oneOf_session:verifyEmailAuthentication_request,supportingFiles=runtime.ts:index.ts'
