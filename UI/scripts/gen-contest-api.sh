#!/bin/bash
set -eu

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
UI_DIR="$SCRIPT_DIR/.."
cd "$UI_DIR"

# 既存画面は旧契約を使うため，新契約の生成物を分けて段階的に移行する．
# 大会と解説の閲覧に必要な操作だけを新契約から生成する．
npx --no-install @openapitools/openapi-generator-cli generate \
  -i "$UI_DIR/../docs/API/front_back.yaml" \
  -g typescript-fetch \
  -o "$UI_DIR/src/api/generated/contest" \
  --openapi-normalizer 'FILTER=operationId:getContest\|listContestProblems\|getProblem\|listProblemEditorials\|getEditorial\|listMyRoles' \
  --global-property 'apis=Contests:Problems:Editorials:Me,models=getContest_200_response:getContest_200_response_rules:listContestProblems_200_response:listContestProblems_200_response_items_inner:listMyProblems_200_response_items_inner:listProblemEditorials_200_response:listProblemEditorials_200_response_items_inner:listProblems_200_response_nextCursor:getEditorial_200_response:listMyRoles_200_response:getMe_200_response_roles_inner:getMe_200_response_roles_inner_scope:getMe_200_response_roles_inner_scope_oneOf:getMe_200_response_roles_inner_scope_oneOf_1:getMe_200_response_roles_inner_scope_oneOf_2:getMe_200_response_roles_inner_scope_oneOf_3:getMe_200_response_roles_inner_scope_oneOf_4:requestEmailRegistration_403_response:requestEmailRegistration_403_response_details_inner,supportingFiles=runtime.ts:index.ts'
