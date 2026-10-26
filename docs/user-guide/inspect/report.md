# Report

`report` writes a markdown document from the main analyses: the size of the capture, the subsystems, the
repeated calls, the errors, the redirect storms and the slowest requests. It is what you attach to a ticket or paste into a
write-up.

```bash
wiretrail checkout.har report > dossier.md
```

## What is in it

<!-- output: rep-default.txt from="# wiretrail report" until="## Subsystems" -->
```text
# wiretrail report

- Creator: WireTrail demo generator (synthetic) 1
- HAR version: 1.2
- Entries: 99
- Window: 70.0s

## Executive Summary

99 requests after filter, 16 error responses, 10 duplicate groups in the top list.
```

Then, in order:

| Section | What it holds |
|---|---|
| `Subsystems` | A table of subsystems with request counts, time windows, errors and duplicates. See [Hosts, subsystems and endpoints](../analyze/inventory.md#subsystems). |
| `Duplicate Index` | The [duplicate groups](../repeats/duplicates.md), with `(retry pattern)` where it applies. |
| `Errors` | One line per [error group](../failures/errors.md): count, status, route and the server's message. |
| `Redirect Storms` | Only [redirect groups](../failures/redirects.md) of five or more. Absent when there are none. |
| `Slowest Requests` | The longest calls with their [bottleneck](../failures/slowest.md). |

```bash
wiretrail checkout.har report
```

<!-- output: rep-default.txt from="## Errors" until="## Slowest Requests" -->
```text
## Errors

- 4x [503] `POST api.shop.example/v1/cart/sync` — inventory service unavailable
- 4x [429] `GET search.shop.example/v1/suggest` — too many requests
- 2x [401] `GET api.shop.example/v1/cart` — JWT expired
- 1x [502] `GET api.shop.example/v1/inventory/{id}` — bad gateway
- 1x [500] `POST api.shop.example/v1/cart/sync` — internal server error
- 1x [402] `POST payments.shop.example/v1/charges` — Your card was declined.
- 1x [401] `GET api.shop.example/v1/auth/session` — JWT expired
- 1x [401] `GET api.shop.example/v1/notifications` — JWT expired
- 1x [0] `GET cdn.shop.example/img/banner.webp`

## Redirect Storms

- 6x [302] `GET cdn.shop.example/img/hero.png`
```

A section with nothing in it is left out, apart from `Subsystems`.

The report is a summary, not a diagnosis: it does not include the ranked findings. Run [`diagnose`](../analyze/diagnose.md) for
those, and paste both.

## Shape it

`--filter` limits every section to part of the capture, except the four lines at the top (`Creator`, `HAR version`, `Entries` and `Window`), which always describe the whole file. `--top` bounds each list, and `--config` names subsystems.
Here, with ownership rules and three rows per list:

```bash
wiretrail checkout.har report --config config/ownership-only.yaml --top 3
```

<!-- output: rep-config.txt from="## Subsystems" until="## Duplicate Index" -->
```text
## Subsystems

| Subsystem | Requests | Window | Errors | Dups |
|---|---:|---|---:|---:|
| Storefront API | 63 | 900ms - 69.8s | 10 | 53 |
| cdn.shop.example | 15 | 330ms - 36.4s | 1 | 8 |
| search.shop.example | 12 | 9.0s - 58.0s | 4 | 2 |
```

## As JSON

`--json` wraps the same markdown in the usual envelope, as one string:

```bash
wiretrail checkout.har report --json | jq -r '.result.markdown' | head -8
```

<!-- output: rep-json.txt -->
```text
# wiretrail report

- Creator: WireTrail demo generator (synthetic) 1
- HAR version: 1.2
- Entries: 99
- Window: 70.0s

## Executive Summary
```

## What it masks

The report holds no headers or bodies. Its error lines use only the message text from a response body, as the server sent it. It still contains
host names, paths and server messages, so read it once before you send it. `--unsafe-include-secrets` is accepted, and it
changes nothing in a report.

## Exit code

`report` always exits `0`.

## Related

- [Handoff](handoff.md) lists the failing calls with what a backend team needs.
- [Diagnose](../analyze/diagnose.md) adds the ranked findings.
- [Hand a failure to the backend team](recipes/hand-a-failure-to-the-backend-team.md) uses both.
