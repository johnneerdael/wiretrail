# Extract a field

`extract` reads one value out of the JSON bodies of many requests and prints it next to the entry id. It
turns "what error code did each failure return?" into one command.

```bash
wiretrail checkout.har extract '$.error.message' --filter "status:>=400" --top 6
```

<!-- output: ext-msg.txt from="e000011" until="next useful" -->
```text
e000011  JWT expired
e000012  JWT expired
e000013  JWT expired
e000015  JWT expired
e000054  bad gateway
e000069  inventory service unavailable
```

## The path

`extract` takes a small JSON path. It supports:

| Form | Meaning |
|---|---|
| `$.error.message` | The key `message` inside the key `error`. The leading `$` is optional. |
| `items[0].sku` | The first element of the array `items`, then its `sku`. |
| `data.cart.items[*].sku` | Every element of an array, or every value of an object, with `[*]`. |

It does not support filters, comparisons or recursive descent (`..`). A path that matches nothing prints no
rows. A path that is not valid, such as `a[x]`, also prints no rows and no error.

```bash
wiretrail checkout.har extract '$.data.cart.items[*].sku'
```

<!-- output: ext-wild.txt from="e000084" until="next useful" -->
```text
e000084  1042
```

## Which body

By default `extract` reads **response** bodies. `--target req` reads request bodies:

```bash
wiretrail checkout.har extract '$.grant_type' --target req
```

<!-- output: ext-req.txt from="e000014" until="next useful" -->
```text
e000014  refresh_token
```

It skips any body that is empty or not valid JSON. `--filter` limits which entries are read, and `--top` limits the number of
values printed, default 10.

A string prints as it is, and any other value prints as JSON, so `1042` and `true` print as written.

## Count the values

A filter and a path together answer a counting question. The error codes of the server errors on one host:

```bash
wiretrail checkout.har extract '$.error.code' --filter "host:api.shop.example status:>=500"
```

<!-- output: ext-code.txt from="e000054" until="next useful" -->
```text
e000054  bad_gateway
e000069  upstream_unavailable
e000070  upstream_unavailable
e000072  upstream_unavailable
e000073  upstream_unavailable
e000076  internal_error
```

With `--json`, the same values arrive as a list that `jq` can group:

```bash
wiretrail checkout.har extract '$.error.code' --filter "status:>=500" --top 2 --json | jq '.result'
```

<!-- output: ext-json.txt -->
```text
{
  "values": [
    {
      "id": "e000054",
      "value": "bad_gateway"
    },
    {
      "id": "e000069",
      "value": "upstream_unavailable"
    }
  ]
}
```

## What it masks

!!! warning "Extract masks by shape, not by name"
    `extract` hides a value only when it **looks like a random token**. It never looks at the key. If you ask for a field
    that holds a short secret, you get the secret:

    ```bash
    wiretrail quirks/short-secrets.har extract '$.password'
    ```

    <!-- output: ext-secret.txt from="e000000" until="next useful" -->
    ```text
    e000000  hunter2
    ```

    A JWT is also not recognized as a random token, because it contains dots. Asking for the access token in the
    refresh response prints the whole token:

    ```bash
    wiretrail checkout.har extract '$.access_token'
    ```

    <!-- output: ext-jwt.txt from="e000014" until="next useful" -->
    ```text
    e000014  eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJodHRwczovL2F1dGguc2hvcC5leGFtcGxlLyIsImF1ZCI6InNob3Atd2ViIiwic3ViIjoidXNlcl84ZjIxYzQiLCJpYXQiOjE3OTI0ODc2NDMsImV4cCI6MTc5MjQ5MTI0M30.loE_3tvaxiDj8FMqqUqivLUS9FtgizVf0ZpLeV4f4b8
    ```

    Do not run `extract` on a credential field and paste the result. See [How redaction works](../concepts/redaction.md#what-each-command-masks).

## Exit code

`extract` always exits `0`.

## Related

- [Search bodies](search.md) finds text when you do not know the key.
- [Export](export.md) gives every entry's metadata as rows.
- [JSON output](../automate/json-output.md) for the structured form of every command.
