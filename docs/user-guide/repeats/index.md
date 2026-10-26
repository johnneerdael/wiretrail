# Repeats

Many network problems are the same request made too many times, or at the wrong time. These commands
find calls that repeat and say what kind of repeat it is. The right fix depends on that: a retry that
backs off is healthy, and a heartbeat that nothing needs is waste.

| Command | The question | Page |
|---|---|---|
| `duplicates` | Which identical calls were made more than once? | [Duplicates](duplicates.md) |
| `retries` | Which calls were repeated after a failure, and how fast? | [Retries](retries.md) |
| `storms` | Where did a lot of calls land in a short window? | [Storms](storms.md) |
| `pagination` | Is the app paging in a loop, or asking once per item (N+1)? | [Pagination and N+1](pagination.md) |
| `rate-limit` | Did the app keep calling after the server said to wait? | [Rate limits](rate-limits.md) |
| `diff` | What differs between calls that look the same? | [What varies between calls](diff.md) |

## Which one to use

- **The same call, many times, spread over the session.** Start with `duplicates`, then `diff`.
- **The same call again right after it failed.** `retries`. A 401 does not count: see [Retries](retries.md).
- **A lot of calls at once.** `storms`. If the calls differ only by an id in the path, it is an N+1: `pagination`.
- **Calls that ignore a 429.** `rate-limit`.

You rarely need to pick. `summary`, `diagnose` and `auto` run the checks for errors, auth, rate limits, retries, storms, duplicates, redirects and slow calls, and
rank what they find. They do not run `pagination`, so an N+1 is never a finding of its own there, and `diff` is only the follow-up they suggest
for repeated identical calls. Come here to read a result in detail.

## Recipes

- [Remove redundant calls](recipes/remove-redundant-calls.md) follows a heartbeat that nothing needs.
- [Spot an N+1](recipes/spot-an-n-plus-one.md) follows a per-item fan-out to the burst it causes.

## Related

- [Analyze](../analyze/index.md) for the ranked overview.
- [Concepts: entries, ids and endpoints](../concepts/entries-ids-endpoints.md) explains how calls are grouped.
