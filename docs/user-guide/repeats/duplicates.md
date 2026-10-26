# Duplicates

`duplicates` lists requests that were made more than once. It groups them by a **fingerprint** and
tags the groups in which a failure was followed by another try. Use it to see what repeats, then decide
for each group whether the repeat is waste, a retry, or normal.

```bash
wiretrail checkout.har duplicates
```

<!-- output: dup-default.txt from="12x  GET" until="next useful" -->
```text
  12x  GET api.shop.example/v1/products/{id}/price
  statuses: 200:12
  entries: e000026, e000027, e000028, e000029, e000030, e000031, e000032, e000033, e000034, e000035, e000036, e000037

  12x  POST api.shop.example/v1/session/ping
  statuses: 200:12
  entries: e000038, e000041, e000042, e000050, e000052, e000055, e000058, e000060, e000062, e000063, e000065, e000068

   6x  GET api.shop.example/v1/flags
  statuses: 200:6
  entries: e000019, e000040, e000051, e000061, e000066, e000071

   6x  GET cdn.shop.example/img/hero.png
  statuses: 302:6
  entries: e000007, e000020, e000039, e000053, e000067, e000074

   5x [retry pattern]  POST api.shop.example/v1/cart/sync
  statuses: 500:1 503:4
  entries: e000069, e000070, e000072, e000073, e000076

   4x  GET api.shop.example/v1/cart
  statuses: 200:2 401:2
  entries: e000012, e000013, e000017, e000085

   3x  POST api.shop.example/graphql
  statuses: 200:3
  entries: e000082, e000083, e000084

   3x  POST api.shop.example/v1/track
  statuses: 200:3
  entries: e000093, e000094, e000095

   3x  POST o0.ingest.sentry.io/api/{id}/envelope/
  statuses: 200:3
  entries: e000021, e000057, e000075

   2x  GET api.shop.example/v1/auth/session
  statuses: 200:1 401:1
  entries: e000011, e000016
```

## What a group is

Two requests are in the same group when they have the same:

- HTTP method,
- host,
- [normalized path](../concepts/entries-ids-endpoints.md), where ids become `{id}`, and
- query string, **without** the cache-buster keys `_`, `cb`, `cachebuster`, `nonce`, `ts` and `timestamp`.

The body is not part of the fingerprint. Two POSTs to the same URL with different bodies are one
group. Use [`diff`](diff.md) to see whether the bodies differ.

Groups need at least two requests and are sorted by size. `--top` bounds the list, with a default of 10.

## Read a group

| Part | Meaning |
|---|---|
| `12x` | The number of requests in the group. |
| `[retry pattern]` | After a request failed with a 5xx, a 429 or no status, a later request in the group tried again. See [Retries](retries.md). |
| `statuses` | How the requests ended. `200:2 401:2` is two 200s and two 401s. |
| `entries` | The ids, in time order. |

## Not every duplicate is waste

A group says "these look alike", not "this is wrong". Each group in the capture needs a different reading:

| Group | What it is |
|---|---|
| `12x GET /v1/products/{id}/price` | Twelve different products, folded into one by normalization. It is an [N+1](pagination.md), and the fix is one request for all twelve. |
| `12x POST /v1/session/ping` | A heartbeat. The calls are identical except for a timestamp. See [Remove redundant calls](recipes/remove-redundant-calls.md). |
| `6x GET /v1/flags` | The same call with a different `cb` each time. The key is ignored, so they group. Caching would remove them. |
| `6x GET /img/hero.png` | The same redirect, six times. See [Redirects](../failures/redirects.md). |
| `5x [retry pattern] POST /v1/cart/sync` | A retry loop. See [Retries](retries.md). |
| `4x GET /v1/cart` | Two 401s and two 200s, over 70 seconds. Calls that happen minutes apart are usually legitimate. |
| `3x POST /graphql` | One endpoint with three different queries. A GraphQL or RPC endpoint always looks like a duplicate, because the body is not compared. |

Look at the spread in time as well as the count. Twelve calls in one second is a burst. Twelve calls
spread over a minute is a rhythm. `timeline` shows which, and `storms` finds bursts.

## Narrow it

```bash
wiretrail checkout.har duplicates --filter "host:api.shop.example method:post" --top 3
```

<!-- output: dup-filter.txt from="12x" until="next useful" -->
```text
  12x  POST api.shop.example/v1/session/ping
  statuses: 200:12
  entries: e000038, e000041, e000042, e000050, e000052, e000055, e000058, e000060, e000062, e000063, e000065, e000068

   5x [retry pattern]  POST api.shop.example/v1/cart/sync
  statuses: 500:1 503:4
  entries: e000069, e000070, e000072, e000073, e000076

   3x  POST api.shop.example/graphql
  statuses: 200:3
  entries: e000082, e000083, e000084
```

## JSON

```bash
wiretrail checkout.har duplicates --top 1 --json | jq '.result.groups[0] | {fingerprint, count, statuses, is_retry_pattern, first_offset_ms, last_offset_ms}'
```

<!-- output: dup-json.txt -->
```text
{
  "fingerprint": "GET api.shop.example /v1/products/{id}/price ",
  "count": 12,
  "statuses": {
    "200": 12
  },
  "is_retry_pattern": false,
  "first_offset_ms": 4200.0,
  "last_offset_ms": 4860.0
}
```

The `fingerprint` ends with the query pairs, so it ends in a space when there are none.

## Exit code

`duplicates` exits `1` when it finds at least one group and `0` when it finds none.

## Related

- [What varies between calls](diff.md) tells a pure duplicate from a call whose body changes.
- [Retries](retries.md) covers the groups tagged `[retry pattern]`.
- [Storms](storms.md) finds bursts.
