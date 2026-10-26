# Gate a build

A gate runs a command in a pipeline and fails the build when the command exits non-zero. WireTrail's exit codes are
designed for this: `0` is clean, `1` is findings, `2` is a file it could not read.

## Choose what to gate on

| Gate | Fails when | Exit code `1` means |
|---|---|---|
| `compare new.har baseline.har --fail-on high` | A new 5xx error appears | A new server error compared with the baseline |
| `diagnose` | Any ranked finding exists | At least one finding of any severity |
| `auto` | Any ranked finding exists | The same as `diagnose` |
| `rules --config wiretrail.yaml` | A rule is broken | At least one rule finding |
| `validate` | The capture looks broken | At least one anomaly |

`compare` is the best first gate. It judges a **change**, so an existing flaw does not fail every build. `diagnose` and `auto`
judge the capture on its own and fail on anything, so they suit a flow that should be clean.

## Compare with a baseline

On the known-good capture, compared with itself, the check passes:

```bash
wiretrail baseline.har compare baseline.har --fail-on high; echo "exit code: $?"
```

<!-- output: ci-compare-pass.txt from="== wiretrail compare" -->
```text
== wiretrail compare ==
max severity: none

next useful commands: diagnose · errors · slowest
exit code: 0
```

On the new capture it fails, because of a new 5xx on the cart sync:

```bash
wiretrail checkout.har compare baseline.har --fail-on high > /dev/null; echo "exit code: $?"
```

<!-- output: cmp-exit-high.txt -->
```text
exit code: 1
```

[Compare against a baseline](../compare/compare.md) explains the levels, and why `--fail-on critical` never fails.

## Fail on a ranked finding

`diagnose` exits `1` when it prints any finding:

```bash
wiretrail baseline.har diagnose --top 20; echo "exit code: $?"
```

<!-- output: ci-diagnose-pass.txt from="== wiretrail diagnose" -->
```text
== wiretrail diagnose ==

no notable findings

next useful commands: errors · auth · duplicates
exit code: 0
```

To fail only on serious findings, count the `high` ones in the JSON:

```bash
wiretrail checkout.har diagnose --top 20 --json | jq '[.result.findings[] | select(.severity == "high")] | length'
```

<!-- output: ci-high-count.txt from="4" -->
```text
4
```

`jq -e` sets its exit code from the last value, so the same expression can be the gate:

```bash
wiretrail checkout.har diagnose --top 20 --json | jq -e '[.result.findings[] | select(.severity == "high")] | length == 0' > /dev/null; echo "exit code: $?"
```

<!-- output: ci-jq-gate.txt from="exit code" -->
```text
exit code: 1
```

`--top 20` matters. A `--top` that is too small can hide findings of any severity, including high ones: each check is cut to `--top` groups before the findings are ranked, and then the
list is cut again. Use a `--top` larger than the number of error groups you expect. [Limitations](../reference/limitations.md#-top-cuts-each-check-before-the-findings-are-ranked) has an example.

## More than one gate

A pipeline can require several checks and report each:

```bash
wiretrail checkout.har compare baseline.har --fail-on high > /dev/null; a=$?; \
wiretrail checkout.har diagnose --top 20 --json | jq -e '[.result.findings[] | select(.severity == "high")] | length == 0' > /dev/null; b=$?; \
echo "compare=$a diagnose=$b"
```

<!-- output: ci-two-gates.txt from="compare=" -->
```text
compare=1 diagnose=1
```

Fail the build if either is non-zero.

## Check your own rules

```bash
wiretrail checkout.har rules --config config/wiretrail.yaml --top 2 > /dev/null; echo "exit code: $?"
```

<!-- output: ci-rules-fail.txt from="exit code" -->
```text
exit code: 1
```

## An example pipeline

This GitHub Actions job installs the release binary and gates a pull request on a regression. It assumes the baseline and the new capture are
in the repository:

```yaml
name: Network regression check
on: pull_request

jobs:
  har:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install WireTrail
        run: |
          curl -fsSL https://github.com/johnneerdael/wiretrail/releases/latest/download/wiretrail-x86_64-unknown-linux-gnu.tar.gz | tar -xz
          sudo mv wiretrail /usr/local/bin/
      - name: Compare with the baseline
        run: wiretrail captures/new.har compare captures/baseline.har --fail-on high
```

Producing `captures/new.har` is the part that depends on your project: a browser test that saves a HAR, or a proxy
that records a test run. WireTrail only reads the result.

## A capture that holds secrets

A capture from a test run can hold credentials. Keep it out of build logs: WireTrail's output masks what it recognizes, and
you should still not echo the capture itself. See [Handle captures safely](../capture/handle-captures-safely.md).

## Related

- [Compare against a baseline](../compare/compare.md) has every threshold.
- [Catch a regression before it ships](../compare/recipes/catch-a-regression.md) is the routine around the gate.
- [Errors and exit codes](../reference/errors-and-exit-codes.md) lists the codes of every command.
