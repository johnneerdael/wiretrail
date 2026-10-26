# Errors

`errors` groups the failed requests and tells you, for each group, what the server said. It reads the
message and the error code out of the response body, lists the correlation ids from the response headers,
and shows a short excerpt of the body with secrets masked.

```bash
wiretrail checkout.har errors --top 3
```

<!-- output: err-top3.txt from="4x" until="next useful" -->
```text
   4x  [503] POST api.shop.example/v1/cart/sync
  message: inventory service unavailable
  code: upstream_unavailable
  correlation: req_fa6f840121, e1e598c1a6e1ae1d-AMS
  body: {"error":{"code":"upstream_unavailable","message":"inventory service unavailable"}}
  entries: e000069, e000070, e000072, e000073

   4x  [429] GET search.shop.example/v1/suggest
  message: too many requests
  code: rate_limited
  body: {"error":{"code":"rate_limited","message":"too many requests"}}
  entries: e000077, e000078, e000079, e000080

   2x  [401] GET api.shop.example/v1/cart
  message: JWT expired
  code: token_expired
  correlation: req_e80ca59c03, 1bd5f212c6ef5780-AMS
  body: {"error":{"code":"token_expired","message":"JWT expired"}}
  entries: e000012, e000013
```

## What counts as an error

A request is an error when its status is a **4xx**, a **5xx**, or **0**, meaning it failed before any
response arrived. A 3xx is not an error. Errors are grouped by host, method, normalized path **and
status**, so a 503 and a 500 on the same route are two groups.

Groups are sorted by count, then by status, highest first. `--top` bounds the list and defaults to 10.

## Read a group

| Part | Meaning |
|---|---|
| `4x  [503] POST api.shop.example/v1/cart/sync` | Count, status, method, host and normalized path. |
| `message:` | The first of these JSON keys that holds a string or number: `message`, `error_description`, `error`, `reason`, `detail`, `details`. If there is none, WireTrail looks for the same keys inside an `error` object. |
| `code:` | The first of `code`, `error_code` or `status`, in the same way. |
| `correlation:` | The ids from the response headers `x-request-id`, `x-correlation-id`, `traceparent`, `x-amzn-trace-id`, `cf-ray` and `x-datadog-trace-id` that are present, for the first request in the group. |
| `body:` | The first 200 characters of the first response body. JSON keys that look secret are masked, and line breaks become spaces. |
| `entries:` | Every entry id in the group. |

The message, code, correlation ids and body come from the **first** request in the group. If later requests in
the group answered differently, you will not see it here. Open one with [`show-entry`](../inspect/show-entry.md).

A response whose body is not JSON, or that has none, shows no `message` or `code`. A request with status
`0` has no response at all:

```bash
wiretrail checkout.har errors --filter "status:0"
```

<!-- output: err-zero.txt from="1x" until="next useful" -->
```text
   1x  [0] GET cdn.shop.example/img/banner.webp
  entries: e000059
```

The `status:0` filter is needed: `status:>=400` skips these requests, as [Filter entries](../analyze/filters.md#a-request-with-no-status-has-status-0) explains.

## Ask for one host

```bash
wiretrail checkout.har errors --filter "host:payments.shop.example"
```

<!-- output: err-payment.txt from="1x" until="next useful" -->
```text
   1x  [402] POST payments.shop.example/v1/charges
  message: Your card was declined.
  code: card_declined
  correlation: req_e4cd9f9aab, 8afbe09b31f39a02-AMS
  body: {"error":{"code":"card_declined","message":"Your card was declined."}}
  entries: e000087
```

A declined card is an error to WireTrail but not necessarily a bug: `message` says the card was declined, which the
server returned on purpose. Read `message` before you assume something is broken.

## JSON

```bash
wiretrail checkout.har errors --top 1 --json | jq '.result.groups[0] | del(.entry_ids)'
```

<!-- output: err-json.txt -->
```text
{
  "host": "api.shop.example",
  "method": "POST",
  "norm_path": "/v1/cart/sync",
  "status": 503,
  "count": 4,
  "error_message": "inventory service unavailable",
  "error_code": "upstream_unavailable",
  "body_snippet": "{\"error\":{\"code\":\"upstream_unavailable\",\"message\":\"inventory service unavailable\"}}",
  "correlation_ids": [
    "req_fa6f840121",
    "e1e598c1a6e1ae1d-AMS"
  ],
  "first_offset_ms": 25000.0,
  "last_offset_ms": 33900.0
}
```

Only `body_snippet` is masked, by JSON key, and `--unsafe-include-secrets` changes only that field. `error_message`, `error_code` and `correlation_ids` are copied
exactly as the server sent them, so read them before you share the output. See [How redaction works](../concepts/redaction.md).

## Exit code

`errors` exits `1` when it lists at least one group and `0` when it lists none.

## Related

- [Status transitions](transitions.md) shows what a failed call turned into.
- [Retries](../repeats/retries.md) shows which failures were tried again.
- [Show an entry](../inspect/show-entry.md) opens one failed request in full.
