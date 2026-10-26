# Your first triage

A shopper says the cart does not save and the page feels slow. You have a capture of the session,
`checkout.har`. This page works through it from the first command to the exact request to hand to a
backend team, and explains each line of output on the way.

Every block of output below is real: it was printed by WireTrail on the demo capture. You can
[download the capture](follow-along.md) and get the same output. The capture is synthetic, so the
shop and its servers do not exist, but nothing about the output is made up.

## 1. Ask for the summary

Give WireTrail a capture and no command, and it runs `summary`.

```bash
wiretrail checkout.har --top 5
```

`--top 5` keeps each list to five rows. The first block counts what is in the capture.

<!-- output: first-summary.txt from="entries:" until="top hosts" -->
```text
entries: 99 total, 99 after filter
duration (first start to last response): 70.0s

status classes:
  2xx: 76
  3xx: 7
  4xx: 9
  5xx: 6
  other: 1

resource types:
  analytics: 3
  api: 77
  document: 3
  media: 9
  other: 3
  static: 4
```

There are 99 requests over 70 seconds. Nine came back 4xx and six 5xx. The one in `other` has no
status at all: the request failed before the server answered. WireTrail counts all three kinds, 16
in all, as errors.

The end of the summary says where to look next.

<!-- output: first-summary.txt from="hints:" -->
```text
hints:
  - 12x duplicate calls: GET api.shop.example /v1/products/{id}/price
  - 16 error responses (4xx/5xx/failed)

recommended next steps:
  [HIGH] retries
         4 retries, final 500 on POST /v1/cart/sync — repeated retries did not recover
  [HIGH] errors --filter "host:api.shop.example"
         4x 503 on POST /v1/cart/sync — inventory service unavailable
  [HIGH] rate-limit
         calls during 429 cooldown on search.shop.example /v1/suggest — 4 429s, follow-ups before Retry-After elapsed
  [HIGH] auth
         suspicious token refresh on auth.shop.example — refresh succeeded but later calls reused the old token
  [MEDIUM] storms
         13 host calls/s burst to api.shop.example — burst of calls in a 1s window

next useful commands: auth · errors · rate-limit · retries · storms
```

Each recommendation has a severity, the command that investigates it, and a one-line reason. The
first one matches the shopper's report: something to do with `POST /v1/cart/sync` kept failing.

## 2. Rank everything with diagnose

The summary shows only as many recommendations as `--top` allows. `diagnose` prints the ranked list
on its own, with ten as the default, and with the evidence behind each finding.

```bash
wiretrail checkout.har diagnose
```

<!-- output: first-diagnose.txt from="[HIGH] retry-exhaustion" until="[HIGH] rate-limit-no-backoff" -->
```text
[HIGH] retry-exhaustion — 4 retries, final 500 on POST /v1/cart/sync
  repeated retries did not recover
  evidence: e000069, e000070, e000072, e000073, e000076
  -> wiretrail <file> retries

[HIGH] 5xx-cluster — 4x 503 on POST /v1/cart/sync
  inventory service unavailable
  evidence: e000069, e000070, e000072, e000073
  -> wiretrail <file> errors --filter "host:api.shop.example"
```

`evidence` lists entry ids. An id such as `e000076` names one request in the capture, in file order,
and it is the same in every command. The line that starts with `->` is the command that drills in,
with `<file>` standing for your capture. See [How findings are ranked](../concepts/how-findings-are-ranked.md)
for what puts a finding at the top.

## 3. Follow the top finding

```bash
wiretrail checkout.har retries --filter "path:/v1/cart/sync"
```

`--filter` narrows the analysis to matching requests. Here, only calls whose path is `/v1/cart/sync`.
[Filter entries](../analyze/filters.md) lists every clause.

<!-- output: first-retries.txt from="POST api.shop.example" -->
```text
POST api.shop.example/v1/cart/sync  (5 attempts, 4 retries, final 500)
  triggered by: 503, 500
  backoff gaps: 1.2s, 2.5s, 5.2s, 8.2s
  entries: e000069, e000070, e000072, e000073, e000076

next useful commands: errors · transitions · show-entry
```

The client sent the same call five times. Four got a 503 and the last got a 500. The gaps between
attempts grow, 1.2 s then 2.5 s then 5.2 s then 8.2 s, so the app does back off. It gave up after
about 17 seconds and the cart never saved. That is the shopper's problem. The statuses are 5xx,
which point at the server and not at the app, but a capture cannot show why the server failed.

## 4. Read the error

```bash
wiretrail checkout.har errors --filter "path:/v1/cart/sync"
```

