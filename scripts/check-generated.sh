#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Freshness gate: regenerate every derived artifact from db.h and fail if the committed files
# drift. Turns "remember to re-run the generator after an OpenROAD SHA bump" into an enforced
# invariant. Run locally before pushing, or in CI (see .github/workflows/generated-freshness.yml).
#
# Touches TWO repos (the generator writes the safe Db API + registry into the sibling opendb crate):
#   <ws>/vyges-tools-opendb-lib   (this repo — bindings, bridges, resolvers, coverage map)
#   <ws>/vyges-tools-opendb       (sibling — generated_api.rs, generated_write_api.rs, generated_registry.rs)
set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
opendb="$(cd "$here/../vyges-tools-opendb" && pwd)"

# db.h must be present (the pinned OpenROAD odb subtree); fetch it if a fresh checkout.
if [ ! -f "$here/vendor/OpenROAD/src/odb/include/odb/db.h" ]; then
  echo "== fetching pinned OpenROAD odb subtree (for db.h) =="
  "$here/scripts/fetch-odb-src.sh"
fi

echo "== regenerating bindings + coverage maps =="
python3 "$here/scripts/generate-bindings.py"
# Two coverage maps, deliberately. The CORE one is the headline metric and its denominator is
# stable by construction (odb's hand-written classes only), so coverage is comparable release to
# release. The FULL one adds the ~75 classes that have an upstream codeGenerator schema — which
# is where the 3D/chiplet surface lives — so anything we bind outside the core is still visible.
python3 "$here/scripts/derive-schema.py"
python3 "$here/scripts/derive-schema.py" --all --out docs/derived-full-schema.json

fail=0
for repo in "$here" "$opendb"; do
  if ! git -C "$repo" diff --quiet; then
    echo "ERROR: generated files in $(basename "$repo") are STALE:" >&2
    git -C "$repo" --no-pager diff --stat >&2
    fail=1
  fi
done

if [ "$fail" != 0 ]; then
  cat >&2 <<'EOF'

The committed generated files do not match a fresh run of the generator.
Fix: run  scripts/generate-bindings.py  (and derive-schema.py), then commit the result
in BOTH vyges-tools-opendb-lib and vyges-tools-opendb.
EOF
  exit 1
fi
echo "OK: all generated files are up to date with db.h."
