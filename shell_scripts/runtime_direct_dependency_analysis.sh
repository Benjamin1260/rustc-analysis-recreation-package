#!/usr/bin/env bash
set -uo pipefail
trap 'echo "Error on line $LINENO" >&2' ERR

OUT="source_use_analysis_results.csv"

LABELS=("tokio" "smol" "async-std" "futures")
PATTERNS=("tokio" "smol" "async_std" "futures")

printf 'project,%s\n' "$(IFS=,; echo "${LABELS[*]}")" > "$OUT"

for dir in */; do
  dir_name=${dir%/}
  results=()

  for pattern in "${PATTERNS[@]}"; do
    if find "$dir" -type f -name "*.rs" \
      -exec grep -hE "^[[:space:]]*use[[:space:]]+${pattern}([[:space:]]*::|[[:space:]]*;|[[:space:]]*\\{)" {} + \
      2>/dev/null | grep -q .
    then
      results+=(1)
    else
      results+=(0)
    fi
  done

  printf "%s,%s\n" "$dir_name" "$(IFS=,; echo "${results[*]}")" >> "$OUT"
done
