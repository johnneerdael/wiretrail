# Timeline

`timeline` lists requests in the order they started, one per line, and marks the ones that repeat.
It is the quickest way to see what happened around a moment: what ran in parallel, what waited for
what, and where a failure sits among its neighbours.

```bash
wiretrail checkout.har timeline
```

<!-- output: tl-default.txt from="0ms" until="next useful" head="5" -->
```text
     0ms     21ms  e000000 GET www.shop.example/  [301] 0 B
    70ms    261ms  e000001 GET shop.example/  [200] 19.4 KiB
   330ms    126ms  e000002 GET cdn.shop.example/assets/app.5b1e0c7d.js  [200] 184.0 KiB
   335ms    199ms  e000003 GET cdn.shop.example/assets/vendor.9c2d41ab.js  [200] 412.0 KiB
   340ms     53ms  e000004 GET cdn.shop.example/assets/app.2e7f1a40.css  [200] 38.0 KiB
```

From left to right: the **offset** from the start of the capture, the **duration** of the request,
its entry id, method, host and normalized path, the status in brackets, the response size, and a marker
when there is one.

## Markers

| Marker | Meaning |
|---|---|
| `DUP` | The request belongs to a [duplicate group](../repeats/duplicates.md). |
| `RETRY` | An earlier request in the same group failed with a 5xx, a 429 or no status, so this one counts as a [retry](../repeats/retries.md). |

A `RETRY` takes priority over `DUP`. Filter to one endpoint to see the pattern:

```bash
wiretrail checkout.har timeline --filter "path:/v1/cart/sync"
```

<!-- output: tl-sync.txt from="25.0s" until="next useful" -->
```text
   25.0s    243ms  e000069 POST api.shop.example/v1/cart/sync  [503] 83 B DUP
   26.2s    210ms  e000070 POST api.shop.example/v1/cart/sync  [503] 83 B RETRY
   28.7s    184ms  e000072 POST api.shop.example/v1/cart/sync  [503] 83 B RETRY
   33.9s    234ms  e000073 POST api.shop.example/v1/cart/sync  [503] 83 B RETRY
   42.1s    192ms  e000076 POST api.shop.example/v1/cart/sync  [500] 69 B RETRY
```

The first attempt is `DUP` and every attempt after a failure is `RETRY`.

## It shows ten rows unless you ask for more

`--top` limits the number of rows, earliest first, and the default is 10. To page through a whole capture,
raise it: `wiretrail checkout.har timeline --top 500`. To see only the interesting part, filter instead of
paging. This lists the calls that took longer than 800 ms:

```bash
wiretrail checkout.har timeline --filter "time:>800" --top 20
```

<!-- output: tl-slow.txt from="450ms" until="next useful" -->
```text
   450ms     1.9s  e000009 GET cdn.shop.example/img/hero.webp  [200] 2.1 MiB
    2.5s    803ms  e000021 POST o0.ingest.sentry.io/api/{id}/envelope/  [200] 13 B
    4.6s     1.0s  e000032 GET api.shop.example/v1/products/{id}/price  [200] 20 B
   15.0s     1.9s  e000059 GET cdn.shop.example/img/banner.webp  [0] 0 B
   63.8s    887ms  e000086 OPTIONS payments.shop.example/v1/charges  [204] 0 B
   67.2s     2.4s  e000090 POST api.shop.example/v1/orders  [201] 36 B
   67.8s     1.1s  e000092 POST api.shop.example/v1/uploads/avatar  [201] 60 B
```

## JSON

Each row is an object with the same fields as the text, plus the first correlation id.

```bash
wiretrail checkout.har timeline --filter "path:/v1/cart/sync" --json | jq '.result.rows[0]'
```

<!-- output: tl-json.txt -->
```text
{
  "id": "e000069",
  "offset_ms": 25000.0,
  "duration_ms": 243.0,
  "method": "POST",
  "host": "api.shop.example",
  "norm_path": "/v1/cart/sync",
  "status": 503,
  "bytes": 83,
  "correlation_id": "req_fa6f840121",
  "marker": "DUP"
}
```

## Exit code

`timeline` always exits `0`.

## Related

- [Filter entries](filters.md) for the clauses that narrow a timeline.
- [Slowest calls](../failures/slowest.md) breaks a slow request into phases.
- [Find the first failure](../failures/recipes/find-the-first-failure.md) uses a filtered timeline.