<!-- output: first-errors.txt from="4x" -->
```text
   4x  [503] POST api.shop.example/v1/cart/sync
  message: inventory service unavailable
  code: upstream_unavailable
  correlation: req_fa6f840121, e1e598c1a6e1ae1d-AMS
  body: {"error":{"code":"upstream_unavailable","message":"inventory service unavailable"}}
  entries: e000069, e000070, e000072, e000073

   1x  [500] POST api.shop.example/v1/cart/sync
  message: internal server error
  code: internal_error
  correlation: req_1c776b9fff, 775f0aeadf5f3512-AMS
  body: {"error":{"code":"internal_error","message":"internal server error"}}
  entries: e000076

next useful commands: transitions · redirects · show-entry
```

`message` and `code` come from the response body. `correlation` lists the ids the server put in its
response headers, such as `x-request-id` and `cf-ray`, which the backend team can search for in their logs.
The `body` line is the first 200 characters of the body, with secret-looking JSON fields masked. All
four come from the first request in each group, so look at the others with `show-entry` if they could differ.

## 5. Look at the last request

```bash
wiretrail checkout.har show-entry e000076
```

<!-- output: first-entry.txt from="== wiretrail entry" until="next useful commands" -->
```text
== wiretrail entry e000076 ==
POST https://api.shop.example/v1/cart/sync  [500] Internal Server Error
host: api.shop.example  http: h2  type: api
server ip: 203.0.113.30
offset: 42100ms  duration: 192ms
request headers:
  accept: application/json, text/plain, */*
  accept-encoding: gzip, deflate, br
  user-agent: Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36
  origin: https://shop.example
  referer: https://shop.example/
  authorization: <redacted>
  x-app-version: 4.12.0
  content-type: application/json
  content-length: 48
response headers:
  content-type: application/json
  date: Tue, 20 Oct 2026 09:14:44 GMT
  server: shopfront
  x-request-id: req_1c776b9fff
  cf-ray: 775f0aeadf5f3512-AMS
request body: {"cart":"c_55d1","items":[{"qty":1,"sku":1042}]}
response body: {"error":{"code":"internal_error","message":"internal server error"}}
```

This is the request and the response, with each body cut at 2,000 characters, and the call's offset and
total duration. The timing phases are in `--json`. `authorization: <redacted>` shows that WireTrail masks
credentials by default. The server IP, the time offset and the correlation ids are what a backend
engineer needs to find this call in their own logs.

!!! warning "Redaction has limits"
    WireTrail masks the headers and JSON keys it knows are secret, and values that look like
    random tokens. It cannot recognize every secret. Read
    [How redaction works](../concepts/redaction.md) before you paste output into a ticket.

## 6. Use the exit code and the JSON

WireTrail exits with `1` when it reports findings, `0` when it finds nothing and `2` when it cannot
read its input. That makes most commands usable as a gate in a script. Nine of them, such as `timeline`, `slowest` and
`show-entry`, never report findings and always exit `0`: [Errors and exit codes](../reference/errors-and-exit-codes.md#when-each-command-returns-1) lists them.

```bash
wiretrail checkout.har errors --top 1 > /dev/null; echo "exit code: $?"
```

<!-- output: first-exit.txt -->
```text
exit code: 1
```

Add `--json` to get the same result as a stable JSON document, which `jq` can slice.

```bash
wiretrail checkout.har errors --filter "path:/v1/cart/sync" --json | jq '.result.groups[] | {status, count, error_code, entry_ids}'
```

<!-- output: first-json.txt -->
```text
{
  "status": 503,
  "count": 4,
  "error_code": "upstream_unavailable",
  "entry_ids": [
    "e000069",
    "e000070",
    "e000072",
    "e000073"
  ]
}
{
  "status": 500,
  "count": 1,
  "error_code": "internal_error",
  "entry_ids": [
    "e000076"
  ]
}
```

The entry ids from JSON go straight into `show-entry` or `curl`, so a script can follow a finding
the way you followed it by hand.

## What you learned

- `wiretrail capture.har` prints a summary that ends in ranked, copy-pasteable next steps.
- `diagnose` ranks every finding and names its evidence and the command that drills in.
- `--filter` scopes a command to the requests you care about.
- `retries`, `errors` and `show-entry` take you from a finding to one request, with secrets masked.
- Exit code `1` means findings and `2` means unreadable input, and `--json` gives scripts the same data.

## Next steps

- The capture holds more than this one story. [Follow along with the demo captures](follow-along.md)
  lists everything planted in it and where to read about each.
- Not sure which command answers your question? [Choose the right command](choose-a-command.md).
- Skip the manual steps: [Auto](../analyze/auto.md) runs the follow-ups for you.
