# Summary

`summary` is what WireTrail runs when you give it a capture and no command. It reads the file once and
reports what is in it: how many requests, how they ended, which hosts they went to, what repeated,
what was slow, and which commands to run next. It is the right first command on any capture.

```bash
wiretrail checkout.har
```

## What it prints

### 1. The size of the capture

<!-- output: sum-full.txt from="entries:" until="top hosts" -->
```text
entries: 99 total, 99 after filter
duration (first start to last response): 70.0s

status classes:
  2xx: 76
  3xx: 7
  4xx: 9
  5xx: 6
  other: 1

resource types:
  analytics: 3
  api: 77
  document: 3
  media: 9
  other: 3
  static: 4
```

- `entries` is the number of requests in the file and, after a [filter](filters.md), the number that
  matched. With no filter the two are equal.
- `duration` runs from the start of the first request to the end of the last response. It is the
  duration of the whole capture, even when a filter selects only a few entries.
- `status classes` counts responses by the first digit of the status. `other` means no usable
  status: the request failed before the server answered, which a HAR records as status `0`.
- `resource types` sorts each request into one group. A call to an analytics host goes to `analytics`
  first, whatever it returns. The hosts are the ones that end in `google-analytics.com`, `analytics.google.com`,
  `doubleclick.net`, `googletagmanager.com`, `segment.io`, `mixpanel.com`, `amplitude.com`, `sentry.io` or
  `crashlytics.com`. Every other call is sorted by its response content type, and then by the file extension
  in its URL. `api` is JSON, XML, GraphQL, gRPC and protobuf, `media` is images, video and audio, `static` is
  scripts, styles and fonts, and `document` is HTML. Anything else is `other`.

### 2. Hosts and repeated calls

<!-- output: sum-full.txt from="top hosts" until="slowest requests" -->
```text
top hosts (by request count):
     63  api.shop.example
     15  cdn.shop.example
     12  search.shop.example
      3  o0.ingest.sentry.io
      3  payments.shop.example
      1  auth.shop.example
      1  shop.example
      1  www.shop.example

top duplicate calls:
    12x  GET api.shop.example /v1/products/{id}/price   (e000026)
    12x  POST api.shop.example /v1/session/ping   (e000038)
     6x  GET api.shop.example /v1/flags   (e000019)
     6x  GET cdn.shop.example /img/hero.png   (e000007)
     5x  POST api.shop.example /v1/cart/sync   (e000069)
     4x  GET api.shop.example /v1/cart   (e000012)
     3x  POST api.shop.example /graphql   (e000082)
     3x  POST api.shop.example /v1/track   (e000093)
     3x  POST o0.ingest.sentry.io /api/{id}/envelope/   (e000021)
     2x  GET api.shop.example /v1/auth/session   (e000011)
```

A **duplicate call** is a group of two or more requests with the same method, host, normalized path
and query. Query keys that are only cache-busters (`_`, `cb`, `cachebuster`, `nonce`, `ts` and
`timestamp`) are ignored. The entry id in parentheses is the first request in the group. A group is
only a candidate: `12x POST /v1/session/ping` is a heartbeat that may be fine, and `5x POST
/v1/cart/sync` is a retry loop. [Duplicates](../repeats/duplicates.md) tells them apart.

### 3. Slow requests and large responses

<!-- output: sum-full.txt from="slowest requests" until="biggest payloads" head="6" -->
```text
slowest requests:
      2.4s  e000090 POST api.shop.example/v1/orders  [201]
      1.9s  e000009 GET cdn.shop.example/img/hero.webp  [200]
      1.9s  e000059 GET cdn.shop.example/img/banner.webp  [0]
      1.1s  e000092 POST api.shop.example/v1/uploads/avatar  [201]
      1.0s  e000032 GET api.shop.example/v1/products/{id}/price  [200]
```

`slowest requests` lists the longest calls with their status in brackets, `[0]` meaning no status.
`biggest payloads` lists the largest responses by size. Both lists have `--top` rows.
[Slowest calls](../failures/slowest.md) breaks each one into phases.

