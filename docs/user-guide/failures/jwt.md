# JWTs

`jwt` finds the JSON Web Tokens in a capture and decodes what they say about themselves: who issued
them, who they are for, and when they expire. It never prints the signature and never prints the token.

```bash
wiretrail checkout.har jwt
```

<!-- output: jwt-default.txt from="8e37a634a25062a2" until="next useful" -->
```text
8e37a634a25062a2 (4x, req.header.authorization) [EXPIRED]
  iss: https://auth.shop.example/
  aud: shop-web
  sub (hashed): b1ca0ff720b2652d
  exp: 1792487342 (expired 301s ago)

fd2ef4823d488917 (56x, resp.body)
  iss: https://auth.shop.example/
  aud: shop-web
  sub (hashed): b1ca0ff720b2652d
  exp: 1792491243 (3600s left)

4bbf025fbfd0c0a4 (7x, req.header.x-device-token)
  iss: https://device.shop.example/
  aud: shop-search
  sub (hashed): 72879224ce3bde2b
  exp: 1792491862 (4211s left)
  warning: token iat is in the future (clock skew?)
```

## Where it looks

In request and response headers, in query values, and in request and response bodies. It treats any run of
letters, digits, `-`, `_` and `.` that has two dots, is at least 20 characters long, and whose first two parts
decode to JSON, as a JWT. Opaque tokens that are not JWTs are not found.

## Read a token

| Part | Meaning |
|---|---|
| `8e37a634a25062a2` | A hash of the whole token. It identifies the token without revealing it, and two calls with the same token have the same hash. |
| `(4x, req.header.authorization)` | How many times the token appears, and **where it was first seen**. |
| `[EXPIRED]` | The token was already expired when it first appeared in the capture. |
| `iss`, `aud` | The issuer and audience claims. |
| `sub (hashed)` | The subject, run through the same hash. WireTrail never prints the subject itself. |
| `exp` | The expiry as a Unix time, with how long it had expired or had left. |
| `warning` | Shown when `iat`, the time of issue, is more than 60 seconds later than the start of the first request in which the token appears. The token claims to be issued after the capture's own clock says it was seen. The capture cannot tell you which of the two clocks is wrong. |

Tokens are sorted with expired ones first, then by how often they appear.

The `source` is where the token was **first** seen, not where it is mostly used. The second token above is
`(56x, resp.body)` because it first arrived in the body of the refresh response. Its 55 other uses are request
headers.

!!! note "Expiry is judged when the token first appears"
    WireTrail does not compare `exp` with today's date. It reconstructs the start time of the first request in which the
    token appears and compares the token with that. `expired 301s ago` means the token had been expired for 301 seconds
    **when it first appeared**. This is why an old capture still reports the right answer. Later uses are not checked, so a
    token that was valid when it first appeared and had expired by its last use is not flagged.

The hash of a token and of a `sub` is computed with the Rust standard library's default hasher. It is
stable for one build of WireTrail and is **not guaranteed to match** between versions, so do not store it.

## Follow one token through the capture

`--json` adds the first and the last entry that used each token:

```bash
wiretrail checkout.har jwt --json | jq '.result.tokens[] | {token_hash, source, occurrences, first_entry_id, last_entry_id, expired: .summary.expired}'
```

<!-- output: jwt-json.txt -->
```text
{
  "token_hash": "8e37a634a25062a2",
  "source": "req.header.authorization",
  "occurrences": 4,
  "first_entry_id": "e000011",
  "last_entry_id": "e000015",
  "expired": true
}
{
  "token_hash": "fd2ef4823d488917",
  "source": "resp.body",
  "occurrences": 56,
  "first_entry_id": "e000014",
  "last_entry_id": "e000098",
  "expired": false
}
{
  "token_hash": "4bbf025fbfd0c0a4",
  "source": "req.header.x-device-token",
  "occurrences": 7,
  "first_entry_id": "e000043",
  "last_entry_id": "e000049",
  "expired": false
}
```

The expired token was used from `e000011` to `e000015`, and the new one first appears in `e000014`, the
refresh response. The expired token's **last** use, `e000015`, comes after the refresh began. That is the evidence
behind [`old-token-reused`](auth.md#token-refreshes).

## Look at one host

```bash
wiretrail checkout.har jwt --filter "host:search.shop.example"
```

<!-- output: jwt-filter.txt from="4bbf025fbfd0c0a4" until="next useful" -->
```text
4bbf025fbfd0c0a4 (7x, req.header.x-device-token)
  iss: https://device.shop.example/
  aud: shop-search
  sub (hashed): 72879224ce3bde2b
  exp: 1792491862 (4211s left)
  warning: token iat is in the future (clock skew?)
```

The search host gets a device token, not a user token. Its issue time is later than the capture's clock at the first request that carried it, which is what the warning
means. The capture cannot show whether the device's clock or the recorder's is the one that is off.

## Showing the raw token

The JSON result always has a `raw_token` field. It is `null` unless you pass `--unsafe-include-secrets`, which fills it with the whole token. The text output does not change:

```bash
wiretrail checkout.har jwt --json | jq -c '.result.tokens[0] | {has_field: has("raw_token"), is_set: (.raw_token != null)}'
```

<!-- output: jwt-raw-off.txt -->
```text
{"has_field":true,"is_set":false}
```

```bash
wiretrail checkout.har jwt --json --unsafe-include-secrets | jq -c '.result.tokens[0] | {has_field: has("raw_token"), is_set: (.raw_token != null)}'
```

<!-- output: jwt-raw.txt -->
```text
{"has_field":true,"is_set":true}
```

Treat that output like a password. See [Handle captures safely](../capture/handle-captures-safely.md).

## What it does not do

`jwt` is **structural only**. It decodes and reads claims, and it does not check a signature, because a HAR has no
key to check it with. A token that decodes and has not expired may still be rejected by the server.

## Exit code

`jwt` exits `1` when any token it found was expired when it first appeared, and `0` otherwise.

## Related

- [Auth failures and refresh](auth.md) uses the same tokens to judge a refresh.
- [Debug a token refresh race](recipes/debug-a-token-refresh-race.md) follows one in detail.
- [How redaction works](../concepts/redaction.md) explains why a JWT in an unusual header is not masked elsewhere.
