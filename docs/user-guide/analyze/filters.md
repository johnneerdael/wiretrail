# Filter entries

`--filter` limits a command to the requests that match. The figures and findings are recomputed for
the subset, so a filtered `summary` or `diagnose` describes only those requests. Three things ignore
it: `validate`, `show-entry` and `curl` with an entry id. They describe the whole capture, or the one
entry you named.

```bash
wiretrail checkout.har --filter "host:search.shop.example" summary
```

A filter is one or more **clauses** of the form `key:value`, separated by spaces. A request must match
every clause.

## The clauses

| Clause | Matches | Example |
|---|---|---|
| `host:` | The host name, as a glob | `host:api.shop.example`, `host:*.shop.example` |
| `method:` | The HTTP method, in any case | `method:post` |
| `path:` | The URL path, as a glob. The query string is not part of it | `path:/v1/cart/sync`, `path:*login*` |
| `status:` | The status, with an optional comparison | `status:429`, `status:>=500`, `status:<300` |
| `time:` | The request duration in milliseconds, with an optional comparison and an optional `ms` | `time:>1000`, `time:>=500ms` |
| `has:` | A header or body that is present | `has:req.header.authorization`, `has:resp.header.retry-after`, `has:resp.body` |

- **Comparisons** are `>=`, `<=`, `>`, `<` and `=`. With none, the value must be equal.
- **Globs** use `*` for any run of characters, and nothing else: no `?`, no brackets, no regular
  expressions. Matching ignores case. A glob must match the whole value, so `host:shop.example` matches
  only that host and not `api.shop.example`. Write `host:*shop.example` for both.
- **`has:`** takes `req.header.<name>`, `resp.header.<name>`, `req.body` or `resp.body`. Header names
  ignore case. A body counts as present when it is not empty.
- **There is no OR and no NOT.** To combine alternatives, run the command twice.

## Examples

Server errors on any host under `shop.example`:

```bash
wiretrail checkout.har --filter "host:*.shop.example status:>=500" errors
```

<!-- output: flt-glob.txt from="==" until="entries:" -->
```text
== wiretrail errors ==

   4x  [503] POST api.shop.example/v1/cart/sync
  message: inventory service unavailable
  code: upstream_unavailable
  correlation: req_fa6f840121, e1e598c1a6e1ae1d-AMS
  body: {"error":{"code":"upstream_unavailable","message":"inventory service unavailable"}}
```

Calls that took more than a second, with their phases:

```bash
wiretrail checkout.har slowest --filter "time:>1000" --top 3
```

<!-- output: flt-time.txt from="2.4s" until="1.9s  e000059" -->
```text
    2.4s  e000090 POST api.shop.example/v1/orders  [201]
  bottleneck: server wait/TTFB
  phases: wait 2.4s / receive 8ms / send 1ms / connect 0ms / dns 0ms / ssl 0ms / blocked 0ms

    1.9s  e000009 GET cdn.shop.example/img/hero.webp  [200]
  bottleneck: download/receive
  phases: wait 120ms / receive 1.8s / send 1ms / connect 0ms / dns 0ms / ssl 0ms / blocked 0ms
```

Responses that carry a `Retry-After` header:

```bash
wiretrail checkout.har --filter "has:resp.header.retry-after" timeline
```

<!-- output: flt-has.txt from="46.0s" until="next useful" -->
```text
   46.0s     25ms  e000077 GET search.shop.example/v1/suggest  [429] 63 B
   46.8s     30ms  e000078 GET search.shop.example/v1/suggest  [429] 63 B
   48.1s     33ms  e000079 GET search.shop.example/v1/suggest  [429] 63 B
   52.2s     29ms  e000080 GET search.shop.example/v1/suggest  [429] 63 B
```

## A request with no status has status 0

A request that failed before the server answered has no status, and a HAR records it as `0`. That
is less than 400, so `status:>=400` skips it. To find those requests, ask for `0`:

```bash
wiretrail checkout.har timeline --filter "status:0"
```

<!-- output: flt-zero.txt from="15.0s" until="next useful" -->
```text
   15.0s     1.9s  e000059 GET cdn.shop.example/img/banner.webp  [0] 0 B
```

## More than one `--filter`

You can repeat the flag. The clauses of all of them are combined, so this is the same as putting every
clause in one:

```bash
wiretrail checkout.har --filter "method:post" --filter "status:>=400" errors --top 3
```

<!-- output: flt-twice.txt from="4x" head="3" -->
```text
   4x  [503] POST api.shop.example/v1/cart/sync
  message: inventory service unavailable
  code: upstream_unavailable
```

## Where the flag goes

`--filter` is a global flag, so it works before or after the command name. The two forms below are
the same:

```bash
wiretrail checkout.har --filter "host:api.shop.example" errors
wiretrail checkout.har errors --filter "host:api.shop.example"
```

## Paths are the raw path

`path:` matches the path as it appears in the URL, not the normalized path that WireTrail shows in
its output. The output shows `/v1/products/{id}/price`, but the filter must match `/v1/products/1042/price`.
Put a `*` where the id goes:

```bash
wiretrail checkout.har --filter "path:/v1/products/*/price" summary --top 1
```

<!-- output: flt-path.txt from="entries:" until="status classes" -->
```text
entries: 99 total, 12 after filter
duration (first start to last response): 70.0s
```

Twelve of the 99 requests matched. [Entries, ids and endpoints](../concepts/entries-ids-endpoints.md) explains normalization.

The query string is not part of the path, so a clause that mentions it matches nothing:

```bash
wiretrail checkout.har --filter "path:*page=2*" summary --top 1
```

<!-- output: flt-query.txt from="entries:" until="duration" -->
```text
entries: 99 total, 0 after filter
```

There is no clause for query parameters.

## Quote the filter

A filter contains characters that your shell treats specially. `>` starts a redirection, and a space
splits the argument. Put the whole filter in quotes:

```bash
wiretrail checkout.har errors --filter "status:>=500"
```

Without the quotes the shell reads `>=500` as a redirect to a file named `=500`, and WireTrail
receives only `status:`.

## A mistake in a filter

A filter that WireTrail cannot read stops the command with exit code `2`:

```bash
wiretrail checkout.har --filter "colour:red" summary
```

<!-- output: flt-bad.txt from="wiretrail:" -->
```text
wiretrail: invalid filter: unknown filter key: colour
```

The other messages are `invalid filter clause: <text>` for a token with no colon, and `invalid number:
<text>` for a `status:` or `time:` that is not a number.

## Related

- [Auto](auto.md) applies your filter to every drill-down.
- [Command line](../reference/command-line.md) lists the global flags.
- [Entries, ids and endpoints](../concepts/entries-ids-endpoints.md) explains why paths are normalized.
