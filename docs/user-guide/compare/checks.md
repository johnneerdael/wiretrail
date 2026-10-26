# Built-in checks

`checks` looks for two kinds of mistake that need no configuration to define: a body whose content type does not match
what it holds, and, when you give it a configuration file, a required header that is missing.

```bash
wiretrail checkout.har checks
```

<!-- output: checks-default.txt from="[content-type] api.shop.example /v1/track" until="next useful" -->
```text
[content-type] api.shop.example /v1/track
  request JSON body without application/json content-type (3 entries)

[content-type] api.shop.example /v1/legacy/status
  JSON response served as text/html (1 entries)

[content-type] api.shop.example /v1/cart/items/{id}
  empty body with JSON content-type (1 entries)
```

## Content-type checks

| Finding | What WireTrail saw |
|---|---|
| `request JSON body without application/json content-type` | The request body starts with `{` or `[` and the request's `Content-Type` does not contain `json`. |
| `JSON response served as text/html` | The response body starts with `{` or `[` and the response content type contains `html`. |
| `empty body with JSON content-type` | A `200` response with a JSON content type and no body. |

Each finding names the host and normalized path and counts the entries. On a capture that kept its bodies, these are real defects, and they matter: a
server that gets JSON labelled as plain text may parse it differently, and a client that gets JSON labelled as HTML may refuse to read it.

If the capture was saved without response bodies (`validate` shows `response bodies: 0%`), the check for an empty body fires on every `200` response with a JSON content type. Ignore it for such a capture.

## Required headers

A configuration file can name headers that every request to a host must carry:

```yaml
required_headers:
  - host: "api.shop.example"
    headers: ["X-App-Version"]
```

```bash
wiretrail checkout.har checks --config config/wiretrail.yaml
```

<!-- output: checks-config.txt from="[missing-header] api.shop.example /v1/track" until="[content-type] api.shop.example /v1/legacy/status" -->
```text
[missing-header] api.shop.example /v1/track
  missing required header: X-App-Version (3 entries)
```

`host` is a glob. Header names ignore case. A finding appears for each header that a matching request lacks.
Without a configuration file, `checks` runs only the content-type checks.

`required_headers` is for `checks`. The `rules:` list in the same file has a similar `require_headers` key, which
belongs to [`rules`](rules.md) and adds matchers by path, method and status.

## JSON

```bash
wiretrail checkout.har checks --json | jq '.result.findings[0]'
```

<!-- output: checks-json.txt -->
```text
{
  "rule": "content-type",
  "host": "api.shop.example",
  "norm_path": "/v1/track",
  "detail": "request JSON body without application/json content-type",
  "entry_ids": [
    "e000093",
    "e000094",
    "e000095"
  ]
}
```

## Exit code

`checks` exits `1` when it reports any finding and `0` when it reports none.

## Related

- [Rules and rule packs](rules.md) for conventions with severities.
- [The configuration file](config-file.md) describes `required_headers`.
- [Show an entry](../inspect/show-entry.md) opens a flagged request.
