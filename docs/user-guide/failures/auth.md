# Auth failures and refresh

`auth` looks at how requests were authorized: which were rejected, whether a host sees different
`Authorization` headers over time, and whether a token refresh went well. It is the command for "why did I
get signed out?" and "why was that 401 sent?".

```bash
wiretrail checkout.har auth
```

<!-- output: auth-default.txt from="auth failures" until="next useful" -->
```text
auth failures:
  2x [401] api.shop.example /v1/cart
  1x [401] api.shop.example /v1/auth/session
  1x [401] api.shop.example /v1/notifications

hosts with inconsistent Authorization: api.shop.example, payments.shop.example

token rotation:
  api.shop.example (2 distinct tokens)

token refreshes:
  e000014 auth.shop.example [200] [old-token-reused]
```

## The four parts

### Auth failures

Requests that came back **401** or **403**, grouped by host, path and status, with a count. `2x [401] /v1/cart`
means two rejected calls to that route. To see why a server rejected one, read its body with [`errors`](errors.md).

### Hosts with inconsistent Authorization

A host that received some requests **with** an `Authorization` header and some **without**. It is a note, not
a fault: a public endpoint such as a config call, or a CORS preflight, carries no header on a host whose other
calls do. In the demo capture both the API and the payments host appear for that reason. The known-good capture
shows the same note:

```bash
wiretrail baseline.har auth
```

<!-- output: auth-baseline.txt from="hosts with" until="next useful" -->
```text
hosts with inconsistent Authorization: api.shop.example, payments.shop.example
```

### Token rotation

A host that received **more than one distinct** `Authorization` value. `api.shop.example (2 distinct tokens)`
means the header changed during the capture, as it should around a refresh. WireTrail compares the whole header
value, so it sees a different token and also a different scheme.

### Token refreshes

A request is treated as a refresh when its normalized path contains `/token`, `/oauth` or `/auth/refresh`, or its query has
`grant_type=refresh_token`. Each refresh is listed with its id, host and status, and tagged when it looks wrong:

| Tag | Meaning |
|---|---|
| `failed` | The refresh did not return a 2xx. |
| `old-token-reused` | The refresh succeeded and a new token appeared afterward, but a **later request still sent a token that was seen before the refresh**. |
| `concurrent` | Another refresh started within this refresh's own duration of its start, before it or after it. |

`[old-token-reused]` on `e000014` is the sign of a race: the app refreshed and then sent a request with the token it
had before. `auth --json` names the later request:

```bash
wiretrail checkout.har auth --json | jq '.result.refreshes'
```

<!-- output: auth-json.txt -->
```text
[
  {
    "id": "e000014",
    "host": "auth.shop.example",
    "status": 200,
    "success": true,
    "concurrent": false,
    "old_token_reused": true,
    "reusing_ids": [
      "e000015"
    ]
  }
]
```

`reusing_ids` is `e000015`, a call to `/v1/notifications` that started 40 ms after the refresh began.
[Debug a token refresh race](recipes/debug-a-token-refresh-race.md) confirms it with the tokens themselves.

## Ranked findings

Two kinds of finding come from `auth`. A refresh that failed or reused an old token is a **HIGH**
`token-refresh-race`. Any 401 or 403 is a **MEDIUM** `auth-failures`, counted across all hosts.

## What it does not see

!!! warning "The refresh check is a heuristic"
    - **It finds a refresh by path or by a query parameter.** A refresh sent as a form body to a path
      such as `/v1/session/renew` is not recognized. On `quirks/refresh-in-body.har` the token changes, but no
      refresh is listed:

        ```bash
        wiretrail quirks/refresh-in-body.har auth
        ```

        <!-- output: auth-body.txt from="token rotation" until="next useful" -->
        ```text
        token rotation:
          api.test.example (2 distinct tokens)
        ```

    - **It compares tokens across hosts.** The check asks whether any request after the refresh used any token that was
      seen before it, on any host. A second service that keeps its own, unrelated token after a refresh on the first one
      makes the refresh look like a race. `quirks/two-hosts-refresh.har` has host A refresh and host B keep its token:

        ```bash
        wiretrail quirks/two-hosts-refresh.har auth
        ```

        <!-- output: auth-cross.txt from="token refreshes" until="next useful" -->
        ```text
        token refreshes:
          e000002 api-a.test.example [200] [old-token-reused]
        ```

        Before you act on `old-token-reused`, confirm that the later request was to the **same** host as the refresh.

## Exit code

`auth` exits `1` when it lists any auth failure, any host with inconsistent `Authorization`, or a refresh that
failed, reused an old token or ran concurrently. Because the inconsistent-host note counts, the command can exit
`1` on a capture with no real problem, as the known-good capture does.

## Related

- [JWTs](jwt.md) decodes the tokens, so you can see which one expired.
- [Debug a token refresh race](recipes/debug-a-token-refresh-race.md) uses `auth`, `jwt` and the timeline together.
- [Status transitions](transitions.md) shows the 401 calls that later succeeded.
