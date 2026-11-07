#!/bin/bash
set -eu
cd "$(dirname "$0")/.."
TASK_MOCK_ID=$(docker run --rm -d \
  --mount "type=bind,source=$(pwd)/src/api/mock/front-back-api-mock.json,target=/data/mock.json,readonly" \
  -p 127.0.0.1::4010 mockoon/cli:latest -d /data/mock.json --disable-admin-api)
trap 'docker stop --time 1 "$TASK_MOCK_ID" >/dev/null' EXIT
TASK_MOCK_ADDRESS=$(docker port "$TASK_MOCK_ID" 4010/tcp)
export MOCK_API_URL="http://${TASK_MOCK_ADDRESS}"
for attempt in {1..30}; do
  if curl --silent --fail "${MOCK_API_URL}/api/v1/languages" >/dev/null; then
    node scripts/check-mock-http.mjs
    exit 0
  fi
  sleep 1
done
docker logs "$TASK_MOCK_ID"
exit 1
