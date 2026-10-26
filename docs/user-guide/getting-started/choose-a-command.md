# Choose the right command

Start from the question you have. If you do not know where to begin, run `wiretrail capture.har auto`,
which prints the summary and then investigates the most serious findings for you.

## I want an overview

| Question | Command |
|---|---|
| What is in this capture, and where should I look first? | [`summary`](../analyze/summary.md) |
| Just investigate it for me | [`auto`](../analyze/auto.md) |
| Rank the problems, with the evidence for each | [`diagnose`](../analyze/diagnose.md) |
| Can this capture answer my question at all? | [`validate`](../analyze/validate.md) |
| Who talks to which servers? | [`hosts`](../analyze/inventory.md#hosts) |
| Which integration or team does a host belong to? | [`subsystems`](../analyze/inventory.md#subsystems) |
| Which endpoints were called, and how did they answer? | [`endpoints`](../analyze/inventory.md#endpoints) |
| What happened, in order? | [`timeline`](../analyze/timeline.md) |

## I think something is called too often

| Question | Command |
|---|---|
| Which identical calls repeat? | [`duplicates`](../repeats/duplicates.md) |
| Which calls were retried after a failure, and how fast? | [`retries`](../repeats/retries.md) |
| Where is a burst of calls in a short window? | [`storms`](../repeats/storms.md) |
| Is it paging in a loop, or asking once per item? | [`pagination`](../repeats/pagination.md) |
| Did the app ignore a rate limit? | [`rate-limit`](../repeats/rate-limits.md) |
| What differs between calls that look the same? | [`diff`](../repeats/diff.md) |

## I think something failed or was slow

| Question | Command |
|---|---|
| Which calls returned errors, and what did the server say? | [`errors`](../failures/errors.md) |
| Which routes redirect again and again, or send the client to another host? | [`redirects`](../failures/redirects.md) |
| Which calls failed first and succeeded on a later try? | [`transitions`](../failures/transitions.md) |
| Which calls were slowest, and what were they waiting for? | [`slowest`](../failures/slowest.md) |
| What does the start of the session look like? | [`startup`](../failures/startup.md) |
| What was the first failure, and what failed after it? | [`cascade`](../failures/cascade.md) |
| Did sign-in, tokens or a token refresh go wrong? | [`auth`](../failures/auth.md) |
| Is a token expired, or issued for the wrong place? | [`jwt`](../failures/jwt.md) |

## I want to look inside, or hand it on

| Question | Command |
|---|---|
| Show me one request and response in full | [`show-entry`](../inspect/show-entry.md) |
| Which responses mention this text? | [`search`](../inspect/search.md) |
| What is the value of this field across responses? | [`extract`](../inspect/extract.md) |
| Give me the entries as data for a spreadsheet or `jq` | [`export`](../inspect/export.md) |
| Write something I can attach to a ticket | [`report`](../inspect/report.md) |
| Replay this call | [`curl`](../inspect/curl.md) |
| Give the backend team what they need to find these calls | [`handoff`](../inspect/handoff.md) |

## I want to compare or enforce

| Question | Command |
|---|---|
| What got worse since the last good capture? | [`compare`](../compare/compare.md) |
| Does the capture break my own rules? | [`rules`](../compare/rules.md) |
| Are required headers missing, or content types wrong? | [`checks`](../compare/checks.md) |

## Two things that apply to every command

- **Narrow it.** `--filter` limits a command to matching requests. `validate`, `show-entry` and `curl <id>` ignore it. See [Filter entries](../analyze/filters.md).
- **Script it.** `--json` prints the result as a stable document, and for most commands the exit code says whether there were findings.
  Nine commands always exit `0`: see [Errors and exit codes](../reference/errors-and-exit-codes.md#when-each-command-returns-1). See also [JSON output](../automate/json-output.md).

## Related

- [Your first triage](first-triage.md) uses six of these commands in a row.
- [Command line](../reference/command-line.md) lists every command and flag.
