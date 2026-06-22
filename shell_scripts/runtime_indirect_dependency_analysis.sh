#!/usr/bin/env bash
set -euo pipefail

declare -A counts=(
  [tokio]=0
  [futures]=0
  [async-std]=0
  [smol]=0
)

for repo in */; do
  tree=""

  while IFS= read -r -d '' manifest; do
    tree+=$'\n'
    tree+="$(cargo tree --manifest-path "$manifest" 2>/dev/null || true)"
  done < <(find "$repo" -name Cargo.toml -print0)

  for dep in tokio futures async-std smol; do
    if grep -qE "(^|[[:space:]])${dep} v" <<< "$tree"; then
      ((counts[$dep]+=1))
    fi
  done
done

for dep in tokio futures async-std smol; do
  echo "$dep: ${counts[$dep]}"
done
