# curl

`curl` writes a `curl` command that repeats a request from the capture, with secrets masked, and labels each
command **SAFE** or **UNSAFE** by what replaying it could do. Use it to reproduce a request by hand, or to put
a runnable example in a ticket.

```bash
wiretrail checkout.har curl e000010
```

<!-- output: curl-get.txt from="# e000010" until="next useful" -->
```text
# e000010 [SAFE: safe]
curl -X GET 'https://api.shop.example/v1/config' \
  -H 'accept: application/json, text/plain, */*' \
  -H 'accept-encoding: gzip, deflate, br' \
  -H 'user-agent: Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36' \
  -H 'origin: https://shop.example' \
  -H 'referer: https://shop.example/' \
  -H 'x-app-version: 4.12.0'
```

## One entry or many

With an id, `curl` writes that entry. With none, it writes every entry that matches `--filter`, up to `--top`,
which defaults to 10:

```bash
wiretrail checkout.har curl --filter "status:>=500" --top 2
```

<!-- output: curl-many.txt from="# e000054" until="next useful" -->
```text
# e000054 [SAFE: safe]
curl -X GET 'https://api.shop.example/v1/inventory/1042?store=ams' \
  -H 'accept: application/json, text/plain, */*' \
  -H 'accept-encoding: gzip, deflate, br' \
  -H 'user-agent: Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36' \
  -H 'origin: https://shop.example' \
  -H 'referer: https://shop.example/' \
  -H 'authorization: <redacted>' \
  -H 'x-app-version: 4.12.0'

# e000069 [UNSAFE: mutating method POST]
curl -X POST 'https://api.shop.example/v1/cart/sync' \
  -H 'accept: application/json, text/plain, */*' \
  -H 'accept-encoding: gzip, deflate, br' \
  -H 'user-agent: Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36' \
  -H 'origin: https://shop.example' \
  -H 'referer: https://shop.example/' \
  -H 'authorization: <redacted>' \
  -H 'x-app-version: 4.12.0' \
  -H 'content-type: application/json' \
  -H 'content-length: 48' \
  --data '{"cart":"c_55d1","items":[{"qty":1,"sku":1042}]}'
```

## The safety label

Each command has a label that says what replaying it could do:

| Label | When |
|---|---|
| `UNSAFE: payment/order endpoint` | The path contains `payment`, `pay`, `order`, `checkout`, `charge`, `refund` or `subscription`. |
| `UNSAFE: mutating method POST` | The method is POST, PUT, PATCH or DELETE, and the path has none of those words. |
| `SAFE: safe` | Anything else. |

```bash
wiretrail checkout.har curl --filter "path:/v1/charges method:post" --top 1
```

<!-- output: curl-pay.txt from="# e000087" until="next useful" -->
```text
# e000087 [UNSAFE: payment/order endpoint]
curl -X POST 'https://payments.shop.example/v1/charges' \
  -H 'accept: application/json, text/plain, */*' \
  -H 'accept-encoding: gzip, deflate, br' \
  -H 'user-agent: Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36' \
  -H 'origin: https://shop.example' \
  -H 'referer: https://shop.example/' \
  -H 'authorization: <redacted>' \
  -H 'content-type: application/json' \
  -H 'content-length: 49' \
  --data '{"amount":8900,"currency":"EUR","order":"o_4410"}'
```

The path test is a substring test, so `/v1/payload` counts as a payment path. The label is a hint and not a guarantee:
a GET can change state on a badly built server. **Never replay an UNSAFE command against a production system** unless you mean to.

## What the command holds

The command has the method, the URL, every request header and the body. Four things are different from the capture:

- **Masking.** The `Authorization` header, cookies and other known secrets are `<redacted>`. So are secret-looking path
  and query values, and secret JSON keys in the body. Only a JSON body is masked: a form-encoded or plain-text body is copied as it is.
- **No HTTP/2 pseudo-headers.** Headers that start with `:` such as `:authority` cannot be sent with `curl -H`, so they are left out.
  The entry `e000011` carried four of them:

    ```bash
    wiretrail checkout.har curl e000011
    ```

    <!-- output: curl-pseudo.txt from="# e000011" until="next useful" -->
    ```text
    # e000011 [SAFE: safe]
    curl -X GET 'https://api.shop.example/v1/auth/session' \
      -H 'accept: application/json, text/plain, */*' \
      -H 'accept-encoding: gzip, deflate, br' \
      -H 'user-agent: Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36' \
      -H 'origin: https://shop.example' \
      -H 'referer: https://shop.example/' \
      -H 'authorization: <redacted>' \
      -H 'x-app-version: 4.12.0'
    ```

- **A body cut at 4,000 characters.** The cut is always applied, a `…` marks it, and line breaks and tabs in the body become spaces.
- **A JSON body rewritten.** With masking on, a JSON body is parsed and written back on one line with its keys in alphabetical order, so it can look different from the capture even where nothing was masked.

Because a masked body is shorter or longer than the original, check `content-length` before you run the command: the header
is copied from the capture, so it can disagree with the body you now send. `curl` does not decompress by default, so with
the `accept-encoding` header from a browser add `--compressed` to read the answer.

## With real credentials

`--unsafe-include-secrets` leaves the headers, the URL and the body content as captured. The command then contains live credentials, so keep it on your own machine. It does not lift the
4,000-character cut on the body, which `show-entry` does lift:

```bash
wiretrail checkout.har curl e000014 --unsafe-include-secrets
```

The label does not change.

## JSON

```bash
wiretrail checkout.har curl e000076 --json | jq '.result.commands[0] | {id, safe, label}'
```

<!-- output: curl-json.txt -->
```text
{
  "id": "e000076",
  "safe": false,
  "label": "mutating method POST"
}
```

The `command` field holds the command text, with line breaks.

## Exit code

`curl` always exits `0`, and `2` when the id does not exist.

## Related

- [Show an entry](show-entry.md) prints the same request for reading.
- [Handoff](handoff.md) includes this command for every failing request.
- [How redaction works](../concepts/redaction.md) says what is masked.
