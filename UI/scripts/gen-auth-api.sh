#!/bin/bash
set -eu
exec bash "$(dirname "$0")/gen-api.sh" "${1:-development}"
