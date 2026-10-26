set -eu
if cmp -s "$ACTUAL" "$EXPECTED"; then
    printf '{"verdict":"accepted"}\n'
else
    printf '{"verdict":"wrong_answer"}\n'
fi