### 4. Hints and recommended next steps

<!-- output: sum-full.txt from="hints:" until="[MEDIUM] storms" -->
```text
hints:
  - 12x duplicate calls: GET api.shop.example /v1/products/{id}/price
  - 16 error responses (4xx/5xx/failed)

recommended next steps:
  [HIGH] retries
         4 retries, final 500 on POST /v1/cart/sync — repeated retries did not recover
  [HIGH] errors --filter "host:api.shop.example"
         4x 503 on POST /v1/cart/sync — inventory service unavailable
  [HIGH] rate-limit
         calls during 429 cooldown on search.shop.example /v1/suggest — 4 429s, follow-ups before Retry-After elapsed
  [HIGH] auth
         suspicious token refresh on auth.shop.example — refresh succeeded but later calls reused the old token
```

`hints` appears when a duplicate group has three or more calls, and when any request errored.
`recommended next steps` is the ranked list that `diagnose` and `auto` also print. Each entry has a
severity, the command to run with a scoping `--filter` where one applies, and the reason.

!!! note "`--top` also limits the recommendations"
    The list is sorted by severity and then cut to `--top` entries, so on a noisy capture the
    low-severity findings fall off first. The default is 10. Run `wiretrail checkout.har diagnose --top 20`
    to see them all. [Diagnose](diagnose.md) shows the difference. Each check behind the list is cut to
    `--top` groups before it is ranked as well, so a very noisy capture can also hide a serious
    finding: see [Limitations](../reference/limitations.md#-top-cuts-each-check-before-the-findings-are-ranked).

## Look at part of a capture

`--filter` limits every figure to the matching requests:

```bash
wiretrail checkout.har --filter "host:payments.shop.example"
```

<!-- output: sum-filter.txt from="entries:" -->
```text
entries: 99 total, 3 after filter
duration (first start to last response): 70.0s

status classes:
  2xx: 2
  4xx: 1

resource types:
  api: 2
  other: 1

top hosts (by request count):
      3  payments.shop.example

top duplicate calls:
     2x  POST payments.shop.example /v1/charges   (e000087)

slowest requests:
     887ms  e000086 OPTIONS payments.shop.example/v1/charges  [204]
     245ms  e000088 POST payments.shop.example/v1/charges  [200]
     201ms  e000087 POST payments.shop.example/v1/charges  [402]

biggest payloads:
        70 B  e000087 payments.shop.example/v1/charges
        34 B  e000088 payments.shop.example/v1/charges
         0 B  e000086 payments.shop.example/v1/charges

hints:
  - 1 error responses (4xx/5xx/failed)

recommended next steps:
  [MEDIUM] errors
         1x 402 on POST /v1/charges — Your card was declined.

next useful commands: errors
```

The recommendations are recomputed for the subset, so each section of the capture gets its own list.

## JSON

Add `--json` for the same data as a document. The fields are the ones in the text, and
`recommendations` is the list above.

```bash
wiretrail checkout.har --json --top 2 | jq '.result | {total_entries, error_count, hints, top_duplicates}'
```

<!-- output: sum-json.txt -->
```text
{
  "total_entries": 99,
  "error_count": 16,
  "hints": [
    "12x duplicate calls: GET api.shop.example /v1/products/{id}/price ",
    "16 error responses (4xx/5xx/failed)"
  ],
  "top_duplicates": [
    {
      "fingerprint": "GET api.shop.example /v1/products/{id}/price ",
      "count": 12,
      "example_id": "e000026"
    },
    {
      "fingerprint": "POST api.shop.example /v1/session/ping ",
      "count": 12,
      "example_id": "e000038"
    }
  ]
}
```

[JSON schemas](../reference/json-schemas.md) lists every field.

## Exit code

`summary` exits `1` when the capture has any error response or any repeated call, and `0` when it has
neither. Use `diagnose` for a gate on ranked findings.

## Related

- [Auto](auto.md) runs the follow-ups for you.
- [Diagnose](diagnose.md) prints the ranked list on its own.
- [Hosts, subsystems and endpoints](inventory.md) for more detail on the lists.
