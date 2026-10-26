# Limitations

What WireTrail does not do, and the places where a result can mislead. Every item here was run on a real file, and the file is in `assets/downloads/quirks/` or is the demo capture, so
you can reproduce it. Items are grouped by what they affect.

## Redaction is pattern-based

WireTrail masks known header names, known JSON key names, and values that look like random tokens. A secret outside those patterns is printed. The full list is in
[How redaction works](../concepts/redaction.md#what-gets-through). In short:

- **A JWT in a header other than the known ones** is printed in full by `show-entry`, `curl` and `handoff`.
- **`extract` and `search` mask by shape only.** A short password, a JWT and a short token are all printed.
- **A form-encoded body is not masked at all.** Only JSON bodies get key-based masking.
- **`errors`, `report`, `diagnose`, `auto` and `summary` copy the server's message unmasked.**
- **`summary` prints the query values** of a repeated call, and `duplicates --json` and `retries --json` include them in `fingerprint`.

Short examples, with the commands that print them:

```console
$ wiretrail quirks/short-secrets.har extract '$.password'
```

<!-- output: ext-secret.txt from="e000000" until="next useful" -->
```text
e000000  hunter2
```

<!-- output: red-form.txt cmd -->
```console
$ wiretrail quirks/unmasked-places.har show-entry e000002 | grep 'request body'
request body: user=ann&password=hunter2&grant_type=password
```

Treat every output as sensitive until you have read it, and strip a capture before you share the file. See [Handle captures safely](../capture/handle-captures-safely.md).

## Pagination flags a constant page size

`pagination` tags a sequence `[repeated-cursor]` when a value of any pagination parameter appears more than once, and `limit` is one of those parameters. A constant page size therefore trips it,
on a walk of three different pages:

<!-- output: pg-const.txt cmd -->
```console
$ wiretrail quirks/constant-limit.har pagination
== wiretrail pagination ==

pagination sequences:
  3 pages  GET api.test.example/v1/items  (by limit,page) [repeated-cursor]

next useful commands: storms · duplicates · endpoints
```

Read the page values with `timeline` before you conclude that a loop exists. See [Pagination and N+1](../repeats/pagination.md).

## Diff treats some real differences as volatile

`diff` calls a body `volatile-only` when every key that differs matches a list of timestamp-like words by **substring**: `timestamp`, `ts`, `nonce`, `date`, `cb`, `cachebuster`, `requestid`, `request_id` and `_`.
Any snake_case key, and any key that contains `ts` or `date` (`events`, `results`, `stats`, `updated`), counts:

<!-- output: diff-volatile.txt cmd -->
```console
$ wiretrail quirks/volatile-substring.har diff
== wiretrail diff ==

POST api.test.example/v1/orders  (2 calls, body: volatile-only)

next useful commands: duplicates · show-entry · endpoints
```

The two calls place different orders. Read the bodies before you call a repeat redundant. See [What varies between calls](../repeats/diff.md#body-verdicts).

## The refresh-race check can be wrong in both directions

`auth` finds a refresh by path (`/token`, `/oauth`, `/auth/refresh`) or by a `grant_type=refresh_token` **query** parameter, and it asks whether a later call used any token seen before the refresh, **on any host**.

A refresh sent in a form body to another path is missed:

<!-- output: auth-body.txt cmd from="== wiretrail auth" -->
```console
$ wiretrail quirks/refresh-in-body.har auth
== wiretrail auth ==

hosts with inconsistent Authorization: api.test.example

token rotation:
  api.test.example (2 distinct tokens)

next useful commands: jwt · transitions · errors
```

A second host that keeps its own token makes a healthy refresh look like a race:

<!-- output: auth-cross.txt cmd from="== wiretrail auth" -->
```console
$ wiretrail quirks/two-hosts-refresh.har auth
== wiretrail auth ==

hosts with inconsistent Authorization: api-a.test.example

token rotation:
  api-a.test.example (2 distinct tokens)

token refreshes:
  e000002 api-a.test.example [200] [old-token-reused]

next useful commands: jwt · transitions · errors
```

Confirm a `[old-token-reused]` tag against the later request's host, and with [`jwt`](../failures/jwt.md). See [Auth failures and refresh](../failures/auth.md#what-it-does-not-see).

## Rate limits

`Retry-After` is read as a number of seconds. An HTTP date is ignored, so no cooldown is checked:

<!-- output: rl-date.txt cmd from="== wiretrail rate-limit" -->
```console
$ wiretrail quirks/retry-after-date.har rate-limit
== wiretrail rate-limit ==

api.test.example /v1/x  (1x 429)

next useful commands: errors · retries · transitions
```

The cooldown check only looks at the same host and path.

## `--top` cuts each check before the findings are ranked

`summary`, `diagnose` and `auto` build their findings from the checks behind `errors`, `retries`, `storms`, `duplicates`, `redirects`, `rate-limit`, `auth` and `slowest`. Each check is asked for
only its first `--top` groups, largest first, and the findings are built from what is left. A large group of little importance can use up the allowance before a small, serious one is seen. The
findings are then ranked, and cut to `--top` once more.

`quirks/many-error-groups.har` has eleven routes that each answered 404 four times, and one route, `/broken`, that answered 503 three times. Three 5xx responses on one route is a `HIGH` finding.
At the default `--top` of 10 the list has no `HIGH` finding, and `grep` counts zero:

<!-- output: lim-top-default.txt cmd -->
```console
$ wiretrail quirks/many-error-groups.har diagnose | grep -c '^\[HIGH\]'
0
```

With `--top 20` every group gets through, and the finding is there:

<!-- output: lim-top-20.txt cmd -->
```console
$ wiretrail quirks/many-error-groups.har diagnose --top 20 | grep '^\[HIGH\]'
[HIGH] 5xx-cluster — 3x 503 on GET /broken
```

Do not read a list with no `HIGH` finding as a clean capture when it has many error routes. Raise `--top`, or ask for the serious groups directly with
`wiretrail capture.har errors --filter "status:>=500"`.

## The ranking commands slow down on a capture with many 429s

The cooldown check grows faster than the square of the number of 429s on one route (doubling them from 2,000 to 4,000 multiplied the time by more than seven), and `summary`, `diagnose`, `auto` and `report` run it. 4,000 of them took about ten seconds. See [Performance](performance.md#the-cooldown-check-scales-with-the-429s).

## A slow TLS handshake is labelled TCP connect

A HAR counts the handshake inside `connect`, so `connect` should never be smaller than `ssl`:

<!-- output: slow-tls.txt cmd from="731ms" until="next useful" -->
```console
$ wiretrail quirks/tls-in-connect.har slowest
   731ms  e000000 GET tls.test.example/  [200]
  bottleneck: TCP connect
  phases: wait 20ms / receive 5ms / send 1ms / connect 700ms / dns 5ms / ssl 600ms / blocked 0ms
```

Read `ssl` in `phases`. See [Timing phases](../concepts/timing-phases.md).

## A relative redirect shows no target

`redirects` reads the host from the entry's `redirectURL` field, not from the `Location` header. A relative or empty `redirectURL` has no host, so the group is listed with no `->` line and is never marked `[cross-host]`:

<!-- output: redir-rel.txt cmd from="6x" until="next useful" -->
```console
$ wiretrail quirks/relative-redirect.har redirects
   6x  [302] GET cdn.test.example/a.png [storm]
  entries: e000000, e000001, e000002, e000003, e000004, e000005
```

## Compare does not see call counts

`compare` judges errors, median time and median size. It does not count calls, so a heartbeat that grew from 3 calls to 12 gives no finding:

<!-- output: cmp-ping.txt cmd from="max severity" until="next useful" -->
```console
$ wiretrail checkout.har compare baseline.har --filter "path:/v1/session/ping"
max severity: none
```

Also: in text, only a **count** of new and removed endpoints is shown, `--json` lists them, and `--top` cuts both; `critical` is accepted by `--fail-on` and never produced; and errors that were already present in the
baseline do not count as new. See [Compare against a baseline](../compare/compare.md#what-compare-does-not-see).

## Export ignores `--top` and `--json`

`export` always writes every matching entry as raw rows. See [Export](../inspect/export.md#-top-and-json-are-ignored).

## Some rule packs apply to every request

The `auth` pack has no host, path or method scope, so it flags images and preflights. `payments` is scoped by path only (`*charge*` and `*payment*`), so it flags a CORS `OPTIONS` call to the charges URL.
`caching` is limited to GET requests with status 200, `rest` to GET requests that carry a body, and `graphql` to POST requests to a path ending in `/graphql`. A pack name that does not exist is ignored with no message:

<!-- output: rules-unknown.txt cmd from="== wiretrail rules" until="next useful" -->
```console
$ wiretrail checkout.har rules --pack nonsense
== wiretrail rules ==
```

Scope a pack with `--filter`. See [Rules and rule packs](../compare/rules.md).

## A misspelled configuration key is ignored

<!-- output: cfg-typo.txt cmd from="api.shop.example" until="cdn.shop.example" -->
```console
$ wiretrail checkout.har subsystems --config config/typo.yaml --top 2
api.shop.example  [-]  (63 req, 10 err, 53 dup)
  window: 900ms - 69.8s
  hosts: api.shop.example
```

`ownershp:` is not an error, and nothing is named. See [The configuration file](../compare/config-file.md#mistakes).

## Filters

`status:>=400` skips a request with no status, because its status is `0`. Ask for it with `status:0`:

<!-- output: flt-zero.txt cmd from="15.0s" until="next useful" -->
```console
$ wiretrail checkout.har timeline --filter "status:0"
   15.0s     1.9s  e000059 GET cdn.shop.example/img/banner.webp  [0] 0 B
```

`path:` matches the raw path and never the query string. There is no OR, no NOT and no regular expression. See [Filter entries](../analyze/filters.md).

## Time

A start time that cannot be parsed is ignored and gets offset `0`, with no warning, so storms, cascades and cooldowns computed from it are wrong. See
[How WireTrail reads a HAR](../concepts/how-wiretrail-reads-a-har.md#time).

## The startup critical path is approximate

A HAR has no dependencies, so `startup` builds a chain of calls that did not overlap, in start order. It is not the real critical path. See [Startup profile](../failures/startup.md).

## Token hashes are not stable across versions

`jwt` hashes tokens and `sub` claims with the Rust standard library's default hasher, which is stable within one build and not guaranteed between versions. Do not store the hashes.

## A finding is not a cause

Ranked findings are patterns. A 503 shows that a server answered with an error, and not why. A retry loop shows that a client retried, and not whether it meant to. A "cascade" is later failures,
not proven consequences. See [How findings are ranked](../concepts/how-findings-are-ranked.md).

## Files it cannot read

Compressed files, files with a byte-order mark, and JSON that is not a HAR all give `failed to parse HAR JSON`, and the message does not say which. Bodies stored with `encoding: base64` are
read as the encoded text. See [Errors and exit codes](errors-and-exit-codes.md) and [HAR support](har-support.md).

## What a HAR cannot tell you

Packet loss, the full TLS certificate chain, client call stacks, DNS resolver details, and what a service worker or a proxy did are not in a HAR unless a tool wrote them into a custom field.
JWT analysis is structural: WireTrail reads the claims and the expiry and does not verify a signature, because a HAR has no key to verify it with. N+1 detection is a heuristic.

## Related

- [How redaction works](../concepts/redaction.md) lists every place a secret can be printed.
- [Performance](performance.md) has the measured costs.
- [Command line](command-line.md) lists every flag.
