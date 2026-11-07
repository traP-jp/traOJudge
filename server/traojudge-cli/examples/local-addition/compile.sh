set -eu
cc -x c "$TRAOJUDGE_SUBMISSION_SOURCE" -o outputs/binary
printf '{}\n'
