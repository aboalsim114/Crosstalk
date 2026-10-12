#!/usr/bin/env bash
# One measurement of the CI `asr-bench` job: runs crosstalk-asr-bench with the
# given arguments, appends the report to the job summary and republishes the
# result lines as annotations (readable through the API).
set -uo pipefail
name="$1"; shift
bench=src-tauri/target/release/examples/asr_bench.exe
out="bench-$name.md"
start=$(date +%s)
"$bench" --threads 4 --markdown "$out" "$@"
code=$?
secs=$(( $(date +%s) - start ))
{ echo; echo "#### $name (${secs} s, code $code)"; cat "$out" 2>/dev/null; } >> "$GITHUB_STEP_SUMMARY"
grep -E '^\| ' "$out" 2>/dev/null | grep -vE '^\| (Modèle|---)' | while read -r line; do echo "::notice title=ASR $name::$line"; done
grep -E '^- `' "$out" 2>/dev/null | while read -r line; do echo "::notice title=ASR $name::$line"; done
echo "::notice title=ASR $name durée::${secs} s (code $code)"
exit $code
