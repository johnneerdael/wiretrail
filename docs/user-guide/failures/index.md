# Failures

These commands find what went wrong in a capture: requests that failed, were redirected over and over, took too
long, or were rejected for their credentials. Each one explains the failure in the terms the server used, so you
can tell a bug from a decision.

## Failed and slow requests

| Command | The question | Page |
|---|---|---|
| `errors` | Which calls returned an error, and what did the server say? | [Errors](errors.md) |
| `redirects` | Which routes redirect again and again, or send the client to another host? | [Redirects](redirects.md) |
| `transitions` | Which failed calls were followed by a success, and which by another failure? | [Status transitions](transitions.md) |
| `slowest` | Which calls were slowest, and which phase took the time? | [Slowest calls](slowest.md) |
| `startup` | What happened in the first seconds, and what ran in sequence? | [Startup profile](startup.md) |
| `cascade` | What failed first, and what failed after it? | [Cascade](cascade.md) |

## Auth

| Command | The question | Page |
|---|---|---|
| `auth` | Which calls were rejected, did the token change, did a refresh go wrong? | [Auth failures and refresh](auth.md) |
| `jwt` | What do the tokens say, and had they expired? | [JWTs](jwt.md) |

## A habit worth having

A failure in a capture is a **fact about one request**. The cause may be in the server, in the client, in
the network or in the capture itself. Before you act, read the response body with `errors`, open one request
with [`show-entry`](../inspect/show-entry.md), and check [`validate`](../analyze/validate.md) that the capture has the
bodies and headers you are relying on.

## Recipes

- [Find the first failure](recipes/find-the-first-failure.md) starts from the earliest error and works outward.
- [Why a call is slow](recipes/why-a-call-is-slow.md) follows one slow call to its phase and its owner.
- [Debug a token refresh race](recipes/debug-a-token-refresh-race.md) confirms a refresh race with the tokens.

## Related

- [Repeats](../repeats/index.md) covers calls that happen too often.
- [Inspect](../inspect/index.md) opens one request in full.
