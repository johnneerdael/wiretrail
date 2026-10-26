# Auto

`auto` is the command for "analyze this capture for me". It prints the summary, then runs the
follow-up analysis for each serious finding and prints that too. You read one report instead of
running six commands. Two kinds of finding name a scope, a 5xx cluster (its host) and repeated
identical calls (their host and route), and their follow-up runs on that part only. The other
follow-ups run on the whole capture.

```bash
wiretrail checkout.har auto
```

## How the output is laid out

First comes the full [summary](summary.md), ending in the ranked recommendations. Then, for every
recommendation at or above the severity floor, a block like this:

<!-- output: auto-default.txt from="[HIGH] retry-exhaustion" until="GET api.shop.example/v1/inventory" -->
```text
[HIGH] retry-exhaustion — 4 retries, final 500 on POST /v1/cart/sync
$ wiretrail checkout.har retries
== wiretrail retries ==

POST api.shop.example/v1/cart/sync  (5 attempts, 4 retries, final 500)
  triggered by: 503, 500
  backoff gaps: 1.2s, 2.5s, 5.2s, 8.2s
  entries: e000069, e000070, e000072, e000073, e000076
```

The first line is the finding. The `$` line is the command the finding recommends. Below it is that
command's normal output. If you gave `auto` a `--filter` or a `--top`, the drill-down ran with them, but
the `$` line does not show them: add them yourself when you run the command again.

`auto` drills into eight commands: `retries`, `errors`, `rate-limit`, `auth`, `storms`, `diff`,
`redirects` and `slowest`. Those are the only commands a recommendation can name.

## What gets drilled, and what does not

By default `auto` drills findings that are medium severity or higher, and lists the rest as one-line
suggestions. Two flags move the line:

```bash
wiretrail checkout.har auto --min-severity high   # only HIGH (no rule produces CRITICAL)
wiretrail checkout.har auto --all                 # everything, including LOW
```

With `--min-severity high`, the medium findings are listed under a heading at the end instead of being
run:

<!-- output: auto-high.txt from="not drilled" -->
```text
not drilled (below threshold):
  [MEDIUM] request-storm — 13 host calls/s burst to api.shop.example   (run: wiretrail checkout.har storms)
  [MEDIUM] request-storm — 12 endpoint calls/s burst to api.shop.example/v1/products/{id}/price   (run: wiretrail checkout.har storms)
  [MEDIUM] wasteful-duplicates — 12x identical GET /v1/products/{id}/price   (run: wiretrail checkout.har diff --filter "host:api.shop.example path:/v1/products/*/price")
  [MEDIUM] wasteful-duplicates — 12x identical POST /v1/session/ping   (run: wiretrail checkout.har diff --filter "host:api.shop.example path:/v1/session/ping")
  [MEDIUM] 4xx — 4x 429 on GET /v1/suggest   (run: wiretrail checkout.har errors)
  [MEDIUM] auth-failures — 4 auth failures (401/403)   (run: wiretrail checkout.har auth)
```

!!! note "The default list stops at ten"
    `--top` bounds the ranked list, and its default is 10. On this capture the ten highest findings
    are four HIGH and six MEDIUM, so the two LOW findings (a redirect storm and a slow call) never
    appear. Add `--top 20`, as in `wiretrail checkout.har auto --all --top 20`, to see them. The
    checks behind the list are also cut to `--top` groups before they are ranked, so a capture with
    many error groups can hide a serious one: see
    [Limitations](../reference/limitations.md#-top-cuts-each-check-before-the-findings-are-ranked).

Two recommendations can point at the same command. Here the host burst and the endpoint burst both run
`storms`, so the same block is printed twice. That is expected: each recommendation is drilled on its own.

## Scope the whole run

`--filter` applies to the summary and to every drill-down, even though the `$` lines do not repeat it.
This runs `auto` on the search host only:

```bash
wiretrail checkout.har auto --filter "host:search.shop.example"
```

<!-- output: auto-filter.txt from="recommended next steps" through="too many requests" -->
```text
recommended next steps:
  [HIGH] rate-limit
         calls during 429 cooldown on search.shop.example /v1/suggest — 4 429s, follow-ups before Retry-After elapsed
  [MEDIUM] errors
         4x 429 on GET /v1/suggest — too many requests
```

## JSON

With `--json`, the result is one document with the summary, the drill-downs that ran and the
suggestions that did not:

```bash
wiretrail checkout.har auto --json | jq '{drilldowns: [.result.drilldowns[] | {severity, kind, command, filter}], not_drilled: [.result.not_drilled[].kind]}'
```

<!-- output: auto-json.txt keep -->
```text
{
  "drilldowns": [
    {
      "severity": "high",
      "kind": "retry-exhaustion",
      "command": "retries",
      "filter": null
    },
    {
      "severity": "high",
      "kind": "5xx-cluster",
      "command": "errors",
      "filter": "host:api.shop.example"
    },
    ...
  ],
  "not_drilled": []
}
```

Each drill-down also carries a `result` object with that command's JSON. See [JSON schemas](../reference/json-schemas.md).

## Exit code

`auto` exits `1` when there is at least one recommendation, whatever the severity floor, and `0` when
there is none. On the known-good capture it is clean:

```bash
wiretrail baseline.har auto > /dev/null; echo "exit code: $?"
```

<!-- output: auto-clean-exit.txt -->
```text
exit code: 0
```

`baseline.har` has two error responses and some repeated calls, and `auto` still exits `0`, because
nothing in it reached the level of a finding. Use that for a [CI gate](../automate/ci-gates.md).

## Related

- [Diagnose](diagnose.md) prints the findings without running the drill-downs.
- [How findings are ranked](../concepts/how-findings-are-ranked.md) explains the severities.
- [Triage an unfamiliar capture](recipes/triage-an-unfamiliar-capture.md) is the longer procedure.
