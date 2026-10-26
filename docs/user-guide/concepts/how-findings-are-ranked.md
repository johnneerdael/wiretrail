# How findings are ranked

`summary`, `diagnose` and `auto` all print the same list of ranked findings. One piece of code produces it, so the three cannot disagree. This page says
what puts a finding on the list and what puts it near the top.

## The ten kinds

A finding is raised when a rule finds a pattern in the capture, after `--filter` has been applied.

| Kind | Severity | Raised when | Drill-down |
|---|---|---|---|
| `5xx-cluster` | high | An error group of the same host, method, path and 5xx status has **3 or more** responses. | `errors --filter "host:…"` |
| `retry-exhaustion` | high | A group of the same call was retried **3 or more** times and its last status is **not** a 2xx. | `retries` |
| `rate-limit-no-backoff` | high | A call to a route started during the `Retry-After` period of a limited response on that route: a 429, or a response with `x-ratelimit-remaining: 0`. `Retry-After` must be a number of seconds. | `rate-limit` |
| `token-refresh-race` | high | A token refresh failed, or it succeeded, a new token then appeared, and a later call still sent a token seen before the refresh. | `auth` |
| `request-storm` | medium | A host or an endpoint received **10 or more** calls in a 1-second window. | `storms` |
| `wasteful-duplicates` | medium | A duplicate group has **10 or more** calls and no call that follows a 5xx, a 429 or a request with no status. | `diff --filter "host:… path:…"` |
| `4xx` | medium | Any 4xx error group, including a single response. | `errors` |
| `auth-failures` | medium | Any 401 or 403, counted across all hosts. | `auth` |
| `redirect-storm` | low | A redirect group of **5 or more** responses. | `redirects` |
| `slow-backend` | low | The single slowest call took more than a second **and** its largest phase is the server wait (TTFB). | `slowest` |

The thresholds are fixed. `--window-ms`, `--min-count` and the other flags of the individual commands do not change them.

`critical` exists as a level, and no rule produces it.

## What puts one above another

1. **Severity**: high before medium before low.
2. **The amount of evidence**: a finding with more entry ids ranks above one with fewer.
3. **The kind's name**, alphabetically, to break a tie.

The sorted list is then cut to `--top`, which is 10 unless you change it, so on a busy capture the low-severity findings are the ones cut.

`--top` acts a second time, earlier. Each check that feeds the list (errors, retries, storms and the others) is asked for only its first `--top` groups, largest first, before any finding
is built from them. A capture with many 4xx groups can push a smaller 5xx cluster out at that point, and the cluster is never ranked. See
[Limitations](../reference/limitations.md#-top-cuts-each-check-before-the-findings-are-ranked).

## What it means for a severity to be "high"

A severity is a judgement of how likely a pattern is to hurt, based on the kind. A server that answers one call with a 503 four times is more likely to
mean a real outage than a call that is made twelve times. It is not a measure of impact: a `high` retry loop on a background call may matter less to your users than a `low`
slow call on the checkout.

A finding is **evidence of a pattern**, not a diagnosis. `token-refresh-race` means the capture fits a race. It does not prove one.

## What is not ranked

Some real problems raise no finding, because no rule covers them:

- A single 500, or a handful of them below the cluster threshold.
- A failed request with no status, unless it also shows up in a rule.
- A slow call whose time went to the network and not the server.
- A heartbeat of fewer than ten calls, or an N+1 below the burst threshold.
- Anything [`rules`](../compare/rules.md) or [`checks`](../compare/checks.md) would report. They keep their own list.

A clean `diagnose` therefore means "none of these ten rules fired". [Errors](../failures/errors.md) and [Slowest calls](../failures/slowest.md) are still worth a look.

## Evidence and the next step

Each finding carries the entry ids that triggered it, so you can open any of them, and the command that investigates it. When the finding is about one host or
route, the command carries a `--filter` that scopes to it. `auto` runs those commands for the findings at or above its severity floor.

## Related

- [Diagnose](../analyze/diagnose.md) prints the list.
- [Auto](../analyze/auto.md) runs the drill-downs.
- [Summary](../analyze/summary.md) ends with the same list.
