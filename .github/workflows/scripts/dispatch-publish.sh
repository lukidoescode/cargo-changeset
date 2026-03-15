#!/usr/bin/env bash
# Dispatches publish (and for the CLI crate: dist + docker) workflows for each
# tag pointing at HEAD, in workspace dependency order. A 60-second pause is
# inserted between depth tiers so crates.io has time to index each tier before
# dependents are published.
set -euo pipefail

tags=$(git tag --points-at HEAD)

if [ -z "$tags" ]; then
  echo "No tags point at HEAD, nothing to dispatch."
  exit 0
fi

# Emit "<depth> <crate_name>" lines sorted by depth, derived from the workspace
# dependency graph. Depth 0 = no internal deps; higher = depends on lower tiers.
depth_order=$(cargo metadata --no-deps --format-version 1 | python3 -c "
import json, sys

data = json.load(sys.stdin)
pkgs = data['packages']

deps = {p['name']: [d['name'] for d in p['dependencies'] if d.get('path')] for p in pkgs}
names = [p['name'] for p in pkgs]

depths = {n: 0 for n in names}
for _ in range(len(names)):
    for n in names:
        for d in deps[n]:
            if depths[n] <= depths[d]:
                depths[n] = depths[d] + 1

for name in sorted(names, key=lambda n: depths[n]):
    print(depths[name], name)
")

current_depth=-1

while IFS=" " read -r depth crate; do
  tag=$(echo "$tags" | grep -E "^${crate}@v" || true)
  [ -z "$tag" ] && continue

  if [ "$current_depth" -ge 0 ] && [ "$depth" -gt "$current_depth" ]; then
    echo "Waiting 60s before depth-${depth} tier..."
    sleep 60
  fi
  current_depth=$depth

  echo "Dispatching publish for $tag (depth=$depth)"
  gh workflow run publish.yml --repo "$GITHUB_REPOSITORY" --ref "refs/tags/$tag"

  if echo "$tag" | grep -q "^cargo-changeset@v"; then
    gh workflow run dist.yml --repo "$GITHUB_REPOSITORY" --ref "refs/tags/$tag"
    gh workflow run docker.yml --repo "$GITHUB_REPOSITORY" --ref "refs/tags/$tag"
  fi
done <<< "$depth_order"
