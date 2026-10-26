# What varies between calls

`diff` answers "these calls look identical, so what is different?" For every endpoint that was called
at least twice, it lists the query parameters, the request headers and the body that change between calls.
It is how you tell a call that is truly redundant from one that only looks that way.

```bash
wiretrail checkout.har diff
```

<!-- output: diff-default.txt from="POST api.shop.example/v1/session/ping" until="GET api.shop.example/v1/cart" -->
```text
POST api.shop.example/v1/session/ping  (12 calls, body: volatile-only)

GET search.shop.example/v1/suggest  (12 calls, body: none)
  query q varies: r, ru, run
  headers vary: x-device-token

GET api.shop.example/v1/flags  (6 calls, body: none)
  query cb varies: 1792487642, 1792487649, 1792487656
```

## What it compares

Calls are grouped by method, host and normalized path, **not** by query. So one group holds calls
with different query strings, and the group shows which keys differ. Only groups with some variation are listed.

| Line | Meaning |
|---|---|
| `12 calls` | Calls in the group. |
| `query q varies: r, ru, run` | The query key that differs, with up to the first three distinct values. Sensitive values are masked. |
| `headers vary: x-device-token` | Request header names whose value differs between calls, including calls that lack the header. Names are lower case. |
| `body: volatile-only` | The body verdict, below. |

### Body verdicts

| Verdict | Meaning |
|---|---|
| `none` | Fewer than two calls have a request body. |
| `identical` | Every body is the same text. |
| `volatile-only` | The bodies are JSON objects, and only keys that name a timestamp, nonce, date, cache-buster or request id differ. |
| `meaningful` | Something else differs, or the bodies are not JSON objects. |

A key counts as volatile when its lower-case name equals or **contains** any of `timestamp`, `ts`, `nonce`,
`date`, `cb`, `cachebuster`, `requestid`, `request_id` or `_`.

!!! warning "The volatile test is broad"
    The list includes `_` and `ts`, and the test is a substring test. So any key with an underscore, such
    as `order_id`, and any key that ends in `ts`, such as `events`, `results` or `points`, counts as
    volatile. Two orders that differ only in those keys get `volatile-only`:

    ```bash
    wiretrail quirks/volatile-substring.har diff
    ```

    <!-- output: diff-volatile.txt from="POST api.test.example" until="next useful" -->
    ```text
    POST api.test.example/v1/orders  (2 calls, body: volatile-only)
    ```

    Those two calls place different orders, and the verdict does not show it. Treat `volatile-only` as "worth
    checking", not "proved redundant", and read the bodies with [`extract`](../inspect/extract.md) or
    [`show-entry`](../inspect/show-entry.md). [Limitations](../reference/limitations.md#diff-treats-some-real-differences-as-volatile) lists this.

## Read a result

The heartbeat is `volatile-only`: only a key named `ts` changes. Which key, and what it holds:

```bash
wiretrail checkout.har extract '$.ts' --target req --filter "path:/v1/session/ping" --top 3
```

<!-- output: diff-ping-ts.txt from="e000038" until="next useful" -->
```text
e000038  1792487648000
e000041  1792487649500
e000042  1792487651000
```

The timestamp is 1.5 seconds apart each time, and nothing else is different. The calls are redundant
apart from the clock, so removing them loses no information. [Remove redundant calls](recipes/remove-redundant-calls.md)
does it.

An endpoint whose calls differ **only** in the id in the path shows nothing. The twelve price calls are
identical once ids are normalized, so `diff` prints an empty list for them:

```bash
wiretrail checkout.har diff --filter "path:/v1/products/*/price"
```

<!-- output: diff-price.txt from="== wiretrail diff" until="next useful" -->
```text
== wiretrail diff ==
```

Empty output means nothing varies in query, headers or body. It does not say the calls are the same
request. Look at the path ids with [`duplicates`](duplicates.md) or `timeline`.

## When a header varies

A varying `authorization` header is worth noticing. It shows that a token changed between calls:

<!-- output: diff-default.txt from="GET api.shop.example/v1/cart" until="POST api.shop.example/graphql" -->
```text
GET api.shop.example/v1/cart  (4 calls, body: none)
  headers vary: authorization

GET api.shop.example/v1/products  (4 calls, body: none)
  query page varies: 1, 2, 3
```

The cart calls span the expired token and its replacement. [Auth failures and refresh](../failures/auth.md)
reads that.

## JSON

```bash
wiretrail checkout.har diff --filter "path:/v1/session/ping" --json | jq '.result.groups[0] | del(.entry_ids)'
```

<!-- output: diff-json.txt -->
```text
{
  "host": "api.shop.example",
  "method": "POST",
  "norm_path": "/v1/session/ping",
  "count": 12,
  "varying_query": [],
  "varying_header_names": [],
  "body_verdict": "volatile-only"
}
```

## Exit code

`diff` exits `1` when any group has a `meaningful` body or a varying `authorization` header, and `0`
otherwise. A heartbeat that is `volatile-only` exits `0`:

```bash
wiretrail checkout.har diff --filter "path:/v1/session/ping" > /dev/null; echo "exit code: $?"
```

<!-- output: diff-exit.txt -->
```text
exit code: 0
```

## Related

- [Duplicates](duplicates.md) finds the groups that `diff` explains.
- [Remove redundant calls](recipes/remove-redundant-calls.md) uses the `volatile-only` verdict.
- [Extract a field](../inspect/extract.md) reads one field across bodies.
