#!/usr/bin/env bash
# 編集直後はlint-stagedと同じ順序で修正し、失敗をモデルへ返す。
set -uo pipefail

INPUT="$(cat)"
FILE_PATH="$(printf '%s' "$INPUT" | jq -r '.tool_input.file_path // empty')"
SESSION_ID="$(printf '%s' "$INPUT" | jq -r '.session_id // "default"')"

[ -z "$FILE_PATH" ] && exit 0

case "$FILE_PATH" in
  */UI/*) ;;
  *) exit 0 ;;
esac

case "$FILE_PATH" in
  */UI/src/api/generated/*) exit 0 ;;
  */UI/dist/*|*/UI/build/*|*/UI/coverage/*|*/UI/node_modules/*) exit 0 ;;
esac

case "$FILE_PATH" in
  *.ts|*.tsx|*.vue|*.js|*.mjs|*.cjs) ;;
  *) exit 0 ;;
esac

# Mark this session as having touched UI files so the Stop hook runs vue-tsc.
touch "/tmp/claude-ui-dirty.${SESSION_ID}"

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"
UI_DIR="$PROJECT_DIR/UI"

cd "$UI_DIR" || exit 2

CHECK_EXIT=0
if ! ./node_modules/.bin/eslint --fix --max-warnings=0 "$FILE_PATH" >&2; then
  CHECK_EXIT=2
fi
if ! ./node_modules/.bin/prettier --write --log-level=warn "$FILE_PATH" >&2; then
  CHECK_EXIT=2
fi

exit "$CHECK_EXIT"
