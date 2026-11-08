#!/usr/bin/env bash
# Always tear down this branch's stack, including after failed acceptance tests.
set -euo pipefail
cd "$(dirname "$0")/.."
JUDGE_WORK_DIR=${JUDGE_WORK_DIR:-$(mktemp -d /tmp/traojudge-aws-XXXXXX)}
mkdir -p "$JUDGE_WORK_DIR"
branch=$(git branch --show-current)
[ -n "$branch" ] || { echo 'Check out a branch first' >&2; exit 1; }
stack=$(python3 -c 'import re,sys;s=re.sub("[^A-Za-z0-9-]","-",sys.argv[1]);print((s if s[0].isalpha() else "Branch-"+s)+"-Judge")' "$branch")
cleanup() {
  original_status=$?
  trap - EXIT
  echo "Destroying ephemeral stack: $stack"
  if ! npx aws-cdk destroy "$stack" --force > "$JUDGE_WORK_DIR/destroy.log" 2>&1; then
    echo "Stack cleanup failed: $stack; inspect $JUDGE_WORK_DIR/destroy.log" >&2
    exit 1
  fi
  if aws cloudformation describe-stacks --region ap-northeast-1 --stack-name "$stack" > /dev/null 2> "$JUDGE_WORK_DIR/deleted-check.log"; then
    echo "Stack still exists after cleanup: $stack" >&2
    exit 1
  elif ! rg -q 'does not exist' "$JUDGE_WORK_DIR/deleted-check.log"; then
    echo "Could not verify stack deletion; inspect $JUDGE_WORK_DIR/deleted-check.log" >&2
    exit 1
  fi
  if [ "$original_status" -eq 0 ] && [ -f "$JUDGE_WORK_DIR/verification.json" ]; then
    "${JUDGE_PYTHON:-python3}" - "$JUDGE_WORK_DIR/verification.json" <<'JUDGE_PYTHON'
import json, sys
from pathlib import Path
path = Path(sys.argv[1])
report = json.loads(path.read_text())
report['stack_deleted'] = True
path.write_text(json.dumps(report, ensure_ascii=False, indent=2))
JUDGE_PYTHON
  fi
  exit "$original_status"
}
trap cleanup EXIT
scripts/prepare.sh
npm ci
npm run build
npx aws-cdk synth --quiet > "$JUDGE_WORK_DIR/synth.log" 2>&1
python3 -m unittest discover -s tests -v
# Existing stacks with this exact branch name are ours; never touch another prefix.
if aws cloudformation describe-stacks --region ap-northeast-1 --stack-name "$stack" > "$JUDGE_WORK_DIR/existing.json" 2> "$JUDGE_WORK_DIR/existing-error.log"; then
  npx aws-cdk destroy "$stack" --force
elif ! rg -q 'does not exist' "$JUDGE_WORK_DIR/existing-error.log"; then
  cat "$JUDGE_WORK_DIR/existing-error.log" >&2
  exit 1
fi
npx aws-cdk deploy "$stack" --require-approval never --outputs-file "$JUDGE_WORK_DIR/outputs.json" > "$JUDGE_WORK_DIR/deploy.log" 2>&1
"${JUDGE_PYTHON:-python3}" scripts/demo.py --stack "$stack" --report "$JUDGE_WORK_DIR/verification.json"
