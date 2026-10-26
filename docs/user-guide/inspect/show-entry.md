# Show an entry

`show-entry` prints one request and its response in full: the URL, the status, the timing, every header, and the
bodies. Credentials and secret-looking values are masked. It is the command to run when another command gives
you an entry id and you want to see the thing itself.

```bash
wiretrail checkout.har show-entry e000014
```

<!-- output: entry-oauth.txt from="== wiretrail entry" until="next useful" -->
```text
== wiretrail entry e000014 ==
POST https://auth.shop.example/oauth/token  [200] OK
host: auth.shop.example  http: h2  type: api
server ip: 203.0.113.31
offset: 1300ms  duration: 407ms
request headers:
  accept: application/json, text/plain, */*
  accept-encoding: gzip, deflate, br
  user-agent: Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36
  origin: https://shop.example
  referer: https://shop.example/
  content-type: application/json
  content-length: 89
response headers:
  content-type: application/json
  date: Tue, 20 Oct 2026 09:14:03 GMT
  server: shopfront
  x-request-id: req_32074e7281
  cf-ray: 543e056979de5c42-AMS
  set-cookie: <redacted>
request body: {"grant_type":"refresh_token","refresh_token":"<redacted>"}
response body: {"access_token":"<redacted>","expires_in":3600,"refresh_token":"<redacted>","token_type":"<redacted>"}
```

## Choose the entry

`show-entry` takes an entry id such as `e000014`, or a bare index. These three name the same entry:

```bash
wiretrail checkout.har show-entry e000014
wiretrail checkout.har show-entry 14
wiretrail checkout.har show-entry e14
```

An id that matches nothing stops with exit code `2`:

```bash
wiretrail checkout.har show-entry e999999
```

<!-- output: entry-bad.txt -->
```text
wiretrail: no entry with id or index 'e999999'
```

`--filter` has no effect here, because the entry is looked up by its id in the whole capture.

## Read the output

| Part | Meaning |
|---|---|
| `POST https://auth.shop.example/oauth/token  [200] OK` | Method, URL with secrets masked, status and status text. |
| `host`, `http`, `type` | The host, the HTTP version, and the [resource type](../analyze/summary.md#1-the-size-of-the-capture). |
| `server ip` | The address the server answered from, when the capture records it. |
| `offset`, `duration` | When the request started, from the start of the capture, and how long it took, in milliseconds. |
| `query` | The query parameters, one per line, with sensitive values masked. Shown when there are any. |
| `request headers`, `response headers` | Every header, in the order the capture has them. HTTP/2 pseudo-headers such as `:authority` are listed too. |
| `request body`, `response body` | The body on one line. In a JSON body, sensitive keys are masked. Any other body, such as a form or plain text, is printed as it is. |

A body is cut to **2,000 characters**, and line breaks become spaces. With `--unsafe-include-secrets` it is
neither cut nor masked.

## What is masked

In the example above:

- `set-cookie` is masked because it is one of the headers WireTrail always masks.
- In both bodies, the JSON keys `refresh_token` and `access_token` are masked, and so is `token_type`: any key whose name
  **contains** `token`, `secret`, `password` or a few similar words is masked, whatever its value.

A query value is masked when its name is one of `token`, `access_token`, `refresh_token`, `id_token`, `key`, `api_key`, `apikey`, `sig`, `signature`, `password` or `secret`, or when the
value looks like an opaque token. A path segment is masked by its shape only. This entry has a long query with credentials in it:

```bash
wiretrail checkout.har show-entry e000098
```

<!-- output: entry-receipt.txt from="GET https" until="request headers" -->
```text
GET https://api.shop.example/v1/receipts/9921/download?d=<redacted>&token=<redacted>&sig=<redacted>&page=1  [200] OK
host: api.shop.example  http: h2  type: other
server ip: 203.0.113.30
offset: 69800ms  duration: 153ms
query:
  d = <redacted>
  token = <redacted>
  sig = <redacted>
  page = 1
```

`token` and `sig` were masked by their names, `d` by the shape of its value, and `page=1` was not. A base64 blob in the path is masked too:

```bash
wiretrail checkout.har show-entry e000008
```

<!-- output: entry-blob.txt from="GET https" until="host:" -->
```text
GET https://cdn.shop.example/cfg/<redacted>/manifest.json  [200] OK
```

!!! warning "Masking works by name and by shape"
    A header outside the fixed list is shown as it is, unless its value looks like a random token. A JWT contains
    dots, which the "looks like a token" test does not allow, so a JWT in a custom header such as `x-device-token`
    is printed in full. A form-encoded body is not masked at all. Read [How redaction works](../concepts/redaction.md) before you paste this output anywhere.

## Show everything

`--unsafe-include-secrets` turns masking off and also removes the 2,000-character limit. Use it only on your own machine,
with a capture you may read, and never paste the result into a ticket.

```bash
wiretrail checkout.har show-entry e000014 --unsafe-include-secrets
```

<!-- output: entry-unsafe.txt from="request body" until="next useful" -->
```text
request body: {"grant_type":"refresh_token","refresh_token":"rt_demo_7f3a9c1e7f3a9c1e7f3a9c1e7f3a9c1e"}
response body: {"access_token":"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJodHRwczovL2F1dGguc2hvcC5leGFtcGxlLyIsImF1ZCI6InNob3Atd2ViIiwic3ViIjoidXNlcl84ZjIxYzQiLCJpYXQiOjE3OTI0ODc2NDMsImV4cCI6MTc5MjQ5MTI0M30.loE_3tvaxiDj8FMqqUqivLUS9FtgizVf0ZpLeV4f4b8","refresh_token":"rt_demo_7f3a9c1e7f3a9c1e7f3a9c1e7f3a9c1e","token_type":"Bearer","expires_in":3600}
```

The tokens here are made up. In a real capture they would be live credentials.

## JSON

```bash
wiretrail checkout.har show-entry e000076 --json | jq '.result | {id, method, status, norm_path, timings, req_body_snippet, resp_body_snippet}'
```

<!-- output: entry-json.txt -->
```text
{
  "id": "e000076",
  "method": "POST",
  "status": 500,
  "norm_path": "/v1/cart/sync",
  "timings": {
    "blocked": null,
    "dns": null,
    "connect": null,
    "ssl": null,
    "send": 1.0,
    "wait": 187.0,
    "receive": 4.0
  },
  "req_body_snippet": "{\"cart\":\"c_55d1\",\"items\":[{\"qty\":1,\"sku\":1042}]}",
  "resp_body_snippet": "{\"error\":{\"code\":\"internal_error\",\"message\":\"internal server error\"}}"
}
```

A phase the capture marks as "does not apply" is `null`. The result also holds the full URL, every header
as a `[name, value]` pair, the server IP and the correlation ids.

## Exit code

`show-entry` exits `0` when it finds the entry and `2` when it does not.

## Related

- [curl](curl.md) writes the same request as a command.
- [How redaction works](../concepts/redaction.md) lists what is and is not masked.
- [Errors](../failures/errors.md) shows entry ids with the server's message.
