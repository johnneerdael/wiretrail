# How redaction works

A HAR is full of credentials, so WireTrail masks what it prints. The masking is **pattern-based**: it recognizes secrets by the name of
the header or the key they sit in, and by the shape of the value. It does not understand what a value means. That makes it reliable for the
common cases and blind to anything outside the patterns, and this page lists both.

!!! warning "Masking is not a guarantee"
    Output is safe to paste into a ticket only if every secret in it is one of the kinds WireTrail recognizes. Several common places are not covered,
    and they are listed below with real output. Read what you are about to share.

## The three rules

**1. By header name.** These request and response headers are always masked, whatever they hold: `authorization`, `cookie`, `set-cookie`,
`proxy-authorization`, `x-api-key`, `x-auth-token` and `x-amz-security-token`.

```bash
wiretrail checkout.har show-entry e000076 | grep 'authorization'
```

<!-- output: red-auth.txt from="authorization" -->
```text
  authorization: <redacted>
```

Three more headers hold URLs and are masked as URLs: `location`, `referer` and `content-location`. A relative value such as `/cb?token=…` cannot be read as a URL, so only rule 3 is applied to it, and
the query key names in it are not checked.

**2. By key name in JSON.** In a JSON body, any key whose name **contains** one of `password`, `token`, `secret`, `authorization`, `access_token`,
`refresh_token`, `id_token`, `api_key`, `apikey` or `client_secret` has its value replaced, at any depth, whatever the value is. The match
ignores case and is a substring match, so `token_type` is masked too.

```bash
wiretrail checkout.har show-entry e000014 | grep 'request body'
```

<!-- output: red-body.txt from="request body" -->
```text
request body: {"grant_type":"refresh_token","refresh_token":"<redacted>"}
```

In a URL query, the keys `token`, `access_token`, `refresh_token`, `id_token`, `key`, `api_key`, `apikey`, `sig`, `signature`, `password` and
`secret` are masked.

**3. By shape.** A value that looks like a random token is masked in a path segment, a query value, a header value or, in some commands,
a body. WireTrail calls such a value **opaque**. A path segment or a query value is tested as a whole. A header value, and a `search` snippet, is first split at spaces and at `; , & = / ? :`, quotes
and brackets, and each piece is tested on its own, so a base64 value that contains `=` or `/` can slip through. A string is opaque when it is at least 16 characters long, is not all hexadecimal and not a UUID, and
either contains a `%`, or is in the base64 alphabet (letters, digits, `+`, `/`, `=`, `_`, `-`) and has **either** 24 or more characters with a
`+`, `/` or `=` and a digit, **or** 32 or more with a digit, an upper-case and a lower-case letter. A dot is **not** in the alphabet.

```bash
wiretrail checkout.har show-entry e000098 | grep -E '^GET|^  (d|token|sig|page) ='
```

<!-- output: red-query.txt from="GET https" -->
```text
GET https://api.shop.example/v1/receipts/9921/download?d=<redacted>&token=<redacted>&sig=<redacted>&page=1  [200] OK
  d = <redacted>
  token = <redacted>
  sig = <redacted>
  page = 1
```

`d`, `token` and `sig` are masked, the first by shape and the other two by name. `page` is not. A blob in the path is masked by shape:

```bash
wiretrail checkout.har show-entry e000008 | grep '^GET'
```

<!-- output: red-path.txt from="GET https" -->
```text
GET https://cdn.shop.example/cfg/<redacted>/manifest.json  [200] OK
```

## What each command masks

| Command | What can carry a secret | How it is protected |
|---|---|---|
| `show-entry`, `curl`, `handoff` | URL, query, headers, bodies | The three rules. **Only JSON bodies** get rule 2. Other bodies are printed as they are. |
| `errors` | `body:` excerpt, `message`, `code`, correlation ids | The excerpt follows rule 2 for JSON. `message`, `code` and the correlation ids are copied from the body and the headers **unmasked**. |
| `diagnose`, `auto`, `summary` (recommended next steps) | The detail line of a finding | The server's error `message` is copied from the body **unmasked**. |
| `report` | Error messages | The messages are copied unmasked. |
| `search` | A window from 40 bytes before a match to 40 bytes after its start | Rule 3 only, by chunk. **No** header or key-name rule. |
| `extract` | The extracted value | Rule 3 only. **No** key-name rule. |
| `diff` | Query values that vary | Rules 2 and 3 for the query. No body content is printed. |
| `jwt` | Claims | The token and its signature are never printed. `iss` and `aud` are printed. `sub` is hashed. `--json --unsafe-include-secrets` adds `raw_token`. |
| `summary` (text), and the `fingerprint` field of `duplicates --json` and `retries --json` | The query pairs of a repeated call, in `top duplicate calls` and `hints` | **None.** Query values are printed as they are. |
| `hosts`, `subsystems`, `endpoints`, `timeline`, `slowest`, `startup`, `cascade`, `storms`, `duplicates` (text), `retries` (text), `transitions`, `redirects`, `pagination`, `rate-limit`, `auth`, `export` | Host and normalized path | Opaque and numeric path segments become `{blob}` and `{id}`. No request headers, bodies or query values are printed. `rate-limit` prints the `retry-after` and `x-ratelimit-*` response headers, and `export` and `timeline --json` print one correlation id. |

