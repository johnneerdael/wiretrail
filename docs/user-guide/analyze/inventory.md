# Hosts, subsystems and endpoints

These three commands answer "what is in this capture?" at three levels of detail. `hosts` counts per
server, `subsystems` groups servers into the integrations or teams that own them, and `endpoints`
lists the routes that were called.

## Hosts

```bash
wiretrail checkout.har hosts --filter "status:>=400"
```

<!-- output: hosts-filter.txt from="api.shop.example" until="next useful" -->
```text
api.shop.example  (10 req, 10 err, 7 dup)
  latency p50/p95/max: 184ms / 243ms / 243ms
  bytes sent/received: 240 B / 689 B
  methods: GET:5 POST:5
  status: 4xx:4 5xx:6

search.shop.example  (4 req, 4 err, 0 dup)
  latency p50/p95/max: 29ms / 33ms / 33ms
  bytes sent/received: 0 B / 252 B
  methods: GET:4
  status: 4xx:4

payments.shop.example  (1 req, 1 err, 0 dup)
  latency p50/p95/max: 201ms / 201ms / 201ms
  bytes sent/received: 49 B / 70 B
  methods: POST:1
  status: 4xx:1
```

The filter kept only the failing requests, so every request counted here is an error. Without it, the
same command lists the busiest hosts first, up to `--top` of them (10 unless you change it).

| Part | Meaning |
|---|---|
| `10 req` | Requests to the host. |
| `10 err` | Requests that were a 4xx, a 5xx or had no status. |
| `7 dup` | Requests that belong to a [duplicate group](../repeats/duplicates.md). This counts every request in each group, including the first. |
| `p50/p95/max` | The median, the 95th percentile and the longest duration of the host's requests. Percentiles use the nearest-rank method. |
| `bytes sent/received` | Request body bytes, and response content bytes. |
| `methods`, `status` | Counts by HTTP method and by status class. |

## Subsystems

`subsystems` groups hosts under a name. WireTrail picks the name in this order: the first rule in your
[configuration file](../compare/config-file.md) that matches, then a built-in vendor name, then the host itself.

```bash
wiretrail checkout.har subsystems
```

<!-- output: sub-default.txt from="Sentry" until="payments.shop.example" -->
```text
Sentry  [-]  (3 req, 0 err, 3 dup)
  window: 2.5s - 40.0s
  hosts: o0.ingest.sentry.io
```

Only one host here is a known vendor: `o0.ingest.sentry.io` becomes `Sentry`. Every other host shows
under its own name, and `[-]` means no owner is set. The built-in vendors are GitHub, Supabase, Google,
TMDB, Cloudflare, AWS and Sentry. `window` is the time of the first and the last request.

Name your own with a configuration file. `--config` points at one:

```bash
wiretrail checkout.har subsystems --config config/ownership-only.yaml
```

<!-- output: sub-config.txt from="Storefront API" until="cdn.shop.example" -->
```text
Storefront API  [web-platform]  (63 req, 10 err, 53 dup)
  window: 900ms - 69.8s
  hosts: api.shop.example
```

If you do not pass `--config`, WireTrail uses a file named `wiretrail.yaml` in the folder you run it
from. In a folder that has one, the same command picks it up on its own:

```bash
cd proj && wiretrail ../checkout.har subsystems
```

<!-- output: sub-auto.txt from="Search" until="Payments" -->
```text
Search  [discovery]  (12 req, 4 err, 2 dup)
  window: 9.0s - 58.0s
  hosts: search.shop.example
```

## Endpoints

An endpoint is a method, a host and a [normalized path](../concepts/entries-ids-endpoints.md), so
`/v1/products/1042/price` and `/v1/products/1043/price` are one endpoint, `/v1/products/{id}/price`.

```bash
wiretrail checkout.har endpoints --top 6
```

<!-- output: end-top.txt from="12  GET api.shop.example" until="next useful" -->
```text
  12  GET api.shop.example/v1/products/{id}/price
  status: 200:12
  content-types: application/json

  12  POST api.shop.example/v1/session/ping
  status: 200:12
  content-types: application/json

  12  GET search.shop.example/v1/suggest
  status: 200:8 429:4
  content-types: application/json
  query keys: q

   6  GET api.shop.example/v1/flags
  status: 200:6
  content-types: application/json
  query keys: cb

   6  GET cdn.shop.example/img/hero.png
  status: 302:6
  content-types: image/png

   5  POST api.shop.example/v1/cart/sync
  status: 500:1 503:4
  content-types: application/json
```

Each endpoint shows its request count, the statuses it returned, its content types and the names
of the query parameters it used. Query values are not listed. Endpoints are sorted by count.

Use `--filter "status:>=400"` to list only the endpoints that failed.

## Related

- [Entries, ids and endpoints](../concepts/entries-ids-endpoints.md) explains normalization.
- [Compare against a baseline](../compare/compare.md) uses the same endpoint grouping.
- [The configuration file](../compare/config-file.md) describes `ownership`.
