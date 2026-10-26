# Validate a capture

`validate` checks whether a capture holds what the analysis needs. A capture can be valid HAR and
still be nearly useless: a browser that was asked not to save bodies, an exporter that stripped the
headers, a recording that started too late. `validate` tells you that before you trust an empty result.

```bash
wiretrail checkout.har validate
```

<!-- output: val-checkout.txt from="HAR 1.2" until="next useful" -->
```text
HAR 1.2 via WireTrail demo generator (synthetic)  (99 entries)
with timings: 99% · response bodies: 81% · POST req bodies: 100%
auth headers: true · cookies: true

anomalies:
  status-0: 1
```

The first line is the HAR version, the program that wrote the file, and the number of entries.

| Line | Meaning |
|---|---|
| `with timings` | The share of entries that report a send, wait or receive time above zero. [Slowest calls](../failures/slowest.md) and [Startup profile](../failures/startup.md) need these. |
| `response bodies` | The share of entries with a response body. [Errors](../failures/errors.md), [Search bodies](../inspect/search.md) and [Extract a field](../inspect/extract.md) need these. |
| `POST req bodies` | Of the POST, PUT and PATCH requests, the share that has a request body. [What varies between calls](../repeats/diff.md) needs these. |
| `auth headers`, `cookies` | Whether any request carries an `Authorization` or a `Cookie` header. |
| `anomalies` | Entries that look broken, with a count. |

The share with timings is 99%, not 100%, because one request has no send, wait or receive time. It is
the failed image request, which spent 30 ms on `dns` and 1,870 ms on `connect` and then failed.

## Anomalies

| Anomaly | Meaning |
|---|---|
| `status-0` | The request has status `0`: it failed before a response arrived. This is often a real failure, not a defect in the file. |
| `missing-method` | An entry with no HTTP method. |
| `zero-duration-with-body` | A request with a response body and a duration of zero. |
| `negative-size` | A body size below `-1`. HAR uses `-1` for "unknown", so lower values are wrong. |

`validate` exits `1` when it reports any anomaly, so a single failed request makes it exit `1`.
Use the exit code as "something to read", not "the file is broken".

## A capture that has been stripped

Many tools remove credentials and bodies on export. `validate` recognizes that and says what is lost:

```bash
wiretrail sanitized.har validate
```

<!-- output: val-sanitized.txt from="HAR 1.2" until="next useful" -->
```text
HAR 1.2 via WireTrail demo generator (synthetic, sanitized)  (99 entries)
with timings: 99% · response bodies: 0% · POST req bodies: 0%
auth headers: false · cookies: false
sanitized: yes (no auth/cookies and few response bodies)

anomalies:
  status-0: 1

sufficiency:
  - few/no response bodies captured — `errors`/`search`/`extract` limited
  - no Authorization headers — `auth`/`jwt` limited
  - no request bodies on POST/PUT/PATCH — `diff` body verdicts limited
```

`sanitized: yes` appears when there are no `Authorization` or `Cookie` headers and fewer than 10% of
responses have a body. `sufficiency` is a separate list, and it does not depend on `sanitized`. It
prints one note for each of four conditions that holds:

- fewer than 10% of responses have a body: `errors`, `search` and `extract` are limited;
- no request carries an `Authorization` header: `auth` and `jwt` are limited;
- there are POST, PUT or PATCH requests and none has a body: `diff` body verdicts are limited;
- no request has any send, wait or receive time: `slowest` and `startup` are limited.

The notes keep you from reading an empty `jwt` result as "no tokens". The analysis still runs on what is
there.

## JSON

```bash
wiretrail checkout.har validate --json | jq '.result | {pct_with_timings, pct_with_resp_body, sanitized, anomalies}'
```

<!-- output: val-json.txt -->
```text
{
  "pct_with_timings": 0.98989898989899,
  "pct_with_resp_body": 0.8080808080808081,
  "sanitized": false,
  "anomalies": [
    {
      "kind": "status-0",
      "count": 1
    }
  ]
}
```

The shares are fractions between 0 and 1 in JSON, and percentages in the text.

## Related

- [What a useful capture holds](../capture/what-a-capture-needs.md) says how to save a capture with everything in it.
- [Handle captures safely](../capture/handle-captures-safely.md) for stripping a capture before you share it.
- [Triage an unfamiliar capture](recipes/triage-an-unfamiliar-capture.md) starts with `validate`.