`--unsafe-include-secrets` turns masking off for `show-entry`, `curl`, `handoff`, `errors`, `search`, `extract`, `diff`, `jwt` and `auto` (for its `errors` and `diff` drill-downs). It does not
change commands that print no secrets, and it changes nothing in `report`.

## What gets through

Each of these was run and printed on a file in `assets/downloads/quirks/`, so you can reproduce it.

**A JWT in a header with another name.** A JWT has dots, and rule 3 does not allow them. Only the headers in rule 1 are masked by name, so a token in `x-device-token`
is printed in full by `show-entry`, `curl` and `handoff`. Here are the first characters of one:

```bash
wiretrail checkout.har show-entry e000043 | grep 'x-device-token' | cut -c1-96
```

<!-- output: red-device.txt from="x-device-token" -->
```text
  x-device-token: eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJodHRwczovL2RldmljZS5zaG9wLmV4Y
```

**A JWT in a field that `extract` reads.** `extract '$.access_token'` prints the whole token, for the same reason. See [Extract a field](../inspect/extract.md#what-it-masks).

**A form-encoded body.** The key-name rule works on JSON. A body in `application/x-www-form-urlencoded`, which is how many sign-in and token calls are sent, is
not parsed, and nothing in it is masked:

```bash
wiretrail quirks/unmasked-places.har show-entry e000002 | grep 'request body'
```

<!-- output: red-form.txt from="request body" -->
```text
request body: user=ann&password=hunter2&grant_type=password
```

`curl` and `handoff` print the same text in their `--data` line:

```bash
wiretrail quirks/unmasked-places.har curl e000002 | grep 'data'
```

<!-- output: red-form-curl.txt from="--data" -->
```text
  --data 'user=ann&password=hunter2&grant_type=password'
```

**A short secret in a query, in the duplicate list.** The fingerprint of a repeated call includes its query values, and `summary` prints fingerprints:

```bash
wiretrail quirks/unmasked-places.har summary | grep -A1 'top duplicate'
```

<!-- output: red-fingerprint.txt from="top duplicate calls" -->
```text
top duplicate calls:
     2x  GET api.test.example /v1/ping session=s3cr3t-short-value  (e000000)
```

The key is `session`, which is not on the list, and the value is too short to look random. `duplicates --json` and `retries --json` carry the same fingerprint in their `fingerprint` field.

**A secret in a server's message.** `errors` copies the message out of the body before it applies any masking. `diagnose`, `auto` and the recommended next steps in `summary` show the same message as the detail of a finding:

```bash
wiretrail quirks/unmasked-places.har errors | grep 'message:'
```

<!-- output: red-message.txt from="message:" -->
```text
  message: invalid token abc123 for ann
```

**A short secret that `search` finds**, and a short value that `extract` returns. See [Search bodies](../inspect/search.md#what-it-masks).

**A secret in a path that does not look random.** `/reset/ab12cd` stays as it is: numeric segments become `{id}`, but a short mixed string is kept.

## What it masks that it did not need to

The key-name rule is a substring match, so it also hides harmless values. `token_type` in the sign-in response above is masked although it holds
`Bearer`. That is deliberate: a false mask costs you a value, and a false pass costs you a credential.

## Using it safely

1. **Treat the output as sensitive until you have read it.** Look at it the way you would look at a log you are about to send.
2. **Prefer the commands that print the least.** `report`, `diagnose` and `summary` print counts and the server's messages. `show-entry`, `curl`, `handoff`, `search` and `extract` print request content.
3. **Search your own output for the words that should not be in it**: `bearer`, `token`, `secret`, `password`, a real user's name.
4. **Strip the capture before you share the file.** WireTrail does not do this for you. See [Handle captures safely](../capture/handle-captures-safely.md).
5. **Give an agent a stripped capture** when you can, because it can quote anything it reads.

## Related

- [Handle captures safely](../capture/handle-captures-safely.md) for what to do about it.
- [Limitations](../reference/limitations.md#redaction-is-pattern-based) lists the same gaps with the other limitations.
- [Show an entry](../inspect/show-entry.md) is the command whose masking is the most complete.
