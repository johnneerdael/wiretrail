# Storms

`storms` finds bursts: many calls to one host, or one endpoint, within a short window. A burst can
overload a server, trip a rate limit, or show a client that fires one request per item.

```bash
wiretrail checkout.har storms
```

<!-- output: storm-default.txt from="host api.shop.example" until="host cdn.shop.example" -->
```text
host api.shop.example  13 calls in 1.0s (13.0/s)
  window: 3.8s - 4.8s
  entries: e000024, e000025, e000026, e000027, e000028, e000029, e000030, e000031, e000032, e000033, e000034, e000035, e000036

endpoint api.shop.example/v1/products/{id}/price  12 calls in 1.0s (12.0/s)
  window: 4.2s - 4.9s
  entries: e000026, e000027, e000028, e000029, e000030, e000031, e000032, e000033, e000034, e000035, e000036, e000037
```

## How a storm is found

For each host, and separately for each endpoint, WireTrail slides a window over the requests in time
order and keeps the window that holds the most. If that count reaches the minimum, it is reported once for that scope.

| Flag | Default | Meaning |
|---|---|---|
| `--window-ms` | `1000` | The width of the window, in milliseconds. |
| `--min-count` | `5` | The fewest calls in the window that count as a storm. |

| Part | Meaning |
|---|---|
| `host` or `endpoint` | The scope. An endpoint is a method-less host and [normalized path](../concepts/entries-ids-endpoints.md). |
| `13 calls in 1.0s` | The most requests that fit in one window of that width. |
| `(13.0/s)` | The count divided by the window, as a rate. |
| `window` | The time of the first and the last call in the busiest window. |
| `entries` | The calls in it. |

The window is measured from request **start** times. Only the densest window per scope is reported, so a
host with two separate bursts shows the larger one.

A host burst and an endpoint burst can overlap. Here the 13 calls to the host and the 12 calls to one endpoint share 11 calls. The host window also catches
two list calls shortly before the burst, and the last call to the endpoint falls shortly after it. The endpoint is the cause and the host is a consequence, so read the endpoint line first.

## Tune it

A narrower window finds sharper bursts:

```bash
wiretrail checkout.har storms --window-ms 300 --min-count 5
```

<!-- output: storm-window.txt from="host cdn.shop.example" until="next useful" -->
```text
host cdn.shop.example  8 calls in 300ms (26.7/s)
  window: 330ms - 450ms
  entries: e000002, e000003, e000004, e000005, e000006, e000007, e000008, e000009

host api.shop.example  6 calls in 300ms (20.0/s)
  window: 4.2s - 4.5s
  entries: e000026, e000027, e000028, e000029, e000030, e000031

endpoint api.shop.example/v1/products/{id}/price  6 calls in 300ms (20.0/s)
  window: 4.2s - 4.5s
  entries: e000026, e000027, e000028, e000029, e000030, e000031
```

The CDN host serves eight files in 120 ms at page load. That is a normal burst: a browser fetching a
page's assets. A storm is a prompt to look, not a fault.

Raise `--min-count` to see only the large ones. With 50, nothing in this capture qualifies:

```bash
wiretrail checkout.har storms --min-count 50
```

<!-- output: storm-none.txt from="== wiretrail storms" until="next useful" -->
```text
== wiretrail storms ==
```

## The ranked finding uses fixed numbers

`summary`, `diagnose` and `auto` raise a **MEDIUM** `request-storm` finding for a burst of **10 or more**
calls in a one-second window. They use their own fixed settings, so `--window-ms` and `--min-count` change what the
`storms` command prints and nothing else.

## JSON

```bash
wiretrail checkout.har storms --top 1 --json | jq '.result.storms[0] | del(.entry_ids)'
```

<!-- output: storm-json.txt -->
```text
{
  "scope_kind": "host",
  "scope": "api.shop.example",
  "peak_count": 13,
  "window_ms": 1000,
  "first_offset_ms": 3800.0,
  "last_offset_ms": 4800.0,
  "calls_per_sec": 13.0
}
```

## Exit code

`storms` exits `1` when it reports at least one storm and `0` when it reports none.

## Related

- [Pagination and N+1](pagination.md) for a burst caused by one request per item.
- [Spot an N+1](recipes/spot-an-n-plus-one.md) works through the burst in this capture.
- [Timeline](../analyze/timeline.md) shows the burst in order.
