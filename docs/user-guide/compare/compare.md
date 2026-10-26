# Compare against a baseline

`compare` takes a capture and a baseline and reports what is worse in the capture: endpoints that now return
errors, endpoints that got slower, and responses that grew. Each finding has a severity, and the exit code can fail a
build at the level you choose.

```bash
wiretrail checkout.har compare baseline.har
```

The capture you are checking comes **first**, and the baseline is the argument to `compare`.

<!-- output: cmp-default.txt from="== wiretrail compare" until="next useful" -->
```text
== wiretrail compare ==
max severity: high
new hosts: auth.shop.example, www.shop.example
new endpoints: 4
removed endpoints: 1

new errors:
  [high] POST api.shop.example/v1/cart/sync -> 503 (5x)
  [medium] GET search.shop.example/v1/suggest -> 429 (4x)
  [medium] GET api.shop.example/v1/cart -> 401 (2x)
  [medium] GET api.shop.example/v1/auth/session -> 401 (1x)
  [medium] GET api.shop.example/v1/notifications -> 401 (1x)
  [medium] POST payments.shop.example/v1/charges -> 402 (1x)

latency regressions:
  [medium] POST api.shop.example/v1/orders p50 319ms -> 2409ms
  [medium] GET cdn.shop.example/img/hero.webp p50 541ms -> 1921ms
  [medium] POST api.shop.example/v1/uploads/avatar p50 294ms -> 1104ms
  [medium] OPTIONS payments.shop.example/v1/charges p50 277ms -> 887ms
  [medium] GET api.shop.example/v1/products p50 128ms -> 558ms

payload growth:
  [low] GET api.shop.example/v1/config 3391B -> 11291B
  [low] POST api.shop.example/v1/cart/sync 11B -> 83B
```

## What it compares

Both captures are reduced to **endpoints**: a method, a host and a normalized path. For each endpoint WireTrail keeps the median
duration (p50), the median response size, and the error statuses. `--filter` applies to both captures.

| Line | Meaning |
|---|---|
| `new hosts`, `removed hosts` | Hosts present only in the capture, or only in the baseline. |
| `new endpoints`, `removed endpoints` | Endpoints present only in one. Only a **count** is shown. |
| `new errors` | An endpoint with a 4xx or 5xx in the capture and **none** in the baseline. The status shown is the highest, and the count is all the errors on it. |
| `latency regressions` | The median time is **more than double** the baseline's and **more than 200 ms** slower. |
| `payload growth` | The median response size is **more than double** the baseline's. |

### Severity

| Finding | Severity |
|---|---|
| New 5xx on an endpoint | `high` |
| New 4xx on an endpoint | `medium` |
| Latency regression | `medium` |
| Payload growth | `low` |

`max severity` is the highest severity among the findings: `none`, `low`, `medium` or `high`. New and removed hosts and endpoints
are information. They never raise the severity by themselves. `critical` is accepted by `--fail-on`, but `compare` never produces it.

## Fail a build

By default `compare` exits `1` if there is any finding and `0` if there is none. `--fail-on` raises the bar: the exit code is `1` only when the maximum severity
**reaches** the level you name.

```bash
wiretrail checkout.har compare baseline.har --fail-on high > /dev/null; echo "exit code: $?"
```

<!-- output: cmp-exit-high.txt -->
```text
exit code: 1
```

| `--fail-on` | Fails when the worst finding is |
|---|---|
| `low` | any finding |
| `medium` | `medium` or `high` |
| `high` | `high` |
| `critical` | never: `compare` has no `critical` finding |

`--fail-on critical` therefore always exits `0`:

```bash
wiretrail checkout.har compare baseline.har --fail-on critical > /dev/null; echo "exit code: $?"
```

<!-- output: cmp-exit-critical.txt -->
```text
exit code: 0
```

A capture compared with itself is clean:

```bash
wiretrail baseline.har compare baseline.har
```

<!-- output: cmp-self.txt from="max severity" until="next useful" -->
```text
max severity: none
```

## Read the full lists in JSON

The text output shows only counts of new and removed endpoints. `--json` has the lists:

```bash
wiretrail checkout.har compare baseline.har --json | jq '.result | {max_severity, new_hosts, removed_hosts, new_endpoints, removed_endpoints}'
```

<!-- output: cmp-json.txt -->
```text
{
  "max_severity": "high",
  "new_hosts": [
    "auth.shop.example",
    "www.shop.example"
  ],
  "removed_hosts": [],
  "new_endpoints": [
    "GET api.shop.example/v1/products/{id}/price",
    "GET cdn.shop.example/img/hero.png",
    "GET www.shop.example/",
    "POST auth.shop.example/oauth/token"
  ],
  "removed_endpoints": [
    "POST api.shop.example/v1/prices"
  ]
}
```

Here the app now makes one request per product price where it made one batched `POST /v1/prices`, and it
signs in through `auth.shop.example`, which the baseline never needed.

## `--top` also cuts the counts

Every list is limited to `--top` entries, and the counts shown for endpoints are the lengths of the **cut** lists:

```bash
wiretrail checkout.har compare baseline.har --top 2
```

<!-- output: cmp-top.txt from="new hosts" until="new errors" -->
```text
new hosts: auth.shop.example, www.shop.example
new endpoints: 2
removed endpoints: 1
```

There are four new endpoints, and the text shows `2`. Use the JSON, or leave `--top` at its default of 10 or higher, if you need the true counts.

## What `compare` does not see

- **How many calls.** The heartbeat that went from 3 calls to 12 is the same endpoint, with the same median time and size.
  `compare` reports nothing for it:

    ```bash
    wiretrail checkout.har compare baseline.har --filter "path:/v1/session/ping"
    ```

    <!-- output: cmp-ping.txt from="max severity" until="next useful" -->
    ```text
    max severity: none
    ```

    Use [`diagnose`](../analyze/diagnose.md) on the new capture for repeated calls, N+1 and storms.
- **Errors that were already there.** If the baseline had any 4xx or 5xx on an endpoint, more errors on it are not "new".
- **Failures with no status.** Only 4xx and 5xx count as errors here.
- **Endpoints that appear only in the new capture.** A new endpoint is counted, and its errors do appear under `new errors`, but its speed
  has nothing to be compared with.
- **The tail.** The median can stay the same while the slowest calls get much worse.

A baseline that cannot be read stops the command with exit code `2` and a message that starts `baseline:`:

```bash
wiretrail checkout.har compare nope.har
```

<!-- output: cmp-missing.txt from="wiretrail:" -->
```text
wiretrail: baseline: failed to read HAR file
```

## Exit code

`0` when the check passes, `1` when it fails, `2` when either file cannot be read.

## Related

- [Catch a regression before it ships](recipes/catch-a-regression.md) is the full routine.
- [Gate a build](../automate/ci-gates.md) runs it in a pipeline.
- [Entries, ids and endpoints](../concepts/entries-ids-endpoints.md) explains how endpoints are formed.
