# Redirects

`redirects` lists the responses that told the client to go somewhere else, grouped by route. It flags
two things: a redirect that happens over and over (a storm), and a redirect that goes to another host.

```bash
wiretrail checkout.har redirects
```

<!-- output: redir-default.txt from="6x" until="next useful" -->
```text
   6x  [302] GET cdn.shop.example/img/hero.png [storm]
  -> cdn.shop.example
  entries: e000007, e000020, e000039, e000053, e000067, e000074

   1x  [301] GET www.shop.example/ [cross-host]
  -> shop.example
  entries: e000000
```

## What counts as a redirect

A response with status 301, 302, 303, 307 or 308, or any response that has a redirect URL. Redirects are
grouped by host, method, normalized path and status.

| Tag | Meaning |
|---|---|
| `[storm]` | The group has **five or more** requests. |
| `[cross-host]` | The redirect target is on a different host from the request. |

`-> cdn.shop.example` is the host of the target. The six requests above all ask for `/img/hero.png` and are sent to
`/img/hero.webp`. The page keeps asking for the old URL, so every view pays for a redirect. The fix is to use
the final URL. The `301` from `www.shop.example` to the bare host is a one-off and is the expected way to canonicalize a host.

`-> host` is shown only when the entry's `redirectURL` is a full URL. WireTrail reads that field and not the `Location` header. When it is relative,
such as `/b.png`, or empty, WireTrail cannot read a host from it. The group is listed without a target and is never marked
`[cross-host]`:

```bash
wiretrail quirks/relative-redirect.har redirects
```

<!-- output: redir-rel.txt from="6x" until="next useful" -->
```text
   6x  [302] GET cdn.test.example/a.png [storm]
  entries: e000000, e000001, e000002, e000003, e000004, e000005
```

## The ranked finding

A redirect group of five or more requests becomes a **LOW** `redirect-storm` finding in `summary`, `diagnose` and `auto`. It is `LOW`,
the lowest level, because a redirect costs a round trip and not a failure.

## JSON

```bash
wiretrail checkout.har redirects --top 2 --json | jq '.result.groups[] | del(.entry_ids)'
```

<!-- output: redir-json.txt -->
```text
{
  "host": "cdn.shop.example",
  "method": "GET",
  "norm_path": "/img/hero.png",
  "status": 302,
  "count": 6,
  "target_host": "cdn.shop.example",
  "cross_host": false,
  "is_storm": true,
  "first_offset_ms": 420.0,
  "last_offset_ms": 36400.0
}
{
  "host": "www.shop.example",
  "method": "GET",
  "norm_path": "/",
  "status": 301,
  "count": 1,
  "target_host": "shop.example",
  "cross_host": true,
  "is_storm": false,
  "first_offset_ms": 0.0,
  "last_offset_ms": 0.0
}
```

## Exit code

`redirects` exits `1` only when a group is a storm. A capture with one redirect exits `0`.

## Related

- [Timeline](../analyze/timeline.md) shows when each redirect happened.
- [Errors](errors.md) for requests that failed rather than redirected.
