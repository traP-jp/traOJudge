#!/usr/bin/env bash

mkdir gen

export TRAO_DOCS_API_VERSION="API-2.0.0"
export BACKEND_TO_JUDGE_API="https://raw.githubusercontent.com/traP-jp/traO-Judge-docs/refs/tags/${TRAO_DOCS_API_VERSION}/api/backend/frontend-backend.yaml"

# Download the API schema
curl "${BACKEND_TO_JUDGE_API}" > gen/backend-api.yaml

# Generate the API server crate
docker run --rm \
  -v ${PWD}:/local openapitools/openapi-generator-cli generate \
  -i /local/gen/backend-api.yaml \
  -g rust-axum \
  -o /local/gen \
  --additional-properties=packageName=gen \