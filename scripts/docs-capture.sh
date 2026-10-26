#!/usr/bin/env bash
# Regenerate the command output that the user guide quotes.
#
# Every line of `docs/user-guide/_meta/captures.txt` is `name|max-lines|command`.
# The command runs in <workdir>, where the demo captures have the short names the guide uses
# (checkout.har, baseline.har, sanitized.har, quirks/, config/, proj/). `wiretrail` in a command
# is the binary you built, wherever it appears (in a pipe, after `cd`, ...), and any other
# tool (jq, grep) runs as written. The output lands in
# `docs/user-guide/_meta/outputs/<name>.txt`, and `scripts/docs-check.py` checks that the
# pages quote it faithfully. See `_meta/outputs/README.md`.
#
# usage: scripts/docs-capture.sh <workdir> [name-filter]
#   WIRETRAIL=/path/to/wiretrail   binary to run (default: target/release/wiretrail)
set -uo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
work=${1:?usage: scripts/docs-capture.sh <workdir> [name-filter]}
only=${2:-}
bin=${WIRETRAIL:-$root/target/release/wiretrail}
list=$root/docs/user-guide/_meta/captures.txt
out=$root/docs/user-guide/_meta/outputs

[ -x "$bin" ] || { echo "wiretrail binary not found: $bin (run cargo build --release)" >&2; exit 2; }
[ -d "$work" ] || { echo "workdir not found: $work (run scripts/docs-workdir.sh $work)" >&2; exit 2; }
mkdir -p "$out"

# A `wiretrail` on the PATH that is the binary under test, so a capture reads like the page.
shim=$(mktemp -d)
trap 'rm -rf "$shim"' EXIT
ln -s "$bin" "$shim/wiretrail"
export PATH="$shim:$PATH" NO_COLOR=1

failed=0
while IFS='|' read -r name max cmd; do
  case "$name" in ''|'#'*) continue ;; esac
  if [ -n "$only" ] && [[ "$name" != *"$only"* ]]; then continue; fi
  tmp=$(mktemp)
  ( cd "$work" && eval "$cmd" ) >"$tmp" 2>&1
  rc=$?
  {
    printf '$ %s\n# exit %s\n' "$cmd" "$rc"
    if [ "${max:-0}" -gt 0 ]; then head -n "$max" "$tmp"; else cat "$tmp"; fi
  } >"$out/$name.txt"
  rm -f "$tmp"
  printf '%-34s exit %s\n' "$name" "$rc"
done <"$list"
exit $failed
