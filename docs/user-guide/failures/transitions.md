# Status transitions

`transitions` finds a failed call that was followed by another call to the same endpoint, and names
what happened next, for the handful of pairs that have a label. It turns "the endpoint returned 401,
then 200" into a label you can scan: auth recovered, rate limit persisted, server error recovered.

```bash
wiretrail checkout.har transitions
```

<!-- output: trans-default.txt from="401 -> 200" until="next useful" -->
```text
401 -> 200  [auth-recovered]  GET api.shop.example/v1/auth/session
  e000011 -> e000016  (gap 700ms)

401 -> 200  [auth-recovered]  GET api.shop.example/v1/cart
  e000013 -> e000017  (gap 520ms)

401 -> 200  [auth-recovered]  GET api.shop.example/v1/notifications
  e000015 -> e000018  (gap 380ms)

502 -> 200  [recovered-5xx]  GET api.shop.example/v1/inventory/{id}
  e000054 -> e000056  (gap 800ms)

503 -> 500  [error-changed]  POST api.shop.example/v1/cart/sync
  e000073 -> e000076  (gap 8.2s)

429 -> 429  [rate-limit-persisted]  GET search.shop.example/v1/suggest
  e000077 -> e000078  (gap 800ms)

429 -> 429  [rate-limit-persisted]  GET search.shop.example/v1/suggest
  e000078 -> e000079  (gap 1.3s)

429 -> 429  [rate-limit-persisted]  GET search.shop.example/v1/suggest
  e000079 -> e000080  (gap 4.1s)

429 -> 200  [rate-limit-recovered]  GET search.shop.example/v1/suggest
  e000080 -> e000081  (gap 5.8s)
```

## How it works

WireTrail takes the calls to each endpoint in time order and compares each call with the next one. A pair is
reported only when the first call **failed** (its status was 0, a 4xx or a 5xx) **and** the pair fits one of the labels below. The endpoint here is the method,
host and normalized path. The query is not part of it.

| Label | The pair |
|---|---|
| `auth-recovered` | A 401 or 403, then a 2xx. |
| `recovered-5xx` | A 5xx, then a 2xx. |
| `rate-limit-recovered` | A 429, then a 2xx. |
| `rate-limit-persisted` | A 429, then another 429. |
| `error-changed` | One failure, then a different failure. |

A pair with no label is not reported. That includes a 404, another 4xx or a request with no status followed by a 200, and a 5xx followed by the same 5xx. Two failures with the **same** status in a row are
not reported either, apart from 429. The two 401s on `/v1/cart` are
one 401 followed by another 401, which has no label, and then a 401 followed by a 200, which is `auth-recovered`.

```bash
wiretrail checkout.har transitions --filter "path:/v1/cart"
```

<!-- output: trans-cart.txt from="401 -> 200" until="next useful" -->
```text
401 -> 200  [auth-recovered]  GET api.shop.example/v1/cart
  e000013 -> e000017  (gap 520ms)
```

`gap` is the time between the **starts** of the two calls. `e000013 -> e000017` names the failed call and the next one.

`503 -> 500 [error-changed]` on the cart sync is the useful one in the list above: the server started with
"unavailable" and ended with "internal error", which suggests something got worse and not better.

## JSON

```bash
wiretrail checkout.har transitions --filter "path:/v1/cart" --json | jq '.result.transitions[0]'
```

<!-- output: trans-json.txt -->
```text
{
  "host": "api.shop.example",
  "method": "GET",
  "norm_path": "/v1/cart",
  "from_status": 401,
  "to_status": 200,
  "from_id": "e000013",
  "to_id": "e000017",
  "gap_ms": 520.0,
  "label": "auth-recovered"
}
```

## Exit code

`transitions` exits `1` when it lists at least one transition and `0` when it lists none.

## Related

- [Retries](../repeats/retries.md) lists the same calls by the failures that triggered them.
- [Auth failures and refresh](auth.md) for the 401 pairs.
- [Rate limits](../repeats/rate-limits.md) for the 429 pairs.
