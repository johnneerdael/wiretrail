# Rate limits

`rate-limit` finds responses where the server said "slow down", and checks whether the client did.
It reads the `Retry-After` header and the `X-RateLimit-*` headers, and flags calls that arrived
while the server's cooldown was still running.

```bash
wiretrail checkout.har rate-limit
```

<!-- output: rl-default.txt from="search.shop.example" until="next useful" -->
```text
search.shop.example /v1/suggest  (4x 429) [cooldown violated]
  retry-after: 5s, 5s, 5s, 5s
  x-ratelimit-limit: 30
  x-ratelimit-remaining: 0
  x-ratelimit-reset: 1792487693
  called during cooldown: e000078, e000079, e000080
```

## What counts as limited

A response is limited when its status is **429**, or when its `X-RateLimit-Remaining` header is `0`.
The second rule catches a 200 that used up the quota. Responses are grouped by host and normalized
path.

| Part | Meaning |
|---|---|
| `4x 429` | Responses with status 429 in the group. |
| `[cooldown violated]` | Another call to the same host and path **started** while a limited response's `Retry-After` period was still running. |
| `retry-after` | The `Retry-After` value of each limited response, in seconds. |
| `x-ratelimit-*` | The first such headers seen on a limited response. |
| `called during cooldown` | The ids of the calls that started inside a cooldown. |

## See what the client did

The timeline of the search host shows the pattern around the limit:

```bash
wiretrail checkout.har timeline --filter "host:search.shop.example" --top 30
```

<!-- output: rl-timeline.txt from="46.0s" until="next useful" -->
```text
   46.0s     25ms  e000077 GET search.shop.example/v1/suggest  [429] 63 B DUP
   46.8s     30ms  e000078 GET search.shop.example/v1/suggest  [429] 63 B
   48.1s     33ms  e000079 GET search.shop.example/v1/suggest  [429] 63 B
   52.2s     29ms  e000080 GET search.shop.example/v1/suggest  [429] 63 B
   58.0s     38ms  e000081 GET search.shop.example/v1/suggest  [200] 35 B RETRY
```

The first 429 arrived at 46.0 s with `Retry-After: 5`. The client called again at 46.8 s, 48.1 s and
52.2 s, all inside a cooldown, and each got another 429. Only the call at 58.0 s succeeded. `transitions`
puts names on those steps:

```bash
wiretrail checkout.har transitions --filter "host:search.shop.example"
```

<!-- output: rl-transitions.txt from="429 -> 429" until="next useful" -->
```text
429 -> 429  [rate-limit-persisted]  GET search.shop.example/v1/suggest
  e000077 -> e000078  (gap 800ms)

429 -> 429  [rate-limit-persisted]  GET search.shop.example/v1/suggest
  e000078 -> e000079  (gap 1.3s)

429 -> 429  [rate-limit-persisted]  GET search.shop.example/v1/suggest
  e000079 -> e000080  (gap 4.1s)

429 -> 200  [rate-limit-recovered]  GET search.shop.example/v1/suggest
  e000080 -> e000081  (gap 5.8s)
```

`rate-limit-persisted` is a 429 followed by another 429, and `rate-limit-recovered` is a 429 followed by a 2xx.

## The ranked finding

A violated cooldown becomes a **HIGH** `rate-limit-no-backoff` finding in `summary`, `diagnose` and
`auto`. The 429 responses also appear as a **MEDIUM** `4xx` finding with the server's message. See
[Errors](../failures/errors.md) for that view.

## Limits of the check

!!! warning "`Retry-After` is read as a number of seconds only"
    The header can also hold an HTTP date, such as `Tue, 20 Oct 2026 09:14:32 GMT`. WireTrail does not
    read that form. On `quirks/retry-after-date.har` the response is listed with no `retry-after` line
    and no cooldown check:

    ```bash
    wiretrail quirks/retry-after-date.har rate-limit
    ```

    <!-- output: rl-date.txt from="api.test.example" until="next useful" -->
    ```text
    api.test.example /v1/x  (1x 429)
    ```

    A server that sends dates gets no `[cooldown violated]`, whatever the client did.

The cooldown check looks only at calls to the **same host and path**. A client that obeys the limit on that path but
calls a different path of the same service is not flagged. The cooldown is counted from when the limited request
*started*, not from when its response arrived.

## JSON

```bash
wiretrail checkout.har rate-limit --json | jq '.result.groups[0]'
```

<!-- output: rl-json.txt -->
```text
{
  "host": "search.shop.example",
  "norm_path": "/v1/suggest",
  "count_429": 4,
  "retry_after_secs": [
    5.0,
    5.0,
    5.0,
    5.0
  ],
  "ratelimit_headers": {
    "x-ratelimit-limit": "30",
    "x-ratelimit-remaining": "0",
    "x-ratelimit-reset": "1792487693"
  },
  "cooldown_violated": true,
  "violating_ids": [
    "e000078",
    "e000079",
    "e000080"
  ],
  "entry_ids": [
    "e000077",
    "e000078",
    "e000079",
    "e000080"
  ]
}
```

## Exit code

`rate-limit` exits `1` when it finds any limited response and `0` when it finds none.

## Related

- [Errors](../failures/errors.md) shows the body the server sent with each 429.
- [Status transitions](../failures/transitions.md) names the 429 patterns.
- [Retries](retries.md) shows how the same calls look as retries.
