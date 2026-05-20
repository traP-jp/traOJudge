#!/usr/bin/env bash
# PostToolUse hook: runs Prettier + ESLint --fix on UI files after Edit/Write/MultiEdit.
# Exit 2 (with stderr) sends ESLint feedback back to the model so it can repair issues.
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
  */UI/dist/*|*/UI/node_modules/*) exit 0 ;;
esac

case "$FILE_PATH" in
  *.ts|*.tsx|*.vue|*.js|*.mjs|*.cjs) ;;
  *) exit 0 ;;
esac

# Mark this session as having touched UI files so the Stop hook runs vue-tsc.
touch "/tmp/claude-ui-dirty.${SESSION_ID}"

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"
UI_DIR="$PROJECT_DIR/UI"

cd "$UI_DIR" || exit 0

npx --no-install prettier --write --log-level=warn "$FILE_PATH" >&2 2>&1 || true

ESLINT_OUT="$(npx --no-install eslint --fix "$FILE_PATH" 2>&1)"
ESLINT_EXIT=$?

if [ $ESLINT_EXIT -ne 0 ]; then
  {
    echo "ESLint reported unfixable issues in $FILE_PATH. Please address them:"
    echo "$ESLINT_OUT"
  } >&2
  exit 2
fi

exit 0
