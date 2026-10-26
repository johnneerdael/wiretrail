# Retries

`retries` lists calls that were repeated after a failed attempt, with the status that triggered each
retry and the gaps between attempts. It answers "did the app retry, did it back off, and did it ever
recover?".

```bash
wiretrail checkout.har retries
```

<!-- output: ret-default.txt from="POST api.shop.example/v1/cart/sync" until="GET api.shop.example/v1/inventory" -->
```text
POST api.shop.example/v1/cart/sync  (5 attempts, 4 retries, final 500)
  triggered by: 503, 500
  backoff gaps: 1.2s, 2.5s, 5.2s, 8.2s
  entries: e000069, e000070, e000072, e000073, e000076
```

## How a retry is defined

WireTrail groups requests with the same [fingerprint](duplicates.md#what-a-group-is). In a group, it
looks for an attempt that **failed with a retryable status**: any 5xx, a 429, or no status at all.
Every later request in the group counts as a retry.

| Part | Meaning |
|---|---|
| `5 attempts` | Requests in the group. |
| `4 retries` | Attempts after the first failure. |
| `final 500` | The status of the last attempt. A 2xx means the call recovered. |
| `triggered by` | The distinct failure statuses seen. `0` means no status. |
| `backoff gaps` | The time between the starts of consecutive attempts. |

Read the gaps to judge the client. Gaps that grow, as here, show back-off. Gaps that stay equal, or
shrink, show a client that hammers a failing server.

!!! note "Only 5xx, 429 and no status trigger a retry"
    A 4xx other than 429 does not. A request that gets a 401 and is repeated with a new token is not
    reported here. On `unauthorized-twice.har`, where one call returns 401, 401 and then 200, `retries`
    prints nothing:

    ```bash
    wiretrail quirks/unauthorized-twice.har retries
    ```

    <!-- output: ret-401.txt from="== wiretrail retries" until="next useful" -->
    ```text
    == wiretrail retries ==
    ```

    The same three calls do appear in [`duplicates`](duplicates.md), without the `[retry pattern]` tag.
    Use [`transitions`](../failures/transitions.md) to find a 401 that was followed by a success.

## Retries that recovered

`retries` lists every group with a retry, recovered or not. These three recovered:

<!-- output: ret-default.txt from="GET api.shop.example/v1/inventory" -->
```text
GET api.shop.example/v1/inventory/{id}  (2 attempts, 1 retries, final 200)
  triggered by: 502
  backoff gaps: 800ms
  entries: e000054, e000056

GET cdn.shop.example/img/banner.webp  (2 attempts, 1 retries, final 200)
  triggered by: 0
  backoff gaps: 4.5s
  entries: e000059, e000064

GET search.shop.example/v1/suggest  (2 attempts, 1 retries, final 200)
  triggered by: 429
  backoff gaps: 12.0s
  entries: e000077, e000081

next useful commands: errors · transitions · show-entry
```

The stock call failed once and succeeded 800 ms later, which is a healthy retry. The image failed with no status, then
loaded 4.5 seconds later. The search call got a 429 and succeeded with the same query 12 seconds later, but three other
calls went out in between and [ignored the rate limit](rate-limits.md). `retries` shows that the attempts have the same
shape, not whether the app meant them as a retry, and it does not see what happened between them.

`timeline` marks the same requests `RETRY`:

```bash
wiretrail checkout.har timeline --filter "path:/img/banner.webp"
```

<!-- output: ret-timeline.txt from="15.0s" until="next useful" -->
```text
   15.0s     1.9s  e000059 GET cdn.shop.example/img/banner.webp  [0] 0 B DUP
   19.5s     71ms  e000064 GET cdn.shop.example/img/banner.webp  [200] 60.0 KiB RETRY
```

## The retry finding

`summary`, `diagnose` and `auto` raise a **HIGH** `retry-exhaustion` finding for a group with three or more
retries whose last status is not a 2xx. The cart sync above is one. A group with fewer retries, or one that recovered,
is listed here and raises no finding.

## JSON

```bash
wiretrail checkout.har retries --top 1 --json | jq '.result.groups[0] | {attempts, retry_count, trigger_statuses, gaps_ms, final_status}'
```

<!-- output: ret-json.txt -->
```text
{
  "attempts": 5,
  "retry_count": 4,
  "trigger_statuses": [
    503,
    500
  ],
  "gaps_ms": [
    1200.0,
    2500.0,
    5200.0,
    8200.0
  ],
  "final_status": 500
}
```

## Exit code

`retries` exits `1` when it lists at least one group and `0` when there are none.

## Related

- [Duplicates](duplicates.md) lists repeats whether or not a failure came first.
- [Status transitions](../failures/transitions.md) shows what a failed call turned into.
- [Find the first failure](../failures/recipes/find-the-first-failure.md) starts from the earliest error.
