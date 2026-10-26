# What a useful capture holds

Every WireTrail command reads some part of a HAR, and the file only helps if it holds that part. A capture can be
valid and still empty of what you need: no bodies, no credentials, no timings. `validate` reports exactly what a file holds.

```bash
wiretrail checkout.har validate
```

<!-- output: val-checkout.txt from="HAR 1.2" until="anomalies" -->
```text
HAR 1.2 via WireTrail demo generator (synthetic)  (99 entries)
with timings: 99% · response bodies: 81% · POST req bodies: 100%
auth headers: true · cookies: true
```

## What each part is for

| In the HAR | Which commands use it | What goes wrong without it |
|---|---|---|
| Request URL, method, status | All of them | Nothing works. This is the minimum. |
| Start time and duration | `timeline`, `storms`, `retries`, `cascade`, `startup`, `slowest`, `rate-limit`, `compare` | Order and timing are lost. A capture with unreadable start times gets offset `0`. |
| Timing phases (`dns`, `connect`, `ssl`, `send`, `wait`, `receive`) | `slowest`, `show-entry`, `report` | `slowest` labels everything `unknown`. |
| Response body | `errors` (message and code), `search`, `extract`, `jwt` in bodies | No error messages, nothing to search. |
| Request body | `diff` (body verdicts), `search`, `extract --target req` | `diff` cannot say whether calls differ. |
| Request headers (`Authorization`, `Cookie`) | `auth`, `jwt`, `rules`, `checks` | No token story. |
| Response headers (`Retry-After`, `X-RateLimit-*`, correlation ids) | `rate-limit`, `errors`, `handoff` | No cooldowns, no ids for the backend team. |
| `serverIPAddress` | `show-entry`, `handoff` | No server address in the hand-off. |

## What a stripped capture loses

A capture that was stripped of credentials and bodies, as many exporters do by default, still loads. `validate` says what is lost:

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

`sufficiency` is the list of commands whose results are limited. Treat an empty `jwt` or `search` result on such a capture as "cannot tell", and not as "nothing there".

## Settings that matter when you record

- **Save the content.** Choose the export that includes response bodies. Without them `errors` shows a status and nothing else.
- **Record from the start.** Switch recording on before the first request, and keep the log across page loads.
- **Keep the proxy's decryption on** for HTTPS, or the paths and bodies are missing.
- **Do not trim.** Some tools can limit body size. A truncated body can fail to parse as JSON, and `extract` skips it.

## What a HAR cannot hold

A HAR is what a browser or a proxy saw at the HTTP level. It does not hold packet loss, the full TLS certificate chain, client call stacks, DNS
resolver internals, or what a service worker or proxy did unless a tool wrote it into a custom field. WireTrail reports what is there and invents nothing.
[Limitations](../reference/limitations.md) covers what WireTrail itself does not do.

## Related

- [Validate a capture](../analyze/validate.md) explains each line of the report.
- [HAR support](../reference/har-support.md) lists the fields WireTrail reads.
- [From a browser](browsers.md) and [From a proxy or a phone](proxies-and-phones.md) say how to switch these on.
