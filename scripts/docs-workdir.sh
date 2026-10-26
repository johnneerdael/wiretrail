#!/usr/bin/env bash
# Build the work directory that scripts/docs-capture.sh runs in.
#
# The guide's examples use short file names. This links them to the committed files, so a
# fresh clone can regenerate every capture:
#
#   checkout.har, baseline.har, sanitized.har   docs/user-guide/assets/downloads/
#   quirks/                                      docs/user-guide/assets/downloads/quirks/
#   config/, proj/wiretrail.yaml                 docs/user-guide/_meta/fixtures/
#   empty.har, truncated.har, not-a-har.json     made here, for the error captures
#
# usage: scripts/docs-workdir.sh <workdir>
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
work=${1:?usage: scripts/docs-workdir.sh <workdir>}
dl=$root/docs/user-guide/assets/downloads
fx=$root/docs/user-guide/_meta/fixtures

mkdir -p "$work/proj"
for f in checkout.har baseline.har sanitized.har; do
  [ -f "$dl/$f" ] || { echo "missing $dl/$f (run python3 scripts/docs-make-demo-hars.py)" >&2; exit 1; }
  ln -sfn "$dl/$f" "$work/$f"
done
ln -sfn "$dl/quirks" "$work/quirks"
ln -sfn "$fx/config" "$work/config"
ln -sfn "$fx/config/wiretrail.yaml" "$work/proj/wiretrail.yaml"

# Inputs for the error captures: a file with no bytes, the first 3,000 bytes of a real capture
# (which ends in the middle of a record), and valid JSON that is not a HAR.
: >"$work/empty.har"
head -c 3000 "$dl/checkout.har" >"$work/truncated.har"
cp "$fx/not-a-har.json" "$work/not-a-har.json"
echo "work directory ready: $work"
