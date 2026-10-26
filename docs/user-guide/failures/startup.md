# Startup profile

`startup` looks at the first seconds of a capture, which is when an app or a page loads, and answers
three questions: how many requests started, how many ran at once, and how long was the chain of calls
that ran one after another. It is the command for "why is it slow to start?".

```bash
wiretrail checkout.har startup --top 8
```

<!-- output: start-default.txt from="73 requests" until="next useful" -->
```text
73 requests in 30.0s · max concurrency 5 · critical path 5.6s

critical path (sequential spine):
      21ms  e000000 GET www.shop.example/  [301]
     261ms  e000001 GET shop.example/  [200]
     199ms  e000003 GET cdn.shop.example/assets/vendor.9c2d41ab.js  [200]
     214ms  e000010 GET api.shop.example/v1/config  [200]
      35ms  e000013 GET api.shop.example/v1/cart  [401]
     407ms  e000014 POST auth.shop.example/oauth/token  [200]
      53ms  e000017 GET api.shop.example/v1/cart  [200]
      97ms  e000019 GET api.shop.example/v1/flags  [200]

slowest in window:
      1.9s  e000009 GET cdn.shop.example/img/hero.webp
      1.9s  e000059 GET cdn.shop.example/img/banner.webp
      1.0s  e000032 GET api.shop.example/v1/products/{id}/price
     803ms  e000021 POST o0.ingest.sentry.io/api/{id}/envelope/
     560ms  e000022 GET api.shop.example/v1/products
     558ms  e000024 GET api.shop.example/v1/products
     558ms  e000025 GET api.shop.example/v1/products
     553ms  e000023 GET api.shop.example/v1/products
```

## What it prints

The first line has three figures:

| Figure | Meaning |
|---|---|
| `73 requests in 30.0s` | Requests that **started** within the window, and the window. |
| `max concurrency 5` | The most requests in flight at the same instant. |
| `critical path 5.6s` | The total duration of the whole sequential chain. Here the chain has 45 calls, and the list below shows only its first `--top`, so the rows add up to less than the total. |

`critical path (sequential spine)` lists a chain of calls, one after another. `slowest in window` lists the
longest requests in the window, with no phases. Both lists show `--top` rows.

!!! note "The critical path is an approximation"
    A HAR does not record which request caused which, so WireTrail cannot know the real critical path. It builds
    a chain greedily: it walks the requests in start order and keeps each one that starts after the previous
    kept request **ended**. The chain is a run of calls that did not overlap, not a dependency chain. Read it as
    "this much of the window was spent waiting on calls that ran in sequence", and check the calls yourself.

## Choose the window

`--window-ms` is the length of the window from the first request, and it defaults to 30000. `0` means the
whole capture:

```bash
wiretrail checkout.har startup --window-ms 5000 --top 5
```

<!-- output: start-5s.txt from="38 requests" until="next useful" -->
```text
38 requests in 5.0s · max concurrency 5 · critical path 3.5s

critical path (sequential spine):
      21ms  e000000 GET www.shop.example/  [301]
     261ms  e000001 GET shop.example/  [200]
     199ms  e000003 GET cdn.shop.example/assets/vendor.9c2d41ab.js  [200]
     214ms  e000010 GET api.shop.example/v1/config  [200]
      35ms  e000013 GET api.shop.example/v1/cart  [401]

slowest in window:
      1.9s  e000009 GET cdn.shop.example/img/hero.webp
      1.0s  e000032 GET api.shop.example/v1/products/{id}/price
     803ms  e000021 POST o0.ingest.sentry.io/api/{id}/envelope/
     560ms  e000022 GET api.shop.example/v1/products
     558ms  e000024 GET api.shop.example/v1/products
```

In the first five seconds, 38 requests started and the sequential chain, 14 calls long, took 3.5 seconds. The list shows only its first five. The slowest
calls in the window are an image download, a queued price call and an error-report connection. A shorter window
isolates the page load from everything after it.

```bash
wiretrail checkout.har startup --window-ms 0 --top 3
```

<!-- output: start-whole.txt from="99 requests" until="next useful" -->
```text
99 requests in 69.8s · max concurrency 5 · critical path 10.3s

critical path (sequential spine):
      21ms  e000000 GET www.shop.example/  [301]
     261ms  e000001 GET shop.example/  [200]
     199ms  e000003 GET cdn.shop.example/assets/vendor.9c2d41ab.js  [200]

slowest in window:
      2.4s  e000090 POST api.shop.example/v1/orders
      1.9s  e000009 GET cdn.shop.example/img/hero.webp
      1.9s  e000059 GET cdn.shop.example/img/banner.webp
```

## Read it with other commands

- A long chain made of calls to different hosts is a **serialization** to look at: each call waited for the one
  before it. Which could have started earlier?
- A high `max concurrency` with a few slow calls says the load is parallel and one call is the cost.
  [Slowest calls](slowest.md) says where those calls lost their time.
- A burst at the start is [Storms](../repeats/storms.md) at work.

## JSON

```bash
wiretrail checkout.har startup --window-ms 5000 --top 2 --json | jq '.result | {window_ms, requests_in_window, max_concurrency, critical_path_ms}'
```

<!-- output: start-json.txt -->
```text
{
  "window_ms": 5000.0,
  "requests_in_window": 38,
  "max_concurrency": 5,
  "critical_path_ms": 3465.0
}
```

The result also holds `critical_path` and `slowest`, each a list of calls with id, method, host, path, offset,
duration and status.

## Exit code

`startup` always exits `0`.

## Related

- [Slowest calls](slowest.md) breaks a slow request into phases.
- [Timeline](../analyze/timeline.md) shows the same calls in order.
- [Why a call is slow](recipes/why-a-call-is-slow.md) follows one slow call.
