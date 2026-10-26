# Diagnose

`diagnose` prints the ranked findings and nothing else. Each finding has a severity, a short title,
the reason, the entries that prove it, and the command that drills in. Use it when you want the list
without the summary around it, or when a script should read the findings.

```bash
wiretrail checkout.har diagnose
```

## Read a finding

<!-- output: dx-default.txt from="[HIGH] token-refresh-race" until="[MEDIUM] request-storm" -->
```text
[HIGH] token-refresh-race — suspicious token refresh on auth.shop.example
  refresh succeeded but later calls reused the old token
  evidence: e000014, e000015
  -> wiretrail <file> auth
```

| Part | Meaning |
|---|---|
| `[HIGH]` | The severity: `HIGH`, `MEDIUM` or `LOW`. Findings are sorted by severity. `CRITICAL` exists as a level, and no rule produces it. |
| `token-refresh-race` | The kind of finding. [How findings are ranked](../concepts/how-findings-are-ranked.md) lists all ten and the rule behind each. |
| `suspicious token refresh on auth.shop.example` | The title, with the numbers and names the rule found. |
| `refresh succeeded but later calls reused the old token` | Why it was raised. |
| `evidence:` | Entry ids, up to eight. Pass one to [`show-entry`](../inspect/show-entry.md). |
| `-> wiretrail <file> auth` | The command that investigates it. `<file>` stands for your capture. A `--filter` is included for a 5xx cluster (its host) and for repeated identical calls (their host and route), and for no other kind. |

A severity is a rule's judgement of how likely the pattern is to matter. It is not proof of a root
cause. `token-refresh-race` says the evidence fits a race, and [Debug a token refresh race](../failures/recipes/debug-a-token-refresh-race.md)
shows how to confirm it.

## The list stops at ten

`--top` bounds the ranked list, and its default is 10. The list is sorted by severity before it is cut,
so the findings that fall off here are the least severe. `--top 20` shows the rest:

```bash
wiretrail checkout.har diagnose --top 20
```

<!-- output: dx-top20.txt from="[MEDIUM] 4xx — 1x 401 on GET /v1/notifications" -->
```text
[MEDIUM] 4xx — 1x 401 on GET /v1/notifications
  JWT expired
  evidence: e000015
  -> wiretrail <file> errors

[LOW] redirect-storm — 6x [302] redirect on cdn.shop.example /img/hero.png
  repeated redirects
  evidence: e000007, e000020, e000039, e000053, e000067, e000074
  -> wiretrail <file> redirects

[LOW] slow-backend — slowest call 2409ms on api.shop.example /v1/orders
  dominated by server wait (TTFB)
  evidence: e000090
  -> wiretrail <file> slowest

next useful commands: errors · auth · duplicates
```

!!! warning "`--top` also cuts each check before it ranks"
    Behind `diagnose` are the checks that `errors`, `retries`, `storms` and the others run, and each
    one is asked for only its first `--top` groups, largest first. A capture with many small 4xx
    groups can use up that allowance before a smaller 5xx cluster is seen, and then the list has no
    `HIGH` finding to rank. [Limitations](../reference/limitations.md#-top-cuts-each-check-before-the-findings-are-ranked)
    has a capture that does this.

## Diagnose part of a capture

`--filter` limits the evidence the rules see, so the findings describe only that subset:

```bash
wiretrail checkout.har diagnose --filter "host:search.shop.example"
```

<!-- output: dx-filter.txt from="[HIGH]" until="next useful" -->
```text
[HIGH] rate-limit-no-backoff — calls during 429 cooldown on search.shop.example /v1/suggest
  4 429s, follow-ups before Retry-After elapsed
  evidence: e000077, e000078, e000079, e000080
  -> wiretrail <file> rate-limit

[MEDIUM] 4xx — 4x 429 on GET /v1/suggest
  too many requests
  evidence: e000077, e000078, e000079, e000080
  -> wiretrail <file> errors
```

## A clean capture

On a capture with no findings, `diagnose` says so and exits `0`:

```bash
wiretrail baseline.har diagnose --top 20
```

<!-- output: dx-clean.txt from="== wiretrail diagnose" until="next useful" -->
```text
== wiretrail diagnose ==

no notable findings
```

That is a statement about the rules, not a clean bill of health. A capture can have errors that no
rule turns into a finding, such as a single 500. Check [`errors`](../failures/errors.md) as well when it matters.

## JSON

```bash
wiretrail checkout.har diagnose --top 3 --json | jq '.result.findings[0]'
```

<!-- output: dx-json.txt -->
```text
{
  "severity": "high",
  "kind": "retry-exhaustion",
  "title": "4 retries, final 500 on POST /v1/cart/sync",
  "detail": "repeated retries did not recover",
  "evidence_ids": [
    "e000069",
    "e000070",
    "e000072",
    "e000073",
    "e000076"
  ],
  "suggested_command": "retries"
}
```

`suggested_command` is the command with its filter, ready to run after the file name.

## Exit code

`diagnose` exits `1` when it prints at least one finding and `0` when there are none.

## Related

- [Auto](auto.md) runs the suggested commands for you.
- [How findings are ranked](../concepts/how-findings-are-ranked.md) has the rule behind each kind.
- [Gate a build](../automate/ci-gates.md) uses the exit code.
