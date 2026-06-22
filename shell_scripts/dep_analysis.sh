#!/usr/bin/env bash
set -euo pipefail
trap 'echo "Error on line $LINENO" >&2' ERR

OUT="dep_analysis_results.csv"
KEYWORDS=("tokio" "smol" "async_std" "futures")

printf 'project,%s\n' "$(IFS=,; echo "${KEYWORDS[*]}")" > "$OUT"

for dir in */; do
  dir_name=${dir%/}
  KEYWORD_COUNTS=()

  for KEYWORD in "${KEYWORDS[@]}"; do
    KEYWORD_COUNT=$(
      find "$dir" -name "*.rs" -exec grep -hEo "^use ${KEYWORD}(::)?" {} + 2>/dev/null |
      wc -l
    ) || KEYWORD_COUNT=0

    KEYWORD_COUNTS+=("$KEYWORD_COUNT")
  done

  printf "%s,%s\n" "$dir_name" "$(IFS=,; echo "${KEYWORD_COUNTS[*]}")" >> "$OUT"
done


# Note to self:
# cargo metadata includes external dependencies
# Cargo tree too
# These are hard to remove since internal crates also might use runtimes (being valid)
# Hard to define internal - external crate boundary
# But, all source code in repo is generally repo's own
# so using pattern matching on 'use RUNTIME::' is valid