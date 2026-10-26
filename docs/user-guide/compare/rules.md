# Rules and rule packs

`rules` checks a capture against conventions. A **rule** is a matcher plus something to require or forbid, and a
**rule pack** is a ready-made group of rules. A rule finding has a severity, a description and the entries that broke it.

```bash
wiretrail checkout.har rules --pack auth,security,caching,payments,rest,graphql --top 20
```

<!-- output: rules-packs.txt from="[high] caching" until="next useful" -->
```text
[high] caching: GET 200 needs Cache-Control
  missing required header: Cache-Control (48 entries)

[high] auth: Authorization required
  missing required header: Authorization (40 entries)

[high] payments: idempotency key on charges
  missing required header: Idempotency-Key (2 entries)

[high] security: no secrets in query
  opaque secret in query param `d` (1 entries)

[medium] rest: no mutation over GET
  GET request carries a body (1 entries)

[low] graphql: operationName required
  GraphQL POST without operationName (2 entries)
```

## Rule packs

`--pack` takes a comma-separated list. These are the packs:

| Pack | What it flags | Severity |
|---|---|---|
| `auth` | A request with no `Authorization` header | high |
| `caching` | A `GET` that returned `200` with no `Cache-Control` request header | high |
| `payments` | A request whose path contains `charge` or `payment` and has no `Idempotency-Key` header | high |
| `security` | A query parameter whose value looks like a random token. It names the **parameter**, never the value | high |
| `rest` | A `GET` request that carries a body | medium |
| `graphql` | A `POST` to a path ending `/graphql` whose body has no `operationName` | low |

Two things make the packs noisy, and both are worth knowing:

- **They apply to every request.** The `auth` pack flags the CDN's images and the search host, which never carry a token, as
  well as the API. The `payments` pack flags the browser's `OPTIONS` preflight to the charges URL, which cannot carry the header.
  Scope a pack with `--filter`:

    ```bash
    wiretrail checkout.har rules --pack auth --filter "host:api.shop.example method:get"
    ```

    <!-- output: rules-auth-scoped.txt from="[high]" until="next useful" -->
    ```text
    [high] auth: Authorization required
      missing required header: Authorization (2 entries)
    ```

- **The header checks read the request.** `caching` looks for a `Cache-Control` header on the **request**, not the response. A client
  rarely sends one, so it flags nearly every successful GET.

A pack name that does not exist is **ignored without a message**. `--pack paymets` prints an empty result and exits `0`:

```bash
wiretrail checkout.har rules --pack nonsense
```

<!-- output: rules-unknown.txt from="== wiretrail rules" until="next useful" -->
```text
== wiretrail rules ==
```

## Your own rules

Rules go in the `rules:` list of [the configuration file](config-file.md). Each has a `name`, optional matchers and one or
more requirements.

```yaml
rules:
  - name: "Storefront calls are fast enough"
    host: "api.shop.example"
    max_latency_ms: 500
  - name: "No tracking hosts in a checkout capture"
    host: "*.ingest.sentry.io"
    forbid: true
```

| Key | Meaning |
|---|---|
| `host`, `path`, `method`, `status` | Matchers, all optional. Each is a glob, and a request must match every one given. `path` is the raw URL path, and `status` is matched against the text of the status, so `"5*"` and `"404"` both work. With no matcher, a rule applies to every request. |
| `require_headers` | Request headers that must be present. A missing one is a **high** finding. |
| `max_latency_ms` | The longest allowed duration in milliseconds. A slower request is a **medium** finding. |
| `forbid` | If `true`, any request that matches is a **high** finding. |

```bash
wiretrail checkout.har rules --config config/wiretrail.yaml
```

<!-- output: rules-config.txt from="[high] No tracking" until="[medium] Storefront calls are fast enough" -->
```text
[high] No tracking hosts in a checkout capture
  matched a forbidden rule (3 entries)
```

The `forbid` rule reports three entries: the three calls to the error-reporting host. A `max_latency_ms` rule produces **one
finding for each distinct duration**, because the duration is part of the text. A rule with a 500 ms budget gives six separate lines here.
If that is too noisy, use `compare` for latency, or `--top` to cut the list.

`rules` reads `wiretrail.yaml` from the current folder when you do not pass `--config`.

## Findings

Findings are sorted by severity, then by how many entries they cover. Each one reads:

```text
[high] payments: idempotency key on charges
  missing required header: Idempotency-Key (2 entries)
```

`--json` lists the findings with their entry ids:

```bash
wiretrail checkout.har rules --pack payments --json | jq '.result.findings'
```

<!-- output: rules-json.txt -->
```text
[
  {
    "rule": "payments: idempotency key on charges",
    "severity": "high",
    "detail": "missing required header: Idempotency-Key",
    "entry_ids": [
      "e000086",
      "e000087"
    ]
  }
]
```

## Exit code

`rules` exits `1` when it reports any finding and `0` when it reports none, including when no rules are configured and no pack is named:

```bash
wiretrail baseline.har rules > /dev/null; echo "exit code: $?"
```

<!-- output: rules-exit-none.txt -->
```text
exit code: 0
```

## Related

- [The configuration file](config-file.md) is where rules live.
- [Built-in checks](checks.md) covers required headers and content types.
- [Compare against a baseline](compare.md) judges latency against a past capture instead of a fixed budget.
